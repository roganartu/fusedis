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
    //   - read-only mode
    let matches = App::new("fusedis")
        .version(crate_version!())
        .author("Tony Lykke")
        .arg(
            Arg::with_name("MOUNT_POINT")
                .required(true)
                .index(1)
                .help("Act as a client, and mount FUSE at given path"),
        )
        .arg(
            Arg::with_name("allow-root")
                .long("allow-root")
                .help("Allow root user to access filesystem"),
        )
        .get_matches();
    env_logger::init();
    let mountpoint = matches.value_of("MOUNT_POINT").unwrap();
    let mut options = vec![MountOption::RO, MountOption::FSName("hello".to_string())];
    options.push(MountOption::AutoUnmount);
    if matches.is_present("allow-root") {
        options.push(MountOption::AllowRoot);
    }
    fuser::mount2(fs::HelloFS, mountpoint, &options).unwrap();
}
