mod config;
mod drivers;
mod fuse;

#[macro_use]
extern crate quick_error;
extern crate log;
extern crate toml;
extern crate url;

use env_logger::Env;
use fuser::MountOption;
use human_panic::setup_panic;
use redis;
use std::error;
use std::path::PathBuf;
use std::process;
use structopt::clap::arg_enum;
use structopt::StructOpt;
use users;
use whoami;

type CLIResult<T> = std::result::Result<T, Box<dyn error::Error>>;

arg_enum! {
    #[derive(Debug, Clone)]
    enum LogLevel {
        Debug,
        Info,
        Error,
    }
}

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "fusekv",
    about = "FUSE key/value store backed by Redis.",
    author = "Tony Lykke <hi@tonylykke.com>",
    rename_all = "kebab-case"
)]
struct Opt {
    /// Path to mount fusekv
    #[structopt(parse(from_os_str))]
    mount: PathBuf,

    /// Path to config file
    #[structopt(parse(from_os_str), short, long)]
    config: Option<PathBuf>,

    /// Redis server(s) to connect to [default: redis://127.0.0.1:6379]
    #[structopt(short, long)]
    server: Option<url::Url>,

    /// Enable Redis cluster mode
    #[structopt(long)]
    cluster_mode: bool,

    /// Mount fusekv in read-only mode. Implies --no-raw
    #[structopt(long)]
    read_only: bool,

    /// Don't mount /raw path that accepts raw Redis commands
    #[structopt(long)]
    disable_raw: bool,

    /// Set the allow_other mount option. Requires root or user_allow_other set in /etc/fuse.conf
    #[structopt(long)]
    allow_other: bool,

    /// User to mount fusekv as. Defaults to current user.
    #[structopt(short, long)]
    user: Option<String>,

    /// Group to mount fusekv as. Defaults to current user.
    #[structopt(short, long)]
    group: Option<String>,

    /// Permissions to give all paths under mount. Must be octal. Use config file for finer-grained, path-based control [default: 755]
    #[structopt(long, parse(try_from_str = config::parse_octal))]
    chmod: Option<u16>,

    /// Maximum number of keys to return to readdir. Set to -1 to disable [default: 1000]
    #[structopt(short, long)]
    max_results: Option<i64>,
}

fn main() {
    setup_panic!();
    process::exit(match run_app() {
        Ok(_) => 0,
        Err(err) => {
            eprintln!("error: {:?}", err);
            1
        }
    });
}

fn run_app() -> CLIResult<()> {
    let env = Env::default().filter_or("FUSEKV_LOG_LEVEL", "info");
    env_logger::init_from_env(env);
    log::debug!("Parsing CLI args.");
    let opt = Opt::from_args();
    log::debug!("Parsed {:?} from CLI.", opt);
    let mountpoint = opt.mount.clone();
    let mut config = match merge_config(opt) {
        Ok(config) => config,
        Err(e) => return Err(Box::new(e)),
    };
    log::debug!("Final loaded config: {:?}.", config);

    // Setup fuse options
    let mut fuse_options = vec![
        MountOption::FSName("fusekv".to_string()),
        MountOption::AutoUnmount,
        MountOption::NoExec,
        MountOption::DefaultPermissions,
        // TODO would async be faster? is that compatible with what we are doing with
        // redis kv?
        MountOption::DirSync,
        MountOption::Sync,
        // TODO do we need block/special devices?
        MountOption::NoDev,
        // TODO do we need to support Suid?
        MountOption::NoSuid,
        // Tracking atime would have perf issues, so don't bother
        MountOption::NoAtime,
    ];
    if config.allow_other {
        log::info!("Setting allow_other mount option.");
        fuse_options.push(MountOption::AllowOther);
    }
    if config.read_only {
        log::info!("Mounting in read-only mode.");
        fuse_options.push(MountOption::RO);
        log::info!("Disabling raw command support due to read-only mode.");
        config.disable_raw = true;
    } else {
        log::info!("Mounting in read-write mode.");
        fuse_options.push(MountOption::RW);
    }

    // TODO how to support multiple drivers here? Do we need a function that returns
    // an Option and then we can match->err on that?
    let mut driver = drivers::redis::RedisDriver::new(match config.redis {
        Some(url) => {
            log::debug!("Attempting to connect to redis URL {}.", url);
            match redis::Client::open(url.to_string()) {
                Ok(v) => v,
                Err(e) => return Err(Box::new(e)),
            }
        }
        None => return Err(Box::new(config::ConfigError::NoDriver)),
    });

    let mut kvfs = fuse::KVFS::new(config.clone(), driver);

    log::debug!("Building directory structure.");
    kvfs.init_static_dirs();

    // Mount the filestystem
    log::info!("Mounting fusekv at {}.", mountpoint.display());
    match fuser::mount2(kvfs, mountpoint, &fuse_options) {
        Ok(v) => Ok(v),
        Err(e) => Err(Box::new(e)),
    }
}

// Merge cli options with config file options.
// CLI options take precedence.
fn merge_config(opt: Opt) -> Result<config::Config, config::ConfigError> {
    let cfgfile = match opt.config {
        Some(config_file) => {
            log::debug!("Reading config from {}.", config_file.display());
            match config::load_file(config_file) {
                Ok(cfg) => cfg,
                Err(e) => return Err(e),
            }
        }
        None => config::ConfigFile::default(),
    };
    let cfg = config::Config {
        cluster_mode: opt.cluster_mode
            || match cfgfile.cluster_mode {
                Some(cfgval) => cfgval,
                None => false,
            },
        redis: match opt.server {
            Some(optval) => Some(config::RedisServer { url: optval }),
            None => match cfgfile.redis {
                Some(cfgval) => Some(cfgval),
                None => Some(config::RedisServer {
                    url: url::Url::parse("redis://127.0.0.1:6379").unwrap(),
                }),
            },
        },
        permission: match cfgfile.permission {
            Some(permission) => permission,
            None => vec![],
        },
        disable_raw: opt.disable_raw
            || match cfgfile.disable_raw {
                Some(cfgval) => cfgval,
                None => false,
            },
        read_only: opt.read_only
            || match cfgfile.read_only {
                Some(cfgval) => cfgval,
                None => false,
            },
        allow_other: opt.allow_other
            || match cfgfile.allow_other {
                Some(cfgval) => cfgval,
                None => false,
            },
        // Defaults to the current user
        uid: match users::get_user_by_name(&match opt.user {
            Some(optval) => optval,
            None => match cfgfile.user {
                Some(cfgval) => cfgval,
                None => whoami::username(),
            },
        }) {
            Some(v) => v.uid(),
            None => return Err(config::ConfigError::UserNotFound),
        },
        // Defaults to the current user
        gid: match users::get_group_by_name(&match opt.group {
            Some(optval) => optval,
            None => match cfgfile.group {
                Some(cfgval) => cfgval,
                None => match users::get_current_groupname() {
                    Some(v) => v.into_string().unwrap(),
                    None => return Err(config::ConfigError::UserNotFound),
                },
            },
        }) {
            Some(v) => v.gid(),
            None => return Err(config::ConfigError::GroupNotFound),
        },
        // Defaults to read/write by current user.
        chmod: match opt.chmod {
            Some(optval) => optval,
            None => match cfgfile.chmod {
                Some(cfgval) => cfgval,
                None => 0o755,
            },
        },
        max_results: match opt.max_results {
            Some(optval) => optval,
            None => match cfgfile.max_results {
                Some(cfgval) => cfgval,
                None => 1000,
            },
        },
    };
    Ok(cfg)
}
