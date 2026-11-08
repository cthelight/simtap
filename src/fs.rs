use std::ffi::OsStr;
use std::io;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use fuser::{
    AccessFlags, BsdFileFlags, Errno, FileAttr, FileHandle, Filesystem, FopenFlags, FileType,
    Generation, INodeNo, KernelConfig, OpenFlags, ReplyAttr, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyIoctl, ReplyLseek, ReplyOpen, ReplyStatfs, ReplyWrite,
    Request, TimeOrNow, WriteFlags, IoctlFlags, LockOwner,
};

use crate::cartridge::CartridgeLibrary;
use crate::tape::{MTIOCGET, MTIOCTOP, MTIOCPOS, TapeState};

const ROOT_INO: u64 = 1;
const TAPE_INO: u64 = 2; // /dev/st0 — rewinds on close
const NST_INO: u64 = 3; // /dev/nst0 — head stays put on close
const TAPE_NAME: &str = "tape";
const NST_NAME: &str = "nst";

/// True for either drive device file (st or nst).
fn is_drive(ino: u64) -> bool {
    matches!(ino, TAPE_INO | NST_INO)
}

/// The FUSE filesystem. The root directory exposes a single regular file,
/// `tape`, which is the emulated drive. All I/O on that file is dispatched to
/// the shared [`TapeState`] (via the library), mirroring the st driver.
pub struct SimFs {
    pub library: Arc<CartridgeLibrary>,
}

impl SimFs {
    fn root_attr(&self) -> FileAttr {
        let now = SystemTime::now();
        FileAttr {
            ino: INodeNo(ROOT_INO),
            size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

    fn drive_attr(&self, st: &TapeState, ino: u64) -> FileAttr {
        let now = SystemTime::now();
        FileAttr {
            ino: INodeNo(ino),
            size: st.data_len,
            blocks: st.data_len.div_ceil(512),
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::RegularFile,
            perm: 0o660,
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }
}

impl Filesystem for SimFs {
    fn init(&mut self, _req: &Request, _config: &mut KernelConfig) -> io::Result<()> {
        Ok(())
    }

    fn destroy(&mut self) {
        let _ = self.library.save();
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        if parent.0 != ROOT_INO {
            reply.error(Errno::ENOENT);
            return;
        }
        let name = name.to_string_lossy();
        match name.as_ref() {
            TAPE_NAME | NST_NAME => {
                let ino = if name == TAPE_NAME { TAPE_INO } else { NST_INO };
                let g = self.library.tape.lock().unwrap();
                let attr = self.drive_attr(&g, ino);
                reply.entry(&Duration::from_secs(1), &attr, Generation(0));
            }
            "." | ".." => {
                let attr = self.root_attr();
                reply.entry(&Duration::from_secs(1), &attr, Generation(0));
            }
            _ => reply.error(Errno::ENOENT),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let attr = match ino.0 {
            ROOT_INO => self.root_attr(),
            i if is_drive(i) => {
                let g = self.library.tape.lock().unwrap();
                self.drive_attr(&g, ino.0)
            }
            _ => {
                reply.error(Errno::ENOENT);
                return;
            }
        };
        reply.attr(&Duration::from_secs(1), &attr);
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        // A tape has no notion of a byte-offset truncation (the st driver
        // ignores O_TRUNC), and no other attribute is meaningful for a drive.
        let attr = match ino.0 {
            ROOT_INO => self.root_attr(),
            i if is_drive(i) => {
                let g = self.library.tape.lock().unwrap();
                self.drive_attr(&g, ino.0)
            }
            _ => {
                reply.error(Errno::ENOENT);
                return;
            }
        };
        reply.attr(&Duration::from_secs(1), &attr);
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        if !is_drive(ino.0) {
            reply.error(Errno::ENOTTY);
            return;
        }
        let raw = flags.0;
        let nonblock = (raw & libc::O_NONBLOCK) != 0;
        // st.c:1280 rew_at_close: /dev/st0 (TAPE_INO) rewinds on close;
        // /dev/nst0 (NST_INO) leaves the head where it is.
        let rewind = ino.0 == TAPE_INO;
        match self.library.with_tape(|st| st.open(raw, nonblock, rewind)) {
            Ok(()) => reply.opened(FileHandle(0), FopenFlags::FOPEN_DIRECT_IO),
            Err(e) => reply.error(Errno::from_i32(e.errno())),
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        if !is_drive(ino.0) {
            reply.error(Errno::ENOTTY);
            return;
        }
        match self.library.with_tape(|st| {
            // The kernel's per-open file offset (`offset`) starts at 0 on every
            // open. A real tape's head is persistent: for the non-rewinding
            // (nst) device it sits mid-tape after the previous close, so the
            // head is open_base (the head at open time) + the in-open offset.
            // For the st device open_base is 0 (rewound on close), so this is
            // just st.pos = offset. Serving data at exactly the position the
            // kernel expects keeps FUSE's page cache (keyed by offset)
            // consistent, even when the kernel reads ahead or out of order.
            st.pos = st.open_base + offset;
            st.read(size as usize)
        }) {
            Ok(buf) => reply.data(&buf),
            Err(e) => reply.error(Errno::from_i32(e.errno())),
        }
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        if !is_drive(ino.0) {
            reply.error(Errno::ENOTTY);
            return;
        }
        match self.library.with_tape(|st| {
            // Write at open_base + in-open offset (see the note in read()). For
            // nst this appends after the previous close instead of overwriting
            // at BOT; for st it is just st.pos = offset.
            st.pos = st.open_base + offset;
            st.write(data)
        }) {
            Ok(n) => reply.written(n as u32),
            Err(e) => reply.error(Errno::from_i32(e.errno())),
        }
    }

    fn lseek(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        _offset: i64,
        _whence: i32,
        reply: ReplyLseek,
    ) {
        // st_fops.llseek = noop_llseek (st.c:4158): a tape is not sparse, so
        // SEEK_HOLE/SEEK_DATA (the only lseeks the FUSE kernel forwards) fail
        // with ESPIPE. SEEK_SET/CUR are handled by the kernel's
        // generic_file_llseek and reached us as the read/write offset instead.
        match ino.0 {
            i if is_drive(i) || i == ROOT_INO => reply.error(Errno::ESPIPE),
            _ => reply.error(Errno::ENOENT),
        }
    }

    fn flush(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        _lock_owner: LockOwner,
        reply: ReplyEmpty,
    ) {
        if is_drive(ino.0) {
            // st_flush: one FLUSH per closed fd (dup'd fds included). Auto
            // WEOF if still writing; the drive itself is released by
            // st_release (FUSE RELEASE, last reference).
            let _ = self.library.with_tape(|st| st.flush());
            let _ = self.library.save();
        }
        reply.ok();
    }

    fn release(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        // st_release: the kernel sends RELEASE once, when the last reference
        // to the open file is gone (after the final FLUSH). Rewind (st device
        // file) and free the drive.
        if is_drive(ino.0) {
            let _ = self.library.with_tape(|st| st.release());
            let _ = self.library.save();
        }
        reply.ok();
    }

    fn ioctl(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        _flags: IoctlFlags,
        cmd: u32,
        in_data: &[u8],
        _out_size: u32,
        reply: ReplyIoctl,
    ) {
        if !is_drive(ino.0) {
            reply.error(Errno::ENOTTY);
            return;
        }
        match cmd {
            MTIOCTOP => {
                if in_data.len() < 8 {
                    reply.error(Errno::EINVAL);
                    return;
                }
                // struct mtop { int mt_op; int mt_count; }
                let op = i32::from_le_bytes([in_data[0], in_data[1], in_data[2], in_data[3]]) as i16;
                let count = i32::from_le_bytes([in_data[4], in_data[5], in_data[6], in_data[7]]);
                match self.library.with_tape(|st| st.mt_op(op, count)) {
                    Ok(()) => reply.ioctl(0, &[]),
                    Err(e) => reply.error(Errno::from_i32(e.errno())),
                }
            }
            MTIOCGET => {
                let bytes = self.library.mtget();
                reply.ioctl(0, &bytes);
            }
            MTIOCPOS => {
                let bytes = self.library.mtpos();
                reply.ioctl(0, &bytes);
            }
            _ => reply.error(Errno::ENOTTY),
        }
    }

    fn readdir(&self, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
        if ino.0 != ROOT_INO {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let entries: [(u64, FileType, &str); 4] = [
            (ROOT_INO, FileType::Directory, "."),
            (ROOT_INO, FileType::Directory, ".."),
            (TAPE_INO, FileType::RegularFile, TAPE_NAME),
            (NST_INO, FileType::RegularFile, NST_NAME),
        ];
        let start = offset as usize;
        for (i, (ino_n, kind, name)) in entries.iter().enumerate() {
            if i < start {
                continue;
            }
            if reply.add(INodeNo(*ino_n), (i + 1) as u64, *kind, *name) {
                break;
            }
        }
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let g = self.library.tape.lock().unwrap();
        let cap = g.capacity;
        let bsize = 4096u32;
        let blocks = cap / bsize as u64 + 1;
        reply.statfs(blocks, blocks, blocks, 1, 1, bsize, 255, bsize);
    }

    fn access(&self, _req: &Request, ino: INodeNo, _mask: AccessFlags, reply: ReplyEmpty) {
        if ino.0 == ROOT_INO || is_drive(ino.0) {
            reply.ok();
        } else {
            reply.error(Errno::ENOENT);
        }
    }

    fn create(
        &self,
        _req: &Request,
        _parent: INodeNo,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: fuser::ReplyCreate,
    ) {
        reply.error(Errno::EACCES);
    }
}
