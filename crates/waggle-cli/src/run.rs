//! Execution: CLI verb → the same [`waggle_mcp::Handler`] the MCP wire
//! uses — one dispatcher, two surfaces, zero drift (design doc `09 §2`).
//! This is also where effects finally become real: the system clock, OS
//! entropy, and the store on disk all enter here and nowhere deeper.

use std::io::{BufRead, Write as _};
use std::path::PathBuf;

use serde_json::{json, Value};
use waggle_core::{EntropyError, Sharer, Timestamp};
use waggle_mcp::{handle_session, Handler, Session};
use waggle_store_sqlite::SqliteStore;

/// OS entropy as a closure — the only randomness source in the binary.
pub fn os_entropy(buf: &mut [u8]) -> Result<(), EntropyError> {
    getrandom::getrandom(buf).map_err(|e| EntropyError(e.to_string()))
}

/// The system clock, once per command — handlers stay clock-free.
pub fn now() -> Timestamp {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    Timestamp::from_unix_ms(ms)
}

/// Store location: `WAGGLE_STORE` overrides; default is
/// `~/.waggle/waggle.db` (the machine-wide store, doc `16 §2`).
#[cfg(unix)] // consumed by the daemon status + shim skew check
pub fn store_path_display() -> String {
    store_path().display().to_string()
}

fn store_path() -> PathBuf {
    if let Ok(p) = std::env::var("WAGGLE_STORE") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".waggle").join("waggle.db")
}

pub fn open_handler() -> Result<Handler<SqliteStore, waggle_store_sqlite::BlobStore>, String> {
    let path = store_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("store dir {}: {e}", dir.display()))?;
    }
    let store = SqliteStore::open(&path).map_err(|e| e.to_string())?;
    let blobs_dir = path
        .parent()
        .map_or_else(|| PathBuf::from("blobs"), |d| d.join("blobs"));
    let blobs = waggle_store_sqlite::BlobStore::open(&blobs_dir).map_err(|e| e.to_string())?;
    let sharer = std::env::var("WAGGLE_SHARER")
        .ok()
        .and_then(|s| Sharer::new(&s).ok())
        .unwrap_or_else(|| Sharer::new("session").expect("static slug"));
    let handler = Handler::new(store, sharer).with_blobs(blobs);
    // CP-11: hosts with an identity sign every mint automatically.
    Ok(match crate::identity::load() {
        Some(key) => handler.with_signer(key),
        None => handler,
    })
}

/// Route a verb through the running daemon when one owns our store —
/// the CLI then behaves EXACTLY like MCP traffic (federation, shared
/// cache, one funnel). No daemon → the direct in-process path below.
#[cfg(unix)]
fn try_daemon_call(tool: &str, args: &Value) -> Option<i32> {
    use std::io::{BufRead, BufReader, Write as _};
    let sock = crate::daemon::socket_path();
    let mut stream = std::os::unix::net::UnixStream::connect(&sock).ok()?;
    let frame = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool, "arguments": args },
    })
    .to_string();
    writeln!(stream, "{frame}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let rpc: Value = serde_json::from_str(&line).ok()?;
    let text = rpc.pointer("/result/content/0/text")?.as_str()?;
    let envelope: Value = serde_json::from_str(text).ok()?;
    let is_err = envelope.get("hint").is_some_and(|h| !h.is_null());
    println!(
        "{}",
        serde_json::to_string_pretty(&envelope).unwrap_or_default()
    );
    Some(i32::from(is_err))
}

/// Run one tool call and print the envelope as JSON. Exit code: 0 on
/// success, 1 when the envelope carries a hint (the error contract).
#[allow(clippy::needless_pass_by_value)] // call sites build args inline
pub fn tool_call(tool: &str, args: Value) -> i32 {
    #[cfg(unix)]
    if std::env::var("WAGGLE_DIRECT").is_err() {
        if let Some(code) = try_daemon_call(tool, &args) {
            return code;
        }
    }
    let handler = match open_handler() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{}", json!({ "hint": e }));
            return 1;
        }
    };
    let envelope = pollster::block_on(handler.dispatch(tool, &args, now(), &mut os_entropy));
    let is_err = envelope.hint.is_some();
    println!(
        "{}",
        serde_json::to_string_pretty(&envelope).unwrap_or_default()
    );
    i32::from(is_err)
}

/// `waggle serve --stdio`: the MCP server over stdin/stdout — the line a
/// harness config points at. One JSON-RPC message per line in, one per
/// line out; EOF ends the session (doc `16 §3`).
pub fn serve_stdio() -> i32 {
    let handler = match open_handler() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("waggle serve: {e}");
            return 1;
        }
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    // One connection, one session: subscriptions live exactly as long
    // as the pipe (doc 21 §3).
    let mut session = Session::default();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let out = pollster::block_on(handle_session(
            &handler,
            &mut session,
            &line,
            now(),
            &mut os_entropy,
        ));
        for frame in out.reply.iter().chain(out.notifications.iter()) {
            if writeln!(stdout, "{frame}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                return 0; // client hung up
            }
        }
    }
    0
}
