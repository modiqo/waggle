//! `waggled` — the single owner of the local store (design docs `16 §2`,
//! `13 §8`). A tokio daemon on a unix socket (F-2: sockets are
//! filesystem-permissioned — no port, no token, no other user): every
//! harness on the machine talks to one process, one `SQLite` writer, one
//! cache. Clients speak newline-delimited MCP JSON-RPC — exactly what the
//! stdio shim forwards.
//!
//! Windows: the daemon is unix-only for now (named pipes tracked in the
//! plan); `serve --stdio` there runs the direct in-process server.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use waggle_mcp::{handle_message, Handler};
use waggle_store_sqlite::SqliteStore;

use crate::run::{now, open_handler, os_entropy};

/// Socket location: `WAGGLE_SOCK` overrides; default sits beside the store.
pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("WAGGLE_SOCK") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".waggle").join("waggled.sock")
}

/// Run the daemon in the foreground. Returns the process exit code.
pub fn run_daemon() -> i32 {
    let handler = match open_handler() {
        Ok(h) => Arc::new(h),
        Err(e) => {
            eprintln!("waggled: {e}");
            return 1;
        }
    };
    let sock = socket_path();
    if let Some(dir) = sock.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("waggled: tokio runtime: {e}");
            return 1;
        }
    };
    runtime.block_on(async move {
        let listener = match bind(&sock) {
            Ok(l) => l,
            Err(code) => return code,
        };
        eprintln!("waggled: listening on {}", sock.display());
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let handler = Arc::clone(&handler);
                    tokio::spawn(async move { serve_client(stream, &handler).await });
                }
                Err(e) => {
                    eprintln!("waggled: accept: {e}");
                    return 1;
                }
            }
        }
    })
}

/// Bind, handling the two stale-socket cases: another live daemon (fine —
/// exit 0 so shim auto-start is race-safe) or a dead socket file (remove
/// and rebind).
fn bind(sock: &Path) -> Result<UnixListener, i32> {
    match UnixListener::bind(sock) {
        Ok(l) => Ok(l),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if std::os::unix::net::UnixStream::connect(sock).is_ok() {
                eprintln!("waggled: already running at {}", sock.display());
                return Err(0);
            }
            let _ = std::fs::remove_file(sock);
            UnixListener::bind(sock).map_err(|e| {
                eprintln!("waggled: bind {}: {e}", sock.display());
                1
            })
        }
        Err(e) => {
            eprintln!("waggled: bind {}: {e}", sock.display());
            Err(1)
        }
    }
}

async fn serve_client(stream: UnixStream, handler: &Handler<SqliteStore>) {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_message(handler, &line, now(), &mut os_entropy).await;
        if let Some(response) = response {
            if write.write_all(response.as_bytes()).await.is_err()
                || write.write_all(b"\n").await.is_err()
            {
                break; // client hung up
            }
        }
    }
}

/// The stdio shim (16 §3): pump stdin→socket and socket→stdout, adding
/// nothing. Auto-starts the daemon when the socket is dead — the harness
/// just spawns `waggle serve --stdio` and the rest is arranged.
pub fn serve_stdio_shim() -> i32 {
    use std::io::{BufRead, BufReader as StdBufReader, Write as _};

    let sock = socket_path();
    let stream = match connect_or_start(&sock) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("waggle shim: {e}");
            return 1;
        }
    };
    let reader = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("waggle shim: {e}");
            return 1;
        }
    };

    // socket → stdout on its own thread; stdin → socket on this one.
    let pump_out = std::thread::spawn(move || {
        let mut stdout = std::io::stdout();
        for line in StdBufReader::new(reader).lines() {
            let Ok(line) = line else { break };
            if writeln!(stdout, "{line}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                break;
            }
        }
    });

    let mut stream = stream;
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if writeln!(stream, "{line}").is_err() {
            break;
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = pump_out.join();
    0
}

fn connect_or_start(sock: &Path) -> Result<std::os::unix::net::UnixStream, String> {
    if let Ok(s) = std::os::unix::net::UnixStream::connect(sock) {
        return Ok(s);
    }
    // Dead or absent: start waggled detached and wait for the socket.
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    std::process::Command::new(exe)
        .args(["serve", "--daemon"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("auto-start waggled: {e}"))?;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(s) = std::os::unix::net::UnixStream::connect(sock) {
            return Ok(s);
        }
    }
    Err(format!(
        "waggled did not come up at {} within 5s — run `waggle serve --daemon` to see why",
        sock.display()
    ))
}
