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
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1); // 1 second

const HELLO_DIR_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o755,
    nlink: 2,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

const HELLO_TXT_CONTENT: &str = "Hello World!\n";

const HELLO_TXT_ATTR: FileAttr = FileAttr {
    ino: 2,
    size: 13,
    blocks: 1,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::RegularFile,
    perm: 0o644,
    nlink: 1,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

// ino, type, attr, name
type DirEntry = (u64, FileType, FileAttr, String);

#[derive(Debug, Clone)]
pub struct KVFS {
    pub config: Config,
    // TODO these both impl r2d2::ManageConnection, surely we don't need two attrs?
    pub pool: Option<r2d2::Pool<RedisConnectionManager>>,
    pub cluster_pool: Option<r2d2::Pool<RedisClusterConnectionManager>>,
    pub direntries_by_group: HashMap<String, Vec<DirEntry>>,
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

        if parent == 1 {
            match self.direntries_by_parent_ino.get(&parent) {
                Some(entries) => match entries.get(&name_str) {
                    Some(entry) => reply.entry(&TTL, &entry.2, 0),
                    None => reply.error(ENOENT),
                },
                None => reply.error(ENOENT),
            };
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &HELLO_DIR_ATTR),
            2 => reply.attr(&TTL, &HELLO_TXT_ATTR),
            _ => reply.error(ENOENT),
        }
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
        if ino == 2 {
            reply.data(&HELLO_TXT_CONTENT.as_bytes()[offset as usize..]);
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let mut entries: Vec<(u64, FileType, String)> = vec![
            (1, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()),
        ];

        // Root dir
        entries.extend(match ino {
            // Root dir
            1..=2 => self.direntries_by_parent_ino[&ino]
                .iter()
                .map(|(_, v)| (v.0, v.1, v.3.clone())),
            // 1 => match self.get_root_direntries() {
            //     Ok(v) => v,
            //     Err(e) => {
            //         log::error!("Error listing root directory: {}", e);
            //         reply.error(EAGAIN);
            //         return;
            //     }
            // },
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
                2,
                FileType::Directory,
                self.get_dir_attr("/raw", 2),
                "raw".to_string(),
            ))
        }
        // (2, FileType::Directory, None, "lock".to_string()),
        // (2, FileType::Directory, None, "kv".to_string()),
        self.direntries_by_group
            .insert("root".to_string(), root_entries.clone());
        self.direntries_by_parent_ino.insert(
            1,
            root_entries
                .iter()
                .map(|e| (e.3.clone(), e.clone()))
                .collect(),
        );
    }

    fn get_dir_attr(&mut self, path: &str, ino: u64) -> FileAttr {
        // TODO implement permissions adjustments from config.
        let now = SystemTime::now();
        FileAttr {
            ino: ino,
            size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::Directory,
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
