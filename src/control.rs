use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use serde::{Deserialize, Serialize};

use crate::cartridge::CartridgeLibrary;

const SOCK_NAME: &str = ".simtap.sock";

/// A control-plane command.
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Cmd {
    Insert { label: String },
    Eject,
    Status,
}

/// A control-plane response.
#[derive(Serialize, Deserialize)]
struct Resp {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
}

fn ok_resp() -> Resp {
    Resp {
        ok: true,
        error: None,
        status: None,
    }
}

fn err_resp(msg: String) -> Resp {
    Resp {
        ok: false,
        error: Some(msg),
        status: None,
    }
}

/// Bind the control socket and serve requests on background threads.
/// Returns once the listener is bound (the accept loop runs detached).
pub fn start_server(library: Arc<CartridgeLibrary>) -> std::io::Result<()> {
    let sock = library.path.join(SOCK_NAME);
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock)?;
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let lib = Arc::clone(&library);
            thread::spawn(move || {
                let _ = handle_client(stream, &lib);
            });
        }
    });
    Ok(())
}

fn handle_client(stream: UnixStream, library: &CartridgeLibrary) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let resp = match parse_cmd(trimmed) {
            Ok(Cmd::Insert { label }) => match library.insert(&label) {
                Ok(()) => ok_resp(),
                Err(e) => err_resp(e.to_string()),
            },
            Ok(Cmd::Eject) => match library.eject() {
                Ok(()) => ok_resp(),
                Err(e) => err_resp(e.to_string()),
            },
            Ok(Cmd::Status) => Resp {
                ok: true,
                error: None,
                status: Some(library.status()),
            },
            Err(msg) => err_resp(msg),
        };
        let mut out = serde_json::to_string(&resp).unwrap_or_else(|_| "{\"ok\":false}".into());
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        writer.flush()?;
    }
    Ok(())
}

fn parse_cmd(line: &str) -> Result<Cmd, String> {
    serde_json::from_str(line).map_err(|e| format!("bad request: {e}"))
}

/// Send a single control request to a running `simtap serve` and return the
/// raw JSON response line.
pub fn send_request(lib_path: &Path, op: &str, label: Option<&str>) -> std::io::Result<String> {
    let sock = lib_path.join(SOCK_NAME);
    let mut stream = UnixStream::connect(&sock)?;
    let req = build_request(op, label)?;
    stream.write_all(req.as_bytes())?;
    stream.write_all(b"\n")?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn build_request(op: &str, label: Option<&str>) -> std::io::Result<String> {
    match op {
        "insert" => {
            let label = label.ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "insert requires a label")
            })?;
            Ok(serde_json::json!({ "op": "insert", "label": label }).to_string())
        }
        "eject" => Ok(serde_json::json!({ "op": "eject" }).to_string()),
        "status" => Ok(serde_json::json!({ "op": "status" }).to_string()),
        other => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown op: {other}"),
        )),
    }
}
