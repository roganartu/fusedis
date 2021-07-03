mod fs;

use fuser::MountOption;
use human_panic::setup_panic;
use structopt::clap::{crate_version, App, Arg};

fn main() {
    setup_panic!();
    // TODO add some more options:
    //   - config file path
    //   - redis url(s)
    //   - sentinel mode
    let matches = App::new("fusedis")
        .version(crate_version!())
        .author("Tony Lykke")
        .arg(
            Arg::with_name("MOUNT_POINT")
                .required(true)
                .index(1)
                .help("Path to mount fusedis at."),
        )
        .arg(
            Arg::with_name("allow-root")
                .long("allow-root")
                .help("Allow root user to access filesystem."),
        )
        .arg(
            Arg::with_name("read-only")
                .long("read-only")
                .help("Mount FS in read-only mode. Implies --no-raw."),
        )
        .arg(
            Arg::with_name("no-raw")
                .long("no-raw")
                .help("Don't mount /raw path that accepts raw Redis commands."),
        )
        .get_matches();
    env_logger::init();
    let mountpoint = matches.value_of("MOUNT_POINT").unwrap();
    let mut options = vec![MountOption::FSName("hello".to_string())];
    options.push(MountOption::AutoUnmount);
    if matches.is_present("allow-root") {
        options.push(MountOption::AllowRoot);
    }
    if matches.is_present("read-only") {
        options.push(MountOption::RO);
        // TODO set --no-raw
    } else {
        options.push(MountOption::RW);
    }
    // TODO handle --no-raw
    fuser::mount2(fs::HelloFS, mountpoint, &options).unwrap();
}
