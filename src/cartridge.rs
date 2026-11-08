use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::tape::{TapeErr, TRes, TapeState};

const TAP_SUFFIX: &str = ".tap";
const META_SUFFIX: &str = ".meta";

/// Cartridge metadata persisted alongside the raw `.tap` data file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CartridgeMeta {
    pub write_prot: bool,
    pub capacity: u64,
    pub density: u32,
    pub block_size: u32,
    pub marks: Vec<u64>,
}

/// Owns the on-disk cartridge store and the single shared [`TapeState`].
///
/// All tape state lives behind one `Arc<Mutex<TapeState>>` so that the FUSE
/// worker threads and the control-server thread all observe the same drive.
pub struct CartridgeLibrary {
    pub path: PathBuf,
    pub default_capacity: u64,
    pub tape: Arc<Mutex<TapeState>>,
}

impl CartridgeLibrary {
    pub fn new(path: PathBuf, default_capacity: u64) -> TRes<Self> {
        fs::create_dir_all(&path).map_err(TapeErr::from_io)?;
        let tape = Arc::new(Mutex::new(TapeState::offline(default_capacity)));
        Ok(Self {
            path,
            default_capacity,
            tape,
        })
    }

    /// A shared handle to the single tape state, for the FUSE filesystem.
    pub fn shared(&self) -> Arc<Mutex<TapeState>> {
        Arc::clone(&self.tape)
    }

    /// Run a closure against the live tape state under the lock.
    pub fn with_tape<R, F>(&self, f: F) -> TRes<R>
    where
        F: FnOnce(&mut TapeState) -> TRes<R>,
    {
        let mut g = self.tape.lock().unwrap();
        f(&mut g)
    }

    /// The 48-byte `struct mtget` payload (MTIOCGET), built under the lock.
    pub fn mtget(&self) -> [u8; 48] {
        let g = self.tape.lock().unwrap();
        g.mtget_bytes()
    }

    /// The 8-byte `struct mtpos` payload (MTIOCPOS), built under the lock.
    pub fn mtpos(&self) -> [u8; 8] {
        let g = self.tape.lock().unwrap();
        g.mtpos_bytes()
    }

    fn tap_path(&self, label: &str) -> PathBuf {
        self.path.join(format!("{label}{TAP_SUFFIX}"))
    }

    fn meta_path(&self, label: &str) -> PathBuf {
        self.path.join(format!("{label}{META_SUFFIX}"))
    }

    /// List cartridge labels present in the library (names without the suffix).
    pub fn labels(&self) -> TRes<Vec<String>> {
        let mut out = Vec::new();
        let rd = fs::read_dir(&self.path).map_err(TapeErr::from_io)?;
        for entry in rd {
            let entry = entry.map_err(TapeErr::from_io)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(TAP_SUFFIX) {
                out.push(stem.to_string());
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Build a fresh loaded [`TapeState`] from disk for `label`.
    pub fn load_cartridge(&self, label: &str) -> TRes<TapeState> {
        let tap = self.tap_path(label);
        if !tap.exists() {
            return Err(TapeErr::ENOMEDIUM);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&tap)
            .map_err(TapeErr::from_io)?;
        let data_len = file.metadata().map_err(TapeErr::from_io)?.len();

        let mut write_prot = false;
        let mut capacity = self.default_capacity;
        let mut density: u32 = 0;
        let mut block_size: u32 = 0;
        let mut marks: Vec<u64> = Vec::new();
        let meta_path = self.meta_path(label);
        if meta_path.exists() {
            let raw = fs::read(&meta_path).map_err(TapeErr::from_io)?;
            let meta: CartridgeMeta = serde_json::from_slice(&raw).map_err(|_| TapeErr::EIO)?;
            write_prot = meta.write_prot;
            capacity = meta.capacity;
            density = meta.density;
            block_size = meta.block_size;
            marks = meta.marks;
        }
        marks.sort_unstable();

        let mut st = TapeState::offline(capacity.max(data_len));
        st.label = Some(label.to_string());
        st.loaded = true;
        st.cartridge_write_prot = write_prot;
        st.density = density;
        st.block_size = block_size;
        st.data_file = Some(file);
        st.data_len = data_len;
        st.marks = marks;
        Ok(st)
    }

    /// Persist a loaded state's raw data and metadata to disk.
    fn save_locked(&self, st: &TapeState) -> TRes<()> {
        if !st.loaded {
            return Ok(());
        }
        let label = match &st.label {
            Some(l) => l.clone(),
            None => return Ok(()),
        };
        if let Some(file) = st.data_file.as_ref() {
            file.sync_all().map_err(TapeErr::from_io)?;
        }
        let meta = CartridgeMeta {
            write_prot: st.cartridge_write_prot,
            capacity: st.capacity,
            density: st.density,
            block_size: st.block_size,
            marks: st.marks.clone(),
        };
        let raw = serde_json::to_vec_pretty(&meta).map_err(|_| TapeErr::EIO)?;
        fs::write(self.meta_path(&label), raw).map_err(TapeErr::from_io)?;
        Ok(())
    }

    /// Persist the current live tape state to disk.
    pub fn save(&self) -> TRes<()> {
        let g = self.tape.lock().unwrap();
        self.save_locked(&g)
    }

    /// Create a fresh blank cartridge with the given capacity.
    pub fn create(&self, label: &str, capacity: u64) -> TRes<()> {
        let tap = self.tap_path(label);
        if tap.exists() {
            return Err(TapeErr::EIO);
        }
        File::create(&tap).map_err(TapeErr::from_io)?;
        let meta = CartridgeMeta {
            write_prot: false,
            capacity,
            density: 0,
            block_size: 0,
            marks: Vec::new(),
        };
        let raw = serde_json::to_vec_pretty(&meta).map_err(|_| TapeErr::EIO)?;
        fs::write(self.meta_path(label), raw).map_err(TapeErr::from_io)?;
        Ok(())
    }

    /// Insert a cartridge. A blank cartridge is created if the label is new.
    /// The previous live cartridge (if any) is saved first.
    pub fn insert(&self, label: &str) -> TRes<()> {
        if !self.tap_path(label).exists() {
            self.create(label, self.default_capacity)?;
        }
        let new_state = self.load_cartridge(label)?;
        let mut g = self.tape.lock().unwrap();
        if g.in_use {
            return Err(TapeErr::EBUSY);
        }
        self.save_locked(&g)?;
        *g = new_state;
        Ok(())
    }

    /// Eject the current cartridge (saving it) and go back to an empty drive.
    pub fn eject(&self) -> TRes<()> {
        let mut g = self.tape.lock().unwrap();
        if g.in_use {
            return Err(TapeErr::EBUSY);
        }
        self.save_locked(&g)?;
        let cap = g.capacity;
        *g = TapeState::offline(cap);
        Ok(())
    }

    /// Human-readable status string for the current live tape.
    pub fn status(&self) -> String {
        let g = self.tape.lock().unwrap();
        if g.loaded {
            g.summary()
        } else {
            "label=(none) loaded=false (drive empty)".to_string()
        }
    }

    /// Path of the control socket for this library.
    pub fn socket_path(&self) -> PathBuf {
        self.path.join(".simtap.sock")
    }
}
