use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use fuser::Config;

use simtap::cartridge::CartridgeLibrary;
use simtap::control;
use simtap::fs::SimFs;

#[derive(Parser)]
#[command(
    name = "simtap",
    version,
    about = "FUSE-backed faithful emulation of a Linux st-driver SCSI tape drive"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the tape drive and mount it at the given path
    Serve {
        /// Directory holding the cartridge files (.tap / .meta)
        #[arg(long)]
        library: PathBuf,
        /// Mount point for the emulated drive
        #[arg(long)]
        mount: PathBuf,
        /// Default capacity (bytes) for newly created cartridges
        #[arg(long, default_value_t = 256 * 1024 * 1024)]
        capacity: u64,
        /// Insert this cartridge on startup (created blank if it does not exist)
        #[arg(long)]
        insert: Option<String>,
    },
    /// Insert a cartridge into a running drive
    Insert {
        /// Cartridge label
        label: String,
        /// Library directory (to locate the control socket)
        #[arg(long)]
        library: PathBuf,
    },
    /// Eject the current cartridge from a running drive
    Eject {
        /// Library directory (to locate the control socket)
        #[arg(long)]
        library: PathBuf,
    },
    /// Show the status of a running drive
    Status {
        /// Library directory (to locate the control socket)
        #[arg(long)]
        library: PathBuf,
    },
    /// List cartridges present in the library
    List {
        /// Library directory
        #[arg(long)]
        library: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve {
            library,
            mount,
            capacity,
            insert,
        } => run_serve(library, mount, capacity, insert),
        Cmd::Insert { label, library } => {
            let resp = control::send_request(&library, "insert", Some(&label))?;
            report(&resp, false)
        }
        Cmd::Eject { library } => {
            let resp = control::send_request(&library, "eject", None)?;
            report(&resp, false)
        }
        Cmd::Status { library } => {
            let resp = control::send_request(&library, "status", None)?;
            report(&resp, true)
        }
        Cmd::List { library } => run_list(&library),
    }
}

fn run_serve(library: PathBuf, mount: PathBuf, capacity: u64, insert: Option<String>) -> Result<()> {
    let lib = Arc::new(CartridgeLibrary::new(library, capacity)?);

    if let Some(label) = &insert {
        lib.insert(label)?;
        eprintln!("simtap: inserted cartridge '{label}'");
    }

    control::start_server(Arc::clone(&lib))?;

    std::fs::create_dir_all(&mount)?;
    let mount_str = mount.to_string_lossy().into_owned();
    eprintln!(
        "simtap: serving tape at {mount_str} (socket: {:?}); unmount with fusermount3 -u {mount_str} or Ctrl-C",
        lib.socket_path()
    );

    let config = Config::default();
    fuser::mount(SimFs { library: Arc::clone(&lib) }, &mount_str, &config)?;

    eprintln!("simtap: unmounted");
    Ok(())
}

fn run_list(library: &Path) -> Result<()> {
    let lib = CartridgeLibrary::new(library.to_path_buf(), 0)?;
    for label in lib.labels()? {
        println!("{label}");
    }
    Ok(())
}

fn report(resp: &str, show_status: bool) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(resp)?;
    let ok = v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
    if ok {
        if show_status {
            if let Some(s) = v.get("status").and_then(|s| s.as_str()) {
                println!("{s}");
            }
        } else {
            println!("ok");
        }
    } else {
        let err = v
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown error");
        eprintln!("simtap: {err}");
        std::process::exit(1);
    }
    Ok(())
}
