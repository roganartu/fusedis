mod config;
mod fs;

#[macro_use]
extern crate quick_error;
extern crate toml;
extern crate url;

use fuser::MountOption;
use human_panic::setup_panic;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "fusedis",
    about = "Redis KV store via FUSE.",
    author = "Tony Lykke",
    rename_all = "kebab-case"
)]
struct Opt {
    /// Path to mount fusedis
    #[structopt(parse(from_os_str))]
    mount: PathBuf,

    /// Path to config file
    #[structopt(parse(from_os_str), short, long)]
    config: Option<PathBuf>,

    /// Mount fusedis in read-only mode. Implies --no-raw
    #[structopt(long)]
    read_only: bool,

    /// Don't mount /raw path that accepts raw Redis commands
    #[structopt(long)]
    disable_raw: bool,

    /// Set the allow_other mount option. Requires root or user_allow_other set in /etc/fuse.conf
    #[structopt(long)]
    allow_other: bool,

    /// User to mount fusedis as. Defaults to current user.
    #[structopt(short, long)]
    user: Option<String>,

    /// Group to mount fusedis as. Defaults to current user.
    #[structopt(short, long)]
    group: Option<String>,

    /// Permissions to give all paths under mount. Must be octal. Use config file for finer-grained control.
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
    // TODO load config from opt
    env_logger::init();
    let mut fuse_options = vec![
        MountOption::FSName("fusedis".to_string()),
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
    if opt.allow_other {
        fuse_options.push(MountOption::AllowOther);
    }
    if opt.read_only {
        fuse_options.push(MountOption::RO);
        // TODO set --no-raw
    } else {
        fuse_options.push(MountOption::RW);
    }
    // TODO handle --no-raw
    fuser::mount2(fs::HelloFS, opt.mount, &fuse_options).unwrap();
}
