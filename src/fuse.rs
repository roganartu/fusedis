use crate::config::Config;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use libc::{EAGAIN, ENOENT};
use r2d2_redis::RedisConnectionManager;
use r2d2_redis_cluster::{r2d2, RedisClusterConnectionManager};
use std::collections::HashMap;
use std::error;
use std::ffi::OsStr;
use std::time::{Duration, SystemTime};

const TTL: Duration = Duration::from_secs(1); // 1 second

// /raw
const RAW_START: u64 = 2;
const RAW_END: u64 = 8191;
// /lock/<name>
const LOCK_START: u64 = 8192;
const LOCK_END: u64 = 100_000_000_000;
// /kv/<name>
const KV_START: u64 = 200_000_000_000;
const KV_END: u64 = 300_000_000_000;

const RAW_HELP: &str = "Send raw commands to Redis.

TODO fill this in with how to use /raw
";

const LOCK_HELP: &str = "Atomic locks via files.

TODO fill this in with how to use /lock
";

const KV_HELP: &str = "Key/Value store via files.

TODO fill this in with how to use /kv
";

// ino, type, attr, name, content
type DirEntry = (u64, FileType, FileAttr, String, Option<String>);

// ino, type, name
type ReadDirEntry = (u64, FileType, String);

#[derive(Debug, Clone)]
pub struct KVFS {
    pub config: Config,
    // TODO these both impl r2d2::ManageConnection, surely we don't need two attrs?
    pub pool: Option<r2d2::Pool<RedisConnectionManager>>,
    pub cluster_pool: Option<r2d2::Pool<RedisClusterConnectionManager>>,
    pub direntries_by_ino: HashMap<u64, DirEntry>,
    pub direntries_by_parent_ino: HashMap<u64, HashMap<String, DirEntry>>,
}

impl Filesystem for KVFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = match name.to_os_string().into_string() {
            Ok(v) => v,
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
        };

        // FUSE root
        if parent == 1 {
            match self.direntries_by_parent_ino.get(&parent) {
                Some(entries) => match entries.get(&name_str) {
                    Some(entry) => reply.entry(&TTL, &entry.2, 0),
                    None => reply.error(ENOENT),
                },
                None => reply.error(ENOENT),
            };
        // TODO add ranges for /lock and /kv here
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            0..=RAW_END => match self.direntries_by_ino.get(&ino) {
                Some(v) => reply.attr(&TTL, &v.2),
                None => reply.error(ENOENT),
            },
            // TODO add ranges for /lock and /kv here
            _ => reply.error(ENOENT),
        };
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        _size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        match ino {
            // FUSE internal range
            0..=RAW_END => match self.direntries_by_ino.get(&ino) {
                Some(v) => match &v.4 {
                    Some(content) => reply.data(&content.as_bytes()[offset as usize..]),
                    None => reply.error(ENOENT),
                },
                None => reply.error(ENOENT),
            },
            // TODO add ranges for /lock and /kv
            _ => reply.error(ENOENT),
        };
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let mut entries: Vec<ReadDirEntry> = vec![(1, FileType::Directory, "..".to_string())];

        // Root dir
        entries.extend(match ino {
            // Root dir
            1 => self.direntries_by_parent_ino[&ino]
                .iter()
                .map(|(_, v)| (v.0, v.1, v.3.clone()))
                .collect::<Vec<ReadDirEntry>>(),
            // /kv
            2048 => match self.get_kv_direntries() {
                Ok(v) => v,
                Err(e) => {
                    log::error!("Error listing root directory: {}", e);
                    reply.error(EAGAIN);
                    return;
                }
            },
            _ => {
                reply.error(ENOENT);
                return;
            }
        });

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}

impl KVFS {
    // Initialize all the static dirs based on the KVFS config.
    // The root dir reserves the first 8192 inodes (13 bits), leaving 51 bits for
    // remaining keys (~2 quadrillion values).
    pub fn init_static_dirs(&mut self) {
        log::debug!("Building static directory list.");
        let mut root_entries: Vec<DirEntry> = vec![];
        if !self.config.disable_raw {
            log::debug!("Setting up /raw, to disable set disable_raw=true.");
            root_entries.push((
                1,
                FileType::Directory,
                self.get_attr(".", FileType::Directory, 1, 0),
                ".".to_string(),
                None,
            ));
            root_entries.push((
                2,
                FileType::RegularFile,
                self.get_attr("/raw", FileType::RegularFile, 2, 0),
                "raw".to_string(),
                None,
            ));
            root_entries.push((
                3,
                FileType::RegularFile,
                self.get_attr("/raw:help", FileType::RegularFile, 3, RAW_HELP.len() as u64),
                "raw:help".to_string(),
                Some(RAW_HELP.to_string()),
            ));
        }

        log::debug!("Setting up /lock.");
        root_entries.push((
            2048,
            FileType::Directory,
            self.get_attr("/lock", FileType::Directory, 2048, 0),
            "lock".to_string(),
            None,
        ));
        root_entries.push((
            2049,
            FileType::RegularFile,
            self.get_attr(
                "/lock:help",
                FileType::RegularFile,
                2049,
                LOCK_HELP.len() as u64,
            ),
            "lock:help".to_string(),
            Some(LOCK_HELP.to_string()),
        ));

        log::debug!("Setting up /kv.");
        root_entries.push((
            4096,
            FileType::Directory,
            self.get_attr("/kv", FileType::Directory, 4096, 0),
            "kv".to_string(),
            None,
        ));
        root_entries.push((
            4097,
            FileType::RegularFile,
            self.get_attr(
                "/kv:help",
                FileType::RegularFile,
                4097,
                KV_HELP.len() as u64,
            ),
            "kv:help".to_string(),
            Some(KV_HELP.to_string()),
        ));

        self.direntries_by_parent_ino.insert(
            1,
            root_entries
                .iter()
                .map(|e| (e.3.clone(), e.clone()))
                .collect(),
        );
        for e in root_entries {
            self.direntries_by_ino.insert(e.0, e.clone());
        }
    }

    fn get_kv_direntries(&mut self) -> Result<Vec<ReadDirEntry>, Box<dyn error::Error>> {
        Ok(vec![])
    }

    fn get_attr(&mut self, path: &str, kind: FileType, ino: u64, size: u64) -> FileAttr {
        // TODO implement permissions adjustments from config.
        let now = SystemTime::now();
        FileAttr {
            ino: ino,
            size: size,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: kind,
            perm: self.config.chmod,
            nlink: 1,
            uid: self.config.uid,
            gid: self.config.gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }
}
