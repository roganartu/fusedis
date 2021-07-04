mod config;
mod fuse;

#[macro_use]
extern crate quick_error;
extern crate toml;
extern crate url;

use fuser::MountOption;
use human_panic::setup_panic;
use std::path::PathBuf;
use structopt::StructOpt;
use whoami::username;

#[derive(Debug, StructOpt)]
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

    /// Permissions to give all paths under mount. Must be octal. Use config file for finer-grained, path-based control [default: 644]
    #[structopt(long, parse(try_from_str = config::parse_octal))]
    chmod: Option<u16>,
}

fn main() {
    setup_panic!();
    // TODO add some more options:
    //   - config file path
    //   - redis url(s)
    //   - sentinel mode
    let opt = Opt::from_args();
    let mountpoint = opt.mount.clone();
    let config = match merge_config(opt) {
        Ok(config) => config,
        Err(e) => panic!("{}", e),
    };
    env_logger::init();
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
        fuse_options.push(MountOption::AllowOther);
    }
    if config.read_only {
        fuse_options.push(MountOption::RO);
        // TODO set --no-raw
    } else {
        fuse_options.push(MountOption::RW);
    }
    // TODO handle --no-raw
    // TODO define some fields on the KVFS and unwrap them from config
    fuser::mount2(fuse::HelloFS, mountpoint, &fuse_options).unwrap();
}

// Merge cli options with config file options.
// CLI options take precedence.
fn merge_config(opt: Opt) -> Result<config::Config, config::ConfigError> {
    let cfgfile = match opt.config {
        Some(config_file) => match config::load_file(config_file) {
            Ok(cfg) => cfg,
            Err(e) => return Err(e),
        },
        None => config::ConfigFile::default(),
    };
    let cfg = config::Config {
        server: match opt.server {
            Some(optval) => vec![config::RedisServer { url: optval }],
            None => match cfgfile.server {
                Some(cfgval) => cfgval,
                None => vec![],
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
        user: match opt.user {
            Some(optval) => optval,
            None => match cfgfile.user {
                Some(cfgval) => cfgval,
                None => whoami::username(),
            },
        },
        // Defaults to the current user
        group: match opt.group {
            Some(optval) => optval,
            None => match cfgfile.group {
                Some(cfgval) => cfgval,
                None => username(),
            },
        },
        // Defaults to read/write by current user.
        chmod: match opt.chmod {
            Some(optval) => optval,
            None => match cfgfile.chmod {
                Some(cfgval) => cfgval,
                None => 0o644,
            },
        },
    };
    println!("{:?}", cfg);
    Ok(cfg)
}
