use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

// ===================== MT operation codes (linux/mtio.h) =====================
pub const MTRESET: i16 = 0;
pub const MTFSF: i16 = 1;
pub const MTBSF: i16 = 2;
pub const MTFSR: i16 = 3;
pub const MTBSR: i16 = 4;
pub const MTWEOF: i16 = 5;
pub const MTREW: i16 = 6;
pub const MTOFFL: i16 = 7;
pub const MTNOP: i16 = 8;
pub const MTRETEN: i16 = 9;
pub const MTBSFM: i16 = 10;
pub const MTFSFM: i16 = 11;
pub const MTEOM: i16 = 12;
pub const MTERASE: i16 = 13;
pub const MTRAS1: i16 = 14;
pub const MTRAS2: i16 = 15;
pub const MTRAS3: i16 = 16;
pub const MTSETBLK: i16 = 20;
pub const MTSETDENSITY: i16 = 21;
pub const MTSEEK: i16 = 22;
pub const MTTELL: i16 = 23;
pub const MTSETDRVBUFFER: i16 = 24;
pub const MTFSS: i16 = 25;
pub const MTBSS: i16 = 26;
pub const MTWSM: i16 = 27;
pub const MTLOCK: i16 = 28;
pub const MTUNLOCK: i16 = 29;
pub const MTLOAD: i16 = 30;
pub const MTUNLOAD: i16 = 31;
pub const MTCOMPRESSION: i16 = 32;
pub const MTSETPART: i16 = 33;
pub const MTMKPART: i16 = 34;
pub const MTWEOFI: i16 = 35;

// ===================== MT_ST masks/shifts =====================
pub const MT_ST_BLKSIZE_SHIFT: u32 = 0;
pub const MT_ST_BLKSIZE_MASK: u32 = 0x00ffffff;
pub const MT_ST_DENSITY_SHIFT: u32 = 24;
pub const MT_ST_DENSITY_MASK: u32 = 0xff000000;

// MTSETDRVBUFFER (op 24) extended-option encodings (mtio.h). The top byte of
// the argument selects the operation; the remaining bits are the option value.
// Plain `mt drvbuffer <n>` uses code 0; the boolean option ops (stoptions /
// stsetoptions / stclearoptions) carry these codes and toggle do_buffer_writes
// / do_async_writes, which together with drv_buffer drive GMT_IM_REP_EN.
pub const MT_ST_OPTIONS: u32 = 0xf0000000;
pub const MT_ST_BOOLEANS: u32 = 0x10000000;
pub const MT_ST_SETBOOLEANS: u32 = 0x30000000;
pub const MT_ST_CLEARBOOLEANS: u32 = 0x40000000;
pub const MT_ST_BUFFER_WRITES: u32 = 0x1;
pub const MT_ST_ASYNC_WRITES: u32 = 0x2;

// ===================== GMT status flags (mtget.mt_gstat) =====================
pub const GMT_EOF: u32 = 0x80000000;
pub const GMT_BOT: u32 = 0x40000000;
pub const GMT_EOT: u32 = 0x20000000;
pub const GMT_SM: u32 = 0x10000000;
pub const GMT_EOD: u32 = 0x08000000;
pub const GMT_WR_PROT: u32 = 0x04000000;
pub const GMT_ONLINE: u32 = 0x01000000;
pub const GMT_D_6250: u32 = 0x00800000;
pub const GMT_D_1600: u32 = 0x00400000;
pub const GMT_D_800: u32 = 0x00200000;
pub const GMT_DR_OPEN: u32 = 0x00040000;
pub const GMT_IM_REP_EN: u32 = 0x00010000;
pub const GMT_CLN: u32 = 0x00008000;

// ===================== mt_type =====================
pub const MT_ISSCSI2: u32 = 0x72;

// ===================== ST eof states (drivers/scsi/st.h) =====================
pub const ST_NOEOF: u8 = 0;
pub const ST_FM_HIT: u8 = 1;
pub const ST_FM: u8 = 2;
pub const ST_EOM_OK: u8 = 3;
pub const ST_EOM_ERROR: u8 = 4;
pub const ST_EOD_1: u8 = 5;
pub const ST_EOD_2: u8 = 6;
pub const ST_EOD: u8 = 7;

// ===================== ST rw states =====================
pub const ST_IDLE: u8 = 0;
pub const ST_READING: u8 = 1;
pub const ST_WRITING: u8 = 2;

// ===================== ioctl command encodings (64-bit) =====================
pub const MTIOCTOP: u32 = 0x4008_6d01;
pub const MTIOCGET: u32 = 0x8030_6d02;
pub const MTIOCPOS: u32 = 0x8008_6d03;

// ===================== Error type =====================
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapeErr {
    EACCES,
    EINVAL,
    ENOSPC,
    ENOMEDIUM,
    EIO,
    ENOSYS,
    EBUSY,
    ENODEV,
    EPERM,
}

impl TapeErr {
    pub fn errno(&self) -> i32 {
        match self {
            TapeErr::EACCES => libc::EACCES,
            TapeErr::EINVAL => libc::EINVAL,
            TapeErr::ENOSPC => libc::ENOSPC,
            TapeErr::ENOMEDIUM => libc::ENOMEDIUM,
            TapeErr::EIO => libc::EIO,
            TapeErr::ENOSYS => libc::ENOSYS,
            TapeErr::EBUSY => libc::EBUSY,
            TapeErr::ENODEV => libc::ENODEV,
            TapeErr::EPERM => libc::EPERM,
        }
    }

    pub fn from_io(e: io::Error) -> TapeErr {
        match e.raw_os_error() {
            Some(raw) => TapeErr::from_raw(raw),
            None => TapeErr::EIO,
        }
    }

    pub fn from_raw(raw: i32) -> TapeErr {
        match raw {
            libc::EACCES => TapeErr::EACCES,
            libc::EINVAL => TapeErr::EINVAL,
            libc::ENOSPC => TapeErr::ENOSPC,
            libc::ENOMEDIUM => TapeErr::ENOMEDIUM,
            libc::ENOSYS => TapeErr::ENOSYS,
            libc::EBUSY => TapeErr::EBUSY,
            libc::ENODEV => TapeErr::ENODEV,
            libc::EPERM => TapeErr::EPERM,
            _ => TapeErr::EIO,
        }
    }
}

impl std::fmt::Display for TapeErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tape error: errno({})", self.errno())
    }
}

impl std::error::Error for TapeErr {}

pub type TRes<T> = Result<T, TapeErr>;

// ===================== TapeState =====================
//
// Model: the tape is a byte stream (the .tap file) plus a sorted list of
// file-mark byte offsets (`marks`). `pos` is the byte offset of the next
// record to read. File f occupies [start(f), start(f+1)) where
// start(0) = 0 and start(f) = marks[f-1].
//
// The state mirrors the st driver's per-partition bookkeeping (drv_file,
// drv_block, eof, at_sm, rw) so that MTIOCGET/MTIOCPOS report exactly what a
// real st-driven SCSI tape would.
#[derive(Debug)]
pub struct TapeState {
    pub label: Option<String>,
    pub loaded: bool,
    // Drive taken offline by `mt offline`/`mt eject` (MTOFFL/MTUNLOAD). The
    // medium stays in the drive but is not usable until `mt load` (MTLOAD) or
    // a fresh cartridge is inserted. Mirrors st.c ready==ST_NO_TAPE: I/O and
    // mt ops return ENOMEDIUM, and MTIOCGET reports GMT_DR_OPEN (st.c:4016).
    pub offline: bool,
    pub cartridge_write_prot: bool,
    pub capacity: u64,
    pub density: u32,
    pub block_size: u32, // 0 = variable-length records
    // Write-buffering model, mirroring the st driver's per-mode options and the
    // negotiated SCSI drive-buffering bit. Together they decide GMT_IM_REP_EN
    // (st.c:4020-4023): a real drive reports immediate-report mode whenever it
    // is doing async/buffered writes or has drive buffering enabled. Defaults
    // match the driver's compile-time defaults (ST_ASYNC_WRITES=1,
    // ST_BUFFER_WRITES=1) and the no-mode-sense fallback drv_buffer=1
    // (st.c:4497), so a fresh drive shows GMT_IM_REP_EN exactly as hardware does.
    pub do_buffer_writes: bool,
    pub do_async_writes: bool,
    pub drv_buffer: u32,
    pub data_file: Option<File>,
    pub data_len: u64,
    pub marks: Vec<u64>, // sorted file-mark byte offsets
    pub pos: u64,
    pub fileno: i64,
    pub blkno: i64,
    pub eof: u8,
    pub at_sm: bool,
    pub rw: u8,
    pub open_mode: i32, // 0 = not open, else O_ACCMODE value
    pub readonly: bool,
    pub in_use: bool,
    pub rewind_on_close: bool, // st.c:1280 rew_at_close: per-open, set by device file
    // Head position at open() time. FUSE reports the kernel's per-open file
    // offset (always 0 at the start of a fresh open); the real tape head is
    // persistent, so the head is open_base + (in-open offset). For the st
    // device open_base is 0 (rewound on close); for nst it is wherever the head
    // was left, letting a fresh nst open append mid-tape instead of at BOT.
    pub open_base: u64,
}

impl TapeState {
    pub fn offline(capacity: u64) -> Self {
        TapeState {
            label: None,
            loaded: false,
            offline: false,
            cartridge_write_prot: false,
            capacity,
            density: 0,
            block_size: 0,
            do_buffer_writes: true, // ST_BUFFER_WRITES default (st_options.h)
            do_async_writes: true, // ST_ASYNC_WRITES default (st_options.h)
            drv_buffer: 1, // "Try buffering if no mode sense" (st.c:4497)
            data_file: None,
            data_len: 0,
            marks: Vec::new(),
            pos: 0,
            fileno: 0,
            blkno: 0,
            eof: ST_NOEOF,
            at_sm: false,
            rw: ST_IDLE,
            open_mode: 0,
            readonly: false,
            in_use: false,
            rewind_on_close: true,
            open_base: 0,
        }
    }

    pub fn effective_write_prot(&self) -> bool {
        self.cartridge_write_prot || self.readonly
    }

    // st.c rw_checks (st.c:1524-1526): the drive is usable only when a medium
    // is loaded AND not taken offline. Otherwise read/write return ENOMEDIUM.
    fn ready(&self) -> bool {
        self.loaded && !self.offline
    }

    // ---------------- open / close ----------------

    pub fn open(&mut self, mode: i32, nonblock: bool, rewind_on_close: bool) -> TRes<()> {
        if self.in_use {
            return Err(TapeErr::EBUSY);
        }
        if !self.loaded && !nonblock {
            return Err(TapeErr::ENOMEDIUM);
        }
        self.in_use = true;
        self.open_mode = mode & libc::O_ACCMODE;
        self.readonly = self.open_mode == libc::O_RDONLY;
        // st.c:1280 rew_at_close = (iminor & 0x80) == 0 — /dev/st0 (rewind on
        // close) vs /dev/nst0 (head stays put).
        self.rewind_on_close = rewind_on_close;
        // The head is where it currently is; a fresh open's in-open offset (0)
        // maps onto this position (see `open_base` in fs.rs read/write).
        self.open_base = self.pos;
        Ok(())
    }

    // st_flush: called by the kernel on every close() of a file descriptor
    // (each dup'd fd closes separately). If we are still writing, write the
    // automatic file mark. Does NOT release the drive — a dup'd fd may still
    // be writing. (GNU dd dups its output fd, so one dd produces two FLUSHes.)
    pub fn flush(&mut self) -> TRes<()> {
        if self.rw == ST_WRITING {
            self.wteof(1)?;
            // The close auto-WEOF leaves the head AT the filemark (st.c:1442
            // sets ST_FM). For the st (rewind-on-close) device release() resets
            // it to ST_NOEOF right away; for nst it persists, so a later read of
            // the blank tape settles ST_FM -> ST_EOD -> -EIO exactly like a real
            // drive instead of returning 0 bytes forever.
            self.eof = ST_FM;
        }
        Ok(())
    }

    // st_release: called when the last reference to the open file goes away.
    // Rewinds for the st (rewind-on-close) device file and frees the drive.
    // Idempotent on in_use: FUSE delivers RELEASE once per open, after the
    // last FLUSH.
    pub fn release(&mut self) -> TRes<()> {
        if !self.in_use {
            return Ok(());
        }
        // Safety net: if the last FLUSH never arrived, still terminate the
        // file we were writing (a close always leaves the tape at an FM).
        if self.rw == ST_WRITING {
            self.wteof(1)?;
        }
        // MTREW on close only for the st (rewind) device file (st_flush:1448).
        if self.rewind_on_close {
            self.rewind();
        }
        self.in_use = false;
        self.open_mode = 0;
        self.readonly = false;
        self.rewind_on_close = true;
        Ok(())
    }

    // ---------------- position helpers ----------------

    fn rewind(&mut self) {
        self.pos = 0;
        self.fileno = 0;
        self.blkno = 0;
        self.eof = ST_NOEOF;
        self.at_sm = false;
        self.rw = ST_IDLE;
    }

    // Index of the first mark strictly greater than pos (the next FM ahead).
    fn next_mark(&self) -> Option<u64> {
        let idx = self.marks.partition_point(|&m| m <= self.pos);
        self.marks.get(idx).copied()
    }

    // Erase everything from `at` onward; head ends at the start of the file
    // located at `at`.
    fn truncate_at(&mut self, at: u64) -> TRes<()> {
        let idx = self.marks.partition_point(|&m| m <= at);
        self.marks.truncate(idx);
        if let Some(file) = self.data_file.as_mut() {
            file.set_len(at).map_err(TapeErr::from_io)?;
        }
        self.data_len = at;
        // All kept marks are <= at, so the head at `at` is at the start of
        // file (marks.len()).
        self.fileno = self.marks.len() as i64;
        self.blkno = 0;
        self.eof = ST_NOEOF;
        Ok(())
    }

    fn finalize_eof_zero(&mut self) {
        if self.eof == ST_FM {
            // Just crossed the last filemark; reading past recorded data hits
            // blank tape (st.c:2091-2096) and settles to ST_EOD on this read.
            self.eof = ST_EOD;
        } else if self.eof == ST_EOD_1 {
            self.eof = ST_EOD_2;
            self.blkno = 0;
            if self.fileno >= 0 {
                self.fileno += 1;
            }
        } else if self.eof == ST_EOD_2 {
            self.eof = ST_EOD;
        }
    }

    // ---------------- data I/O ----------------

    pub fn read(&mut self, n: usize) -> TRes<Vec<u8>> {
        if !self.ready() {
            return Err(TapeErr::ENOMEDIUM);
        }
        if n == 0 {
            return Ok(Vec::new());
        }
        if self.block_size != 0 && !n.is_multiple_of(self.block_size as usize) {
            return Err(TapeErr::EINVAL);
        }
        self.rw = ST_READING;

        // The zero-byte read that completes crossing an FM hit mid-read.
        if self.eof == ST_FM_HIT {
            self.eof = ST_FM;
            self.blkno = 0;
            if self.fileno >= 0 {
                self.fileno += 1;
            }
            return Ok(Vec::new());
        }

        let next_mark = self.next_mark();
        let limit = next_mark.unwrap_or(self.data_len);
        let avail = limit.saturating_sub(self.pos);

        if avail == 0 {
            // Past the last filemark, the drive keeps reporting blank tape:
            // two 0-byte reads (EOD_1->EOD_2->EOD) then -EIO (st.c:2176-2184).
            if self.eof == ST_EOD {
                return Err(TapeErr::EIO);
            }
            self.finalize_eof_zero();
            return Ok(Vec::new());
        }

        let take = std::cmp::min(n as u64, avail);
        let mut buf = vec![0u8; take as usize];
        {
            let file = self.data_file.as_ref().ok_or(TapeErr::ENOMEDIUM)?;
            file.read_at(&mut buf, self.pos).map_err(TapeErr::from_io)?;
        }
        self.pos += take;
        let records = if self.block_size == 0 {
            1
        } else {
            (take / self.block_size as u64) as i64
        };
        if self.blkno >= 0 {
            self.blkno += records;
        }

        if take < avail {
            // Read exactly what was requested; no FM crossed.
            if self.eof == ST_FM {
                self.eof = ST_NOEOF;
            }
        } else if next_mark.is_some() {
            // Reached the next FM exactly at the end of this read.
            self.eof = ST_FM_HIT;
        } else {
            // Reached end of recorded data.
            if self.eof == ST_FM {
                self.eof = ST_NOEOF;
            }
        }
        Ok(buf)
    }

    pub fn write(&mut self, data: &[u8]) -> TRes<usize> {
        if !self.ready() {
            return Err(TapeErr::ENOMEDIUM);
        }
        if self.effective_write_prot() {
            return Err(TapeErr::EACCES);
        }
        if data.is_empty() {
            return Ok(0);
        }
        if self.block_size != 0 && !data.len().is_multiple_of(self.block_size as usize) {
            return Err(TapeErr::EINVAL);
        }
        if self.pos + data.len() as u64 > self.capacity {
            // End of medium: the drive flags EOD so `mt status` reports it
            // (st.c:1716-1732). A later write may still proceed (next volume).
            self.eof = ST_EOD_1;
            return Err(TapeErr::ENOSPC);
        }

        // A write erases everything from the current position onward.
        self.truncate_at(self.pos)?;
        {
            let file = self.data_file.as_mut().ok_or(TapeErr::ENOMEDIUM)?;
            file.write_at(data, self.pos).map_err(TapeErr::from_io)?;
        }
        let records = if self.block_size == 0 {
            1
        } else {
            (data.len() as u64 / self.block_size as u64) as i64
        };
        if self.blkno >= 0 {
            self.blkno += records;
        }
        self.data_len = self.pos + data.len() as u64;
        self.pos += data.len() as u64;
        self.eof = ST_NOEOF;
        self.rw = ST_WRITING;
        Ok(data.len())
    }

    // ---------------- MTIOCTOP operations ----------------

    pub fn mt_op(&mut self, op: i16, mut count: i32) -> TRes<()> {
        if !self.loaded {
            return Err(TapeErr::ENOMEDIUM);
        }

        // Drive is offline (`mt offline`/`mt eject`). st.c: only `mt load`
        // (MTLOAD) can bring it back (do_load_unload with load_code, st.c:2635);
        // MTOFFL/MTUNLOAD are already offline -> ENOMEDIUM (st.c:2642-2644) and
        // every other op -> ENOMEDIUM (st.c:2767).
        if self.offline {
            return match op {
                MTLOAD => {
                    self.offline = false;
                    self.rewind();
                    Ok(())
                }
                _ => Err(TapeErr::ENOMEDIUM),
            };
        }

        // Automatic WEOF before a position-changing op while still writing.
        if self.rw == ST_WRITING && matches!(op, MTREW | MTOFFL | MTSEEK | MTBSF | MTBSFM) {
            self.wteof(1)?;
            if op == MTBSF || op == MTBSFM {
                count += 1;
            }
            self.rw = ST_IDLE;
        }

        // Cross an FM hit mid-read before spacing (st.c:3726-3737). The mark the
        // head is sitting on counts as the first mark to cross: advance fileno
        // to the file that begins here and consume one of the requested marks.
        if self.eof == ST_FM_HIT {
            if matches!(op, MTFSF | MTFSFM | MTEOM) {
                count -= 1;
                if self.fileno >= 0 {
                    self.fileno += 1;
                }
                // If the requested spacing is fully consumed by the mark we are
                // on, the head stays at that mark (the start of file `fileno`).
                // Settle exactly as st.c does after a successful MTFSF
                // (st.c:3030,3055): at the mark, block 0, eof=ST_FM -- so the
                // next read returns data instead of a phantom 0-byte FM hit.
                if count <= 0 {
                    self.pos = if self.fileno == 0 {
                        0
                    } else {
                        self.marks[(self.fileno - 1) as usize]
                    };
                    self.blkno = 0;
                    self.eof = ST_FM;
                }
            } else if matches!(op, MTBSF | MTBSFM) {
                count += 1;
                if self.fileno >= 0 {
                    self.fileno += 1;
                }
            }
        }

        // Reset rw (prevents further automatic WEOF) for all but these.
        if !matches!(op, MTNOP | MTSETBLK | MTSETDENSITY | MTWSM | MTSETDRVBUFFER | MTSETPART) {
            self.rw = ST_IDLE;
        }

        match op {
            MTFSF => self.pure_fsf(count),
            MTBSF => self.pure_bsf(count),
            MTFSFM => {
                self.pure_fsf(count)?;
                self.pure_bsf(1)
            }
            MTBSFM => {
                self.pure_bsf(count)?;
                self.pure_fsf(1)
            }
            MTFSR => self.pure_fsr(count),
            MTBSR => self.pure_bsr(count),
            MTWEOF | MTWEOFI => self.wteof(count),
            MTWSM => {
                self.wteof(count)?;
                self.at_sm = true;
                Ok(())
            }
            MTREW | MTRETEN => {
                self.rewind();
                Ok(())
            }
            MTNOP => Ok(()),
            MTEOM => self.mt_eom(),
            MTERASE => self.mt_erase(),
            MTSETBLK => {
                self.block_size = (count as u32) & MT_ST_BLKSIZE_MASK;
                Ok(())
            }
            MTSETDENSITY => {
                self.density = ((count as u32) & MT_ST_DENSITY_MASK) >> MT_ST_DENSITY_SHIFT;
                Ok(())
            }
            MTSEEK => self.mt_seek(count),
            MTFSS | MTBSS => {
                if count != 0 {
                    self.fileno = -1;
                    self.blkno = -1;
                    self.at_sm = true;
                    self.eof = ST_NOEOF;
                }
                Ok(())
            }
            MTLOCK | MTUNLOCK => Ok(()),
            MTSETDRVBUFFER => {
                // st.c:3711-3716: MTSETDRVBUFFER (and the MT_ST_* options that
                // ride on it: stoptions/sttimeout/defblksize/defdensity/...)
                // require CAP_SYS_ADMIN; a non-root caller gets EPERM. Root
                // (euid 0) always holds it, so euid is a faithful proxy.
                if unsafe { libc::geteuid() } != 0 {
                    return Err(TapeErr::EPERM);
                }
                // Root: apply the option. The top byte selects the operation
                // (st.c:3049 for the plain drvbuffer; st_set_options for the
                // extended MT_ST_* boolean codes).
                let arg = count as u32;
                match arg & MT_ST_OPTIONS {
                    // Plain `mt drvbuffer <n>` (code 0): set the SCSI
                    // drive-buffering bit (st.c:3049).
                    0 => self.drv_buffer = arg & 7,
                    // `mt stoptions <opts>` (MT_ST_BOOLEANS): set every boolean
                    // from the given bits (st.c:2331-2348).
                    MT_ST_BOOLEANS => {
                        self.do_buffer_writes = (arg & MT_ST_BUFFER_WRITES) != 0;
                        self.do_async_writes = (arg & MT_ST_ASYNC_WRITES) != 0;
                    }
                    // `mt stsetoptions`/`stclearoptions` (MT_ST_SET/CLEARBOOLEANS):
                    // toggle only the bits that are present (st.c:2351-2387).
                    MT_ST_SETBOOLEANS | MT_ST_CLEARBOOLEANS => {
                        let value = (arg & MT_ST_SETBOOLEANS) != 0;
                        if (arg & MT_ST_BUFFER_WRITES) != 0 {
                            self.do_buffer_writes = value;
                        }
                        if (arg & MT_ST_ASYNC_WRITES) != 0 {
                            self.do_async_writes = value;
                        }
                    }
                    // Other extended codes (write threshold, def blksize/density/
                    // drvbuffer, timeouts, set-cln): accepted, but they have no
                    // observable effect on this emulator's status output.
                    _ => {}
                }
                Ok(())
            }
            MTOFFL | MTUNLOAD => {
                // st.c:3891-3892 -> do_load_unload(0): the medium stays in the
                // drive but the drive goes offline. I/O then fails with
                // ENOMEDIUM and MTIOCGET reports GMT_DR_OPEN until `mt load`.
                self.offline = true;
                Ok(())
            }
            MTLOAD => {
                // st.c:3896-3897 -> do_load_unload(load_code): reload the
                // medium and leave it at the beginning of partition 0 (BOT).
                self.rewind();
                Ok(())
            }
            // Compression is accepted as a no-op (st.c:3922 -> st_compression);
            // this emulator performs no actual compression.
            MTCOMPRESSION => Ok(()),
            MTSETPART | MTMKPART => Err(TapeErr::EINVAL),
            _ => Err(TapeErr::ENOSYS),
        }
    }

    fn pure_fsf(&mut self, n: i32) -> TRes<()> {
        if n <= 0 {
            return Ok(());
        }
        let target = self.fileno + n as i64;
        if target < 0 {
            return Err(TapeErr::EIO);
        }
        if target as usize > self.marks.len() {
            // Spacing past the last file mark -> end of data.
            self.pos = self.data_len;
            self.fileno = self.marks.len() as i64;
            self.blkno = 0;
            self.eof = ST_EOD;
            return Ok(());
        }
        let start = if target == 0 {
            0
        } else {
            self.marks[(target - 1) as usize]
        };
        self.pos = start;
        self.fileno = target;
        self.blkno = 0;
        self.eof = ST_FM;
        Ok(())
    }

    fn pure_bsf(&mut self, n: i32) -> TRes<()> {
        if n <= 0 {
            return Ok(());
        }
        let target = self.fileno - n as i64;
        if target < 0 {
            return Err(TapeErr::EIO);
        }
        let start = if target == 0 {
            0
        } else {
            self.marks[(target - 1) as usize]
        };
        self.pos = start;
        self.fileno = target;
        self.blkno = -1;
        self.eof = ST_NOEOF;
        Ok(())
    }

    fn pure_fsr(&mut self, n: i32) -> TRes<()> {
        if n <= 0 {
            return Ok(());
        }
        if self.block_size != 0 {
            let target = self.pos + n as u64 * self.block_size as u64;
            self.pos = std::cmp::min(target, self.data_len);
        }
        if self.blkno >= 0 {
            self.blkno += n as i64;
        }
        self.recompute_fileno();
        self.eof = ST_NOEOF;
        Ok(())
    }

    fn pure_bsr(&mut self, n: i32) -> TRes<()> {
        if n <= 0 {
            return Ok(());
        }
        if self.block_size != 0 {
            let back = n as u64 * self.block_size as u64;
            self.pos = self.pos.saturating_sub(back);
        }
        if self.blkno >= 0 {
            self.blkno -= n as i64;
        }
        self.recompute_fileno();
        self.eof = ST_NOEOF;
        Ok(())
    }

    fn recompute_fileno(&mut self) {
        // fileno = number of file marks at or before the head: pos sitting
        // exactly on mark k means the head is at the start of file k.
        self.fileno = self.marks.partition_point(|&m| m <= self.pos) as i64;
    }

    pub fn wteof(&mut self, n: i32) -> TRes<()> {
        if !self.ready() {
            return Err(TapeErr::ENOMEDIUM);
        }
        if self.effective_write_prot() {
            return Err(TapeErr::EACCES);
        }
        if n <= 0 {
            return Ok(());
        }
        self.truncate_at(self.pos)?;
        for _ in 0..n {
            self.marks.push(self.pos);
        }
        self.marks.sort_unstable();
        self.fileno = self.marks.len() as i64;
        self.blkno = 0;
        self.eof = ST_NOEOF;
        self.rw = ST_IDLE;
        Ok(())
    }

    fn mt_eom(&mut self) -> TRes<()> {
        if !self.ready() {
            return Err(TapeErr::ENOMEDIUM);
        }
        self.pos = self.data_len;
        self.fileno = self.marks.len() as i64;
        self.blkno = -1;
        self.eof = ST_EOD;
        Ok(())
    }

    fn mt_erase(&mut self) -> TRes<()> {
        if !self.ready() {
            return Err(TapeErr::ENOMEDIUM);
        }
        if self.effective_write_prot() {
            return Err(TapeErr::EACCES);
        }
        if let Some(file) = self.data_file.as_mut() {
            file.set_len(0).map_err(TapeErr::from_io)?;
        }
        self.data_len = 0;
        self.marks.clear();
        self.pos = 0;
        self.fileno = 0;
        self.blkno = 0;
        self.eof = ST_NOEOF;
        self.at_sm = false;
        self.rw = ST_IDLE;
        Ok(())
    }

    fn mt_seek(&mut self, block: i32) -> TRes<()> {
        if !self.ready() {
            return Err(TapeErr::ENOMEDIUM);
        }
        if block < 0 {
            return Err(TapeErr::EIO);
        }
        let unit = if self.block_size == 0 { 512u64 } else { self.block_size as u64 };
        let target = block as u64 * unit;
        if target > self.data_len {
            self.pos = self.data_len;
            self.fileno = -1;
            self.blkno = -1;
            self.eof = ST_EOD;
            return Ok(());
        }
        self.pos = target;
        self.recompute_fileno();
        self.blkno = -1;
        self.eof = ST_NOEOF;
        Ok(())
    }

    // ---------------- status / position reporting ----------------

    // Build the 48-byte little-endian `struct mtget` payload:
    // five `long` (8B) fields followed by two `__kernel_daddr_t` (4B) fields.
    pub fn mtget_bytes(&self) -> [u8; 48] {
        let mt_type: i64 = MT_ISSCSI2 as i64;
        let mt_resid: i64 = 0; // partition
        let mt_dsreg: i64 = ((self.block_size & MT_ST_BLKSIZE_MASK) as i64)
            | (((self.density & 0xff) as i64) << MT_ST_DENSITY_SHIFT as i64);

        let mut mt_gstat: i64 = 0;
        if self.cartridge_write_prot {
            mt_gstat |= GMT_WR_PROT as i64;
        }
        if self.blkno == 0 {
            if self.fileno == 0 {
                mt_gstat |= GMT_BOT as i64;
            } else {
                mt_gstat |= GMT_EOF as i64;
            }
        }
        if self.eof == ST_EOM_OK || self.eof == ST_EOM_ERROR {
            mt_gstat |= GMT_EOT as i64;
        } else if self.eof >= ST_EOM_OK {
            mt_gstat |= GMT_EOD as i64;
        }
        match self.density {
            1 => mt_gstat |= GMT_D_800 as i64,
            2 => mt_gstat |= GMT_D_1600 as i64,
            3 => mt_gstat |= GMT_D_6250 as i64,
            _ => {}
        }
        // st.c:4014-4017: GMT_ONLINE only when the drive is ready
        // (medium loaded AND not offline); GMT_DR_OPEN otherwise.
        if self.loaded && !self.offline {
            mt_gstat |= GMT_ONLINE as i64;
        } else {
            mt_gstat |= GMT_DR_OPEN as i64;
        }
        if self.at_sm {
            mt_gstat |= GMT_SM as i64;
        }
        // st.c:4020-4023: GMT_IM_REP_EN when the drive does async writes, or
        // buffered fixed-block writes, or has drive buffering enabled. With the
        // default options (do_async_writes=1, drv_buffer=1) a fresh drive always
        // sets it -- matching what a real st-driven SCSI tape reports.
        if self.do_async_writes
            || (self.do_buffer_writes && self.block_size != 0)
            || self.drv_buffer != 0
        {
            mt_gstat |= GMT_IM_REP_EN as i64;
        }

        let mt_erreg: i64 = 0;

        let mut out = [0u8; 48];
        out[0..8].copy_from_slice(&mt_type.to_le_bytes());
        out[8..16].copy_from_slice(&mt_resid.to_le_bytes());
        out[16..24].copy_from_slice(&mt_dsreg.to_le_bytes());
        out[24..32].copy_from_slice(&mt_gstat.to_le_bytes());
        out[32..40].copy_from_slice(&mt_erreg.to_le_bytes());
        out[40..44].copy_from_slice(&(self.fileno as i32).to_le_bytes());
        out[44..48].copy_from_slice(&(self.blkno as i32).to_le_bytes());
        out
    }

    // Build the 8-byte little-endian `struct mtpos` payload (absolute block).
    pub fn mtpos_bytes(&self) -> [u8; 8] {
        let unit = if self.block_size == 0 { 512u64 } else { self.block_size as u64 };
        ((self.pos / unit) as i64).to_le_bytes()
    }

    // Human-readable summary for the CLI / control `status` command.
    pub fn summary(&self) -> String {
        let label = self.label.clone().unwrap_or_else(|| "(none)".into());
        format!(
            "label={} loaded={} fileno={} blkno={} pos={} datalen={} marks={} \
             block_size={} density={} rw={} eof={} write_prot={} at_sm={}",
            label,
            self.loaded,
            self.fileno,
            self.blkno,
            self.pos,
            self.data_len,
            self.marks.len(),
            self.block_size,
            self.density,
            self.rw,
            self.eof,
            self.effective_write_prot(),
            self.at_sm
        )
    }
}

// ===================== tests =====================

#[cfg(test)]
mod tests {
    use super::*;

    // A loaded tape backed by a temp file. The TempDir keeps the file alive.
    // NOTE: must be opened O_RDWR — pread/pwrite on an O_WRONLY fd fails EBADF.
    fn tape(capacity: u64) -> (TapeState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(dir.path().join("t.tap"))
            .unwrap();
        let mut st = TapeState::offline(capacity);
        st.label = Some("t".into());
        st.loaded = true;
        st.data_file = Some(file);
        (st, dir)
    }

    fn gstat(st: &TapeState) -> i64 {
        let b = st.mtget_bytes();
        i64::from_le_bytes(b[24..32].try_into().unwrap())
    }

    #[test]
    fn write_advances_and_records() {
        let (mut st, _d) = tape(1 << 20);
        assert_eq!(st.write(b"AAA").unwrap(), 3);
        assert_eq!(st.pos, 3);
        assert_eq!(st.blkno, 1);
        assert_eq!(st.rw, ST_WRITING);
        assert_eq!(st.data_len, 3);
        assert_eq!(st.mt_op(MTWEOF, 1).unwrap(), ());
        assert_eq!(st.fileno, 1);
        assert_eq!(st.marks, vec![3]);
        assert_eq!(st.blkno, 0);
    }

    #[test]
    fn read_across_fm_is_two_step() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, true).unwrap();
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.write(b"BBB").unwrap();
        st.flush().unwrap();
        st.release().unwrap();
        assert_eq!(st.pos, 0);

        let r = st.read(3).unwrap();
        assert_eq!(r, b"AAA");
        assert_eq!(st.eof, ST_FM_HIT);

        let r = st.read(3).unwrap();
        assert!(r.is_empty());
        assert_eq!(st.eof, ST_FM);
        assert_eq!(st.fileno, 1);
        assert_eq!(st.blkno, 0);

        // release() auto-WEOFed, so BBB ends exactly on the second FM.
        let r = st.read(3).unwrap();
        assert_eq!(r, b"BBB");
        assert_eq!(st.eof, ST_FM_HIT);
        assert_eq!(st.fileno, 1);
        assert_eq!(st.blkno, 1);

        let r = st.read(3).unwrap();
        assert!(r.is_empty());
        assert_eq!(st.eof, ST_FM);
        assert_eq!(st.fileno, 2);
        assert_eq!(st.blkno, 0);
    }

    #[test]
    fn partial_record_read() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, true).unwrap();
        st.write(b"AAA").unwrap();
        st.flush().unwrap();
        st.release().unwrap();
        assert_eq!(st.read(2).unwrap(), b"AA");
        assert_eq!(st.pos, 2);
        assert_eq!(st.read(1).unwrap(), b"A");
        assert_eq!(st.pos, 3);
        assert!(st.read(1).unwrap().is_empty());
    }

    #[test]
    fn write_truncates_from_pos() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.write(b"BBBB").unwrap();
        assert_eq!(st.data_len, 8);
        st.mt_op(MTREW, 0).unwrap();
        assert_eq!(st.fileno, 0);
        assert_eq!(st.pos, 0);
        st.write(b"CC").unwrap();
        assert_eq!(st.data_len, 2);
        assert!(st.marks.is_empty());
        st.mt_op(MTREW, 0).unwrap();
        assert_eq!(st.read(2).unwrap(), b"CC");
    }

    #[test]
    fn fixed_block_size_enforced() {
        let (mut st, _d) = tape(1 << 20);
        st.mt_op(MTSETBLK, 512).unwrap();
        assert_eq!(st.block_size, 512);
        assert_eq!(st.write(&[0u8; 513]).unwrap_err(), TapeErr::EINVAL);
        assert_eq!(st.write(&[0u8; 512]).unwrap(), 512);
        st.mt_op(MTREW, 0).unwrap();
        assert_eq!(st.read(513).unwrap_err(), TapeErr::EINVAL);
        assert_eq!(st.read(512).unwrap().len(), 512);
    }

    #[test]
    fn capacity_enospc() {
        let (mut st, _d) = tape(10);
        assert_eq!(st.write(&[0u8; 11]).unwrap_err(), TapeErr::ENOSPC);
        assert_eq!(st.write(&[0u8; 10]).unwrap(), 10);
        assert_eq!(st.write(&[0u8; 1]).unwrap_err(), TapeErr::ENOSPC);
    }

    // An ENOSPC write flags end-of-data so `mt status` reports EOD
    // (st.c:1716-1732 sets ST_EOD_1). A later in-capacity write clears it.
    #[test]
    fn enospc_write_sets_eod_status() {
        let (mut st, _d) = tape(10);
        assert_eq!(st.write(&[0u8; 11]).unwrap_err(), TapeErr::ENOSPC);
        assert_eq!(st.eof, ST_EOD_1);
        // mt_gstat: EOF/EOD region -> GMT_EOD (eof >= ST_EOM_OK).
        let g = gstat(&st);
        assert_ne!(g & GMT_EOD as i64, 0);

        // A write that fits still proceeds and clears the EOD flag.
        assert_eq!(st.write(&[0u8; 10]).unwrap(), 10);
        assert_eq!(st.eof, ST_NOEOF);
        assert_eq!(gstat(&st) & GMT_EOD as i64, 0);
    }

    // Reading past the last filemark settles to EOD and then fails with EIO,
    // exactly like blank-tape reads on a real drive (st.c:2088-2096,2176-2184):
    // one 0-byte read per EOD_1/EOD_2 step, then -EIO at ST_EOD.
    #[test]
    fn read_past_last_mark_is_eio() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.mt_op(MTREW, 0).unwrap();

        assert_eq!(st.read(3).unwrap(), b"AAA");
        assert_eq!(st.eof, ST_FM_HIT);
        // Cross the (only) mark: 0 bytes, settle to ST_FM.
        assert!(st.read(3).unwrap().is_empty());
        assert_eq!(st.eof, ST_FM);
        // Past the last mark: 0 bytes, settle to ST_EOD.
        assert!(st.read(3).unwrap().is_empty());
        assert_eq!(st.eof, ST_EOD);
        // Further reads are blank tape -> -EIO.
        assert_eq!(st.read(3).unwrap_err(), TapeErr::EIO);
        assert_eq!(st.read(3).unwrap_err(), TapeErr::EIO);
    }

    // MTCOMPRESSION is accepted as a no-op rather than rejected (st.c:3922).
    #[test]
    fn compression_op_is_noop() {
        let (mut st, _d) = tape(1 << 20);
        assert_eq!(st.mt_op(MTCOMPRESSION, 1).unwrap(), ());
        assert_eq!(st.mt_op(MTCOMPRESSION, 0).unwrap(), ());
    }

    #[test]
    fn write_protected() {
        let (mut st, _d) = tape(1 << 20);
        st.cartridge_write_prot = true;
        assert_eq!(st.write(b"A").unwrap_err(), TapeErr::EACCES);
        assert_eq!(st.mt_op(MTWEOF, 1).unwrap_err(), TapeErr::EACCES);
    }

    #[test]
    fn open_release_semantics() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, true).unwrap();
        assert_eq!(st.open(libc::O_RDWR, false, true).unwrap_err(), TapeErr::EBUSY);
        st.write(b"AAA").unwrap();
        st.flush().unwrap();
        st.release().unwrap();
        // automatic WEOF at close, then rewind
        assert_eq!(st.marks, vec![3]);
        assert_eq!(st.pos, 0);
        assert!(!st.in_use);
        assert!(!st.readonly);

        let mut off = TapeState::offline(100);
        assert_eq!(off.open(libc::O_RDWR, false, true).unwrap_err(), TapeErr::ENOMEDIUM);
        off.open(libc::O_RDWR, true, true).unwrap();
        assert!(off.in_use);

        let (mut rd, _d2) = tape(1 << 20);
        rd.open(libc::O_RDONLY, false, true).unwrap();
        assert!(rd.readonly);
        assert_eq!(rd.write(b"A").unwrap_err(), TapeErr::EACCES);
    }

    // /dev/st0: close rewinds; /dev/nst0: close does not. Both auto-WEOF.
    #[test]
    fn st_close_rewinds_nst_close_does_not() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, true).unwrap();
        st.write(b"AAA").unwrap();
        st.flush().unwrap();
        st.release().unwrap();
        assert_eq!(st.pos, 0); // st: rewound
        assert_eq!(st.marks, vec![3]); // auto WEOF still happened

        let (mut nst, _d2) = tape(1 << 20);
        nst.open(libc::O_RDWR, false, false).unwrap();
        nst.write(b"AAA").unwrap();
        nst.flush().unwrap();
        nst.release().unwrap();
        assert_eq!(nst.pos, 3); // nst: head stays put
        assert_eq!(nst.marks, vec![3]); // auto WEOF still happened
        assert_eq!(nst.fileno, 1);
        assert_eq!(nst.blkno, 0);
    }

    // After an nst close the auto-WEOF leaves the head AT the filemark
    // (st.c:1442). Reopening and reading the blank tape past it must settle
    // ST_FM -> ST_EOD and then fail with -EIO, not return 0 bytes forever.
    #[test]
    fn nst_read_after_close_sees_eod() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap(); // nst
        st.write(b"AAA").unwrap();
        st.flush().unwrap();
        st.release().unwrap(); // no rewind: head stays at the mark
        assert_eq!(st.pos, 3);
        assert_eq!(st.eof, ST_FM);

        st.open(libc::O_RDONLY, false, false).unwrap();
        assert!(st.read(3).unwrap().is_empty());
        assert_eq!(st.eof, ST_EOD);
        assert_eq!(st.read(3).unwrap_err(), TapeErr::EIO);
    }

    // FUSE sends RELEASE once per open, after the last FLUSH; the release
    // work must happen exactly once.
    #[test]
    fn release_is_idempotent() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        st.write(b"AAA").unwrap();
        st.flush().unwrap(); // auto WEOF
        assert_eq!(st.marks, vec![3]);
        st.release().unwrap();
        assert_eq!(st.pos, 3); // nst: no rewind
        assert!(!st.in_use);
        st.release().unwrap(); // a second RELEASE is a no-op
        assert_eq!(st.marks, vec![3]);
        assert_eq!(st.pos, 3);
    }

    // The exact syscall pattern GNU dd produces: open, dup the fd, close the
    // original (FLUSH before any write), write through the dup, close the dup
    // (FLUSH with data pending). The auto WEOF must land after the data, and
    // the drive must stay in use until the final close.
    #[test]
    fn dd_dup_flush_pattern() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        st.flush().unwrap(); // close of the original fd, before any write
        assert_eq!(st.marks.len(), 0); // nothing written yet: no WEOF
        assert!(st.in_use); // the dup keeps the drive busy
        st.write(b"AAA").unwrap();
        st.flush().unwrap(); // close of the dup: auto WEOF after the data
        assert_eq!(st.marks, vec![3]);
        st.release().unwrap(); // last reference: release (no rewind, nst)
        assert!(!st.in_use);
        assert_eq!(st.pos, 3);
    }

    // The classic nst workflow: two archives appended on one tape, each close
    // auto-WEOF separates them, then navigate back and read each.
    #[test]
    fn nst_multi_archive_append_and_navigate() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        st.write(b"AAA").unwrap();
        st.flush().unwrap();
        st.release().unwrap(); // WEOF at 3, pos stays 3

        st.open(libc::O_RDWR, false, false).unwrap();
        assert_eq!(st.pos, 3); // reopened where we left off
        st.write(b"BBB").unwrap();
        st.flush().unwrap();
        st.release().unwrap(); // WEOF at 6
        assert_eq!(st.marks, vec![3, 6]);
        assert_eq!(st.pos, 6);

        // Navigate back to file 1 and read BBB.
        st.mt_op(MTBSF, 1).unwrap();
        assert_eq!(st.fileno, 1);
        assert_eq!(st.pos, 3);
        assert_eq!(st.blkno, -1);
        assert_eq!(st.read(3).unwrap(), b"BBB");

        // Back to file 0 and read AAA.
        st.mt_op(MTBSF, 1).unwrap();
        assert_eq!(st.fileno, 0);
        assert_eq!(st.pos, 0);
        assert_eq!(st.read(3).unwrap(), b"AAA");
    }

    // General-N: k archives on one tape produce EXACTLY k filemarks at the
    // cumulative byte offsets. Every file must be reachable by forward (fsf)
    // AND backward (bsf) navigation and read back byte-identical. This is the
    // test that would catch an off-by-one that only manifests at mark 3+.
    #[test]
    fn multi_archive_n_files_bitperfect() {
        let files: Vec<Vec<u8>> = vec![
            b"AAAA".to_vec(),
            b"BBBBB".to_vec(),
            b"CCCC".to_vec(),
            b"DDDDDD".to_vec(),
            b"E".to_vec(),
        ];
        let (mut st, _d) = tape(1 << 20);

        let mut expected_marks: Vec<u64> = Vec::new();
        let mut off: u64 = 0;
        for f in &files {
            st.open(libc::O_RDWR, false, false).unwrap();
            st.write(f).unwrap();
            st.flush().unwrap();
            st.release().unwrap(); // auto WEOF separates each archive
            off += f.len() as u64;
            expected_marks.push(off);
        }
        // Exactly one filemark per archive, at the cumulative offsets.
        assert_eq!(st.marks, expected_marks);
        assert_eq!(st.data_len, off);

        // Forward: rewind + fsf(i) lands exactly on file i and reads it back.
        for (i, f) in files.iter().enumerate() {
            st.mt_op(MTREW, 0).unwrap();
            st.mt_op(MTFSF, i as i32).unwrap();
            assert_eq!(st.fileno, i as i64);
            assert_eq!(st.pos, if i == 0 { 0 } else { expected_marks[i - 1] });
            assert_eq!(st.read(f.len()).unwrap(), *f);
        }

        // Backward: space to the last file, then bsf(1) back through every
        // file. Reading a file leaves the head on its trailing filemark, so
        // the next bsf(1) must land on the previous file (at-mark adjust).
        st.mt_op(MTREW, 0).unwrap();
        st.mt_op(MTFSF, (files.len() - 1) as i32).unwrap();
        for i in (0..files.len()).rev() {
            assert_eq!(st.fileno, i as i64);
            assert_eq!(st.pos, if i == 0 { 0 } else { expected_marks[i - 1] });
            assert_eq!(st.read(files[i].len()).unwrap(), files[i]);
            if i > 0 {
                st.mt_op(MTBSF, 1).unwrap();
            }
        }
    }

    #[test]
    fn fileno_at_mark_positions() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.write(b"BBB").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        // marks = [3, 6]; pos exactly on the middle mark => start of file 1.
        st.pos = 3;
        st.recompute_fileno();
        assert_eq!(st.fileno, 1);
        st.pos = 4;
        st.recompute_fileno();
        assert_eq!(st.fileno, 1);
        st.pos = 6;
        st.recompute_fileno();
        assert_eq!(st.fileno, 2);
        st.pos = 0;
        st.recompute_fileno();
        assert_eq!(st.fileno, 0);
    }

    #[test]
    fn fsf_bsf_spacing() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.write(b"BBB").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.mt_op(MTREW, 0).unwrap();
        assert_eq!(st.pos, 0);

        st.mt_op(MTFSF, 2).unwrap();
        assert_eq!(st.fileno, 2);
        assert_eq!(st.pos, 6);
        assert_eq!(st.eof, ST_FM);

        st.mt_op(MTBSF, 1).unwrap();
        assert_eq!(st.fileno, 1);
        assert_eq!(st.pos, 3);
        assert_eq!(st.blkno, -1);

        st.mt_op(MTFSF, 5).unwrap();
        assert_eq!(st.eof, ST_EOD);
        assert_eq!(st.fileno, 2);
        assert_eq!(st.pos, 6); // EOD: past the last mark
    }

    // st.c:2974-2977: MTFSFM(n) = FSF(n) then BSF(1); MTBSFM(n) = BSF(n) then FSF(1).
    #[test]
    fn fsfm_bsfm_match_st() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.write(b"BBB").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.mt_op(MTREW, 0).unwrap();

        st.mt_op(MTFSFM, 2).unwrap(); // FSF(2) -> file 2, BSF(1) -> BOT of file 1
        assert_eq!(st.fileno, 1);
        assert_eq!(st.pos, 3);
        assert_eq!(st.blkno, -1);
        assert_eq!(st.eof, ST_NOEOF);

        st.mt_op(MTBSFM, 1).unwrap(); // BSF(1) -> file 0, FSF(1) -> BOT of file 1
        assert_eq!(st.fileno, 1);
        assert_eq!(st.pos, 3);
        assert_eq!(st.blkno, 0);
        assert_eq!(st.eof, ST_FM);

        // BSF past BOT fails, as on a real drive.
        st.mt_op(MTREW, 0).unwrap();
        assert_eq!(st.mt_op(MTBSFM, 1).unwrap_err(), TapeErr::EIO);
    }

    // Reading a file to its mark then spacing forward must behave exactly like
    // a clean rewind+fsf: no phantom 0-byte read, consistent fileno/pos/eof.
    // This pins the st.c:3726-3737 + st.c:3055 settle. The old code left
    // eof=ST_FM_HIT when the adjusted count hit 0, so the next read returned a
    // spurious empty record and bumped fileno a second time.
    #[test]
    fn fsf1_from_just_hit_mark_is_clean() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap();      // file0
        st.mt_op(MTWEOF, 1).unwrap();   // mark[0]=3
        st.write(b"BBB").unwrap();      // file1
        st.mt_op(MTWEOF, 1).unwrap();   // mark[1]=6
        st.mt_op(MTREW, 0).unwrap();    // head to BOT

        // Read file0 to its mark: head sits ON mark[0], eof=ST_FM_HIT, fileno=0.
        assert_eq!(st.read(3).unwrap(), b"AAA");
        assert_eq!(st.eof, ST_FM_HIT);
        assert_eq!(st.fileno, 0);
        assert_eq!(st.pos, 3);

        // fsf 1 from the just-hit mark lands cleanly at the start of file1.
        st.mt_op(MTFSF, 1).unwrap();
        assert_eq!(st.fileno, 1);
        assert_eq!(st.pos, 3);
        assert_eq!(st.blkno, 0);
        assert_eq!(st.eof, ST_FM);

        // The very next read returns file1's data -- no phantom empty read.
        assert_eq!(st.read(3).unwrap(), b"BBB");
    }

    // fsf N (N>=2) from a just-hit mark still spaces N files from the original
    // fileno (st.c: drv_file = orig_fileno + N), and the next read is clean.
    #[test]
    fn fsf2_from_just_hit_mark_spaces_two() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap();      // file0
        st.mt_op(MTWEOF, 1).unwrap();   // mark[0]=3
        st.write(b"BBB").unwrap();      // file1
        st.mt_op(MTWEOF, 1).unwrap();   // mark[1]=6
        st.write(b"CCC").unwrap();      // file2
        st.mt_op(MTWEOF, 1).unwrap();   // mark[2]=9
        st.mt_op(MTREW, 0).unwrap();    // head to BOT

        assert_eq!(st.read(3).unwrap(), b"AAA");
        assert_eq!(st.eof, ST_FM_HIT);
        assert_eq!(st.fileno, 0);

        // fsf 2 from the just-hit mark -> start of file2 (orig_fileno + 2).
        st.mt_op(MTFSF, 2).unwrap();
        assert_eq!(st.fileno, 2);
        assert_eq!(st.pos, 6);
        assert_eq!(st.blkno, 0);
        assert_eq!(st.eof, ST_FM);
        assert_eq!(st.read(3).unwrap(), b"CCC");
    }

    // fsf 1 from a just-hit mark must equal rewind;fsf 1 (same physical target),
    // so the two navigation styles are interchangeable on a real drive.
    #[test]
    fn fsf_from_mark_matches_clean_fsf() {
        let (mut st, _d) = tape(1 << 20);
        for _ in 0..3 {
            st.write(b"XXX").unwrap();
            st.mt_op(MTWEOF, 1).unwrap();
        }
        // Clean path: rewind then fsf 2 -> start of file2.
        st.mt_op(MTREW, 0).unwrap();
        st.mt_op(MTFSF, 2).unwrap();
        let clean = (st.fileno, st.pos, st.eof);
        assert_eq!(clean, (2, 6, ST_FM));

        // At-mark path: rewind, read file0 to its mark, fsf 1 -> also file2?
        // No: from mark[0] (orig_fileno 0), fsf 2 -> file2. Match the clean target.
        st.mt_op(MTREW, 0).unwrap();
        assert_eq!(st.read(3).unwrap(), b"XXX"); // file0 -> at mark[0]
        assert_eq!(st.eof, ST_FM_HIT);
        st.mt_op(MTFSF, 2).unwrap();
        assert_eq!((st.fileno, st.pos, st.eof), clean);
        assert_eq!(st.read(3).unwrap(), b"XXX"); // file2 data, clean
    }

    #[test]
    fn erase_clears() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.mt_op(MTERASE, 0).unwrap();
        assert_eq!(st.data_len, 0);
        assert!(st.marks.is_empty());
        assert_eq!(st.fileno, 0);
        assert_eq!(st.pos, 0);
    }

    #[test]
    fn mtget_payload_layout() {
        let (mut st, _d) = tape(1 << 20);
        let b = st.mtget_bytes();
        assert_eq!(b.len(), 48);
        // mt_type at offset 0 (long)
        assert_eq!(
            i64::from_le_bytes(b[0..8].try_into().unwrap()),
            MT_ISSCSI2 as i64
        );
        // GMT_ONLINE + GMT_BOT at fresh BOT
        let g = gstat(&st);
        assert_ne!(g & GMT_ONLINE as i64, 0);
        assert_ne!(g & GMT_BOT as i64, 0);
        assert_eq!(g & GMT_EOF as i64, 0);

        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        let b = st.mtget_bytes();
        let g = i64::from_le_bytes(b[24..32].try_into().unwrap());
        assert_ne!(g & GMT_EOF as i64, 0);
        assert_eq!(g & GMT_BOT as i64, 0);
        // fileno at 40 (4B), blkno at 44 (4B)
        assert_eq!(i32::from_le_bytes([b[40], b[41], b[42], b[43]]), 1);
        assert_eq!(i32::from_le_bytes([b[44], b[45], b[46], b[47]]), 0);
    }

    #[test]
    fn mtget_offline() {
        let st = TapeState::offline(100);
        let b = st.mtget_bytes();
        let g = i64::from_le_bytes(b[24..32].try_into().unwrap());
        assert_eq!(g & GMT_ONLINE as i64, 0);
        assert_ne!(g & GMT_DR_OPEN as i64, 0);
    }

    // `mt offline` takes the drive offline (medium stays in the drive): I/O and
    // mt ops return ENOMEDIUM and MTIOCGET reports DR_OPEN; `mt load` restores
    // the medium at BOT. Mirrors st.c do_load_unload (st.c:2635/3891-3897).
    #[test]
    fn mt_offline_goes_offline_and_load_restores() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap(); // terminate the file (rw -> IDLE)

        st.mt_op(MTOFFL, 0).unwrap();
        assert!(st.offline);
        // I/O and other mt ops fail with ENOMEDIUM while offline (st.c:1524,2767).
        assert_eq!(st.read(3).unwrap_err(), TapeErr::ENOMEDIUM);
        assert_eq!(st.write(b"X").unwrap_err(), TapeErr::ENOMEDIUM);
        assert_eq!(st.mt_op(MTREW, 0).unwrap_err(), TapeErr::ENOMEDIUM);
        // `mt offline` when already offline -> ENOMEDIUM (st.c:2642-2644).
        assert_eq!(st.mt_op(MTOFFL, 0).unwrap_err(), TapeErr::ENOMEDIUM);
        // MTIOCGET reports DR_OPEN, not ONLINE (st.c:4014-4017).
        let g = gstat(&st);
        assert_eq!(g & GMT_ONLINE as i64, 0);
        assert_ne!(g & GMT_DR_OPEN as i64, 0);

        st.mt_op(MTLOAD, 1).unwrap(); // medium reloaded at BOT
        assert!(!st.offline);
        assert_eq!(st.pos, 0);
        assert_eq!(st.fileno, 0);
        let g = gstat(&st);
        assert_ne!(g & GMT_ONLINE as i64, 0);
        assert_eq!(g & GMT_DR_OPEN as i64, 0);
        // I/O works again and the earlier data is intact.
        assert_eq!(st.read(3).unwrap(), b"AAA".to_vec());
    }

    #[test]
    fn mt_unload_offline_and_load() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        st.mt_op(MTUNLOAD, 0).unwrap();
        assert!(st.offline);
        st.mt_op(MTLOAD, 1).unwrap();
        assert!(!st.offline);
        assert_eq!(st.pos, 0);
    }

    #[test]
    fn mt_load_when_online_rewinds_to_bot() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        st.write(b"AAA").unwrap();
        st.mt_op(MTWEOF, 1).unwrap();
        st.mt_op(MTLOAD, 1).unwrap(); // reload while online -> BOT
        assert_eq!(st.pos, 0);
        assert_eq!(st.fileno, 0);
        assert!(!st.offline);
    }

    // st.c:3711-3716: MTSETDRVBUFFER (and the MT_ST_* options riding on it)
    // require CAP_SYS_ADMIN; a non-root caller gets EPERM. euid 0 is the proxy.
    #[test]
    fn setdrvbuffer_requires_root() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        let res = st.mt_op(MTSETDRVBUFFER, 0);
        if unsafe { libc::geteuid() } == 0 {
            res.unwrap();
        } else {
            assert_eq!(res.unwrap_err(), TapeErr::EPERM);
        }
    }

    // st.c:4020-4023: GMT_IM_REP_EN reflects the drive's write-buffering mode.
    // With the default options (do_async_writes=1, drv_buffer=1) a fresh drive
    // always reports it, even at block_size 0 -- matching real hardware.
    #[test]
    fn im_rep_en_set_by_default() {
        let (st, _d) = tape(1 << 20);
        assert_ne!(gstat(&st) & GMT_IM_REP_EN as i64, 0);
    }

    // GMT_IM_REP_EN = do_async_writes || (do_buffer_writes && block_size!=0)
    // || drv_buffer!=0. Exercise each term directly (white-box, euid-independent).
    #[test]
    fn im_rep_en_formula() {
        let (mut st, _d) = tape(1 << 20);
        // All terms off -> cleared.
        st.do_async_writes = false;
        st.do_buffer_writes = false;
        st.drv_buffer = 0;
        st.block_size = 0;
        assert_eq!(gstat(&st) & GMT_IM_REP_EN as i64, 0);
        // drv_buffer alone -> set.
        st.drv_buffer = 1;
        assert_ne!(gstat(&st) & GMT_IM_REP_EN as i64, 0);
        // do_async_writes alone -> set.
        st.drv_buffer = 0;
        st.do_async_writes = true;
        assert_ne!(gstat(&st) & GMT_IM_REP_EN as i64, 0);
        // do_buffer_writes only matters with a fixed block size.
        st.do_async_writes = false;
        st.do_buffer_writes = true;
        st.block_size = 0; // buffered but variable-length -> not the trigger
        assert_eq!(gstat(&st) & GMT_IM_REP_EN as i64, 0);
        st.block_size = 512; // buffered fixed-block -> set
        assert_ne!(gstat(&st) & GMT_IM_REP_EN as i64, 0);
    }

    // st.c:3049: `mt drvbuffer <n>` (plain, code 0) sets drv_buffer = n & 7.
    // st.c:2331-2387: the MT_ST_* boolean ops toggle do_buffer_writes /
    // do_async_writes. All gated on CAP_SYS_ADMIN (euid 0 proxy); skip the state
    // assertions when running unprivileged.
    #[test]
    fn setdrvbuffer_and_stoptions_update_buffering_state() {
        let (mut st, _d) = tape(1 << 20);
        st.open(libc::O_RDWR, false, false).unwrap();
        if unsafe { libc::geteuid() } != 0 {
            return; // non-root cannot exercise the mutation (would get EPERM)
        }
        // Plain drvbuffer: sets the SCSI drive-buffering bit (st.c:3049).
        st.mt_op(MTSETDRVBUFFER, 0).unwrap();
        assert_eq!(st.drv_buffer, 0);
        st.mt_op(MTSETDRVBUFFER, 3).unwrap();
        assert_eq!(st.drv_buffer, 3);
        // stclearoptions: clear do_async_writes (MT_ST_ASYNC_WRITES bit only).
        st.mt_op(MTSETDRVBUFFER, (MT_ST_CLEARBOOLEANS | MT_ST_ASYNC_WRITES) as i32)
            .unwrap();
        assert!(!st.do_async_writes);
        assert!(st.do_buffer_writes); // untouched by that clear
        // stsetoptions: set do_async_writes back on.
        st.mt_op(MTSETDRVBUFFER, (MT_ST_SETBOOLEANS | MT_ST_ASYNC_WRITES) as i32)
            .unwrap();
        assert!(st.do_async_writes);
        // stoptions (MT_ST_BOOLEANS): set every boolean from the bits; giving
        // only BUFFER_WRITES forces async off and buffer-writes on.
        st.mt_op(MTSETDRVBUFFER, (MT_ST_BOOLEANS | MT_ST_BUFFER_WRITES) as i32)
            .unwrap();
        assert!(st.do_buffer_writes);
        assert!(!st.do_async_writes);
    }

    #[test]
    fn mtpos_absolute_blocks() {
        let (mut st, _d) = tape(1 << 20);
        st.write(&[0u8; 1024]).unwrap();
        assert_eq!(st.mtpos_bytes(), 2i64.to_le_bytes()); // block_size 0 -> /512
        st.mt_op(MTSETBLK, 512).unwrap();
        assert_eq!(st.mtpos_bytes(), 2i64.to_le_bytes());
        st.mt_op(MTREW, 0).unwrap();
        assert_eq!(st.mtpos_bytes(), 0i64.to_le_bytes());
    }

    #[test]
    fn auto_weof_before_rewind() {
        let (mut st, _d) = tape(1 << 20);
        st.write(b"AAA").unwrap(); // rw = WRITING
        st.mt_op(MTREW, 0).unwrap();
        // rewinding while writing auto-inserted a WEOF at the old head
        assert_eq!(st.marks, vec![3]);
        assert_eq!(st.pos, 0);
        assert_eq!(st.fileno, 0);
        assert_eq!(st.rw, ST_IDLE);
    }
}
