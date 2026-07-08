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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

use waggle_mcp::{handle_message, Handler};
use waggle_store::ReadStore as _;
use waggle_store_sqlite::SqliteStore;

use crate::run::{now, open_handler, os_entropy};

/// Shared daemon state for status reporting and idle detection.
struct DaemonState {
    started_unix_ms: u64,
    active_connections: AtomicU64,
    last_activity_secs: AtomicU64,
    /// G-8: cached foreign resolutions, honoring each response's OWN
    /// `revalidate_after` stamp. key → (envelope text, expires unix-ms).
    resolutions: std::sync::Mutex<std::collections::HashMap<String, (String, u64)>>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The pidfile sits beside the socket: written at bind, removed at
/// shutdown; `stop` uses it to terminate orphans whose socket died.
fn pid_path(sock: &Path) -> PathBuf {
    sock.with_extension("pid")
}

fn cleanup(sock: &Path) {
    let _ = std::fs::remove_file(sock);
    let _ = std::fs::remove_file(pid_path(sock));
}

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
        let _ = std::fs::write(pid_path(&sock), std::process::id().to_string());
        let state = Arc::new(DaemonState {
            started_unix_ms: now_secs() * 1000,
            active_connections: AtomicU64::new(0),
            last_activity_secs: AtomicU64::new(now_secs()),
            resolutions: std::sync::Mutex::new(std::collections::HashMap::new()),
        });
        spawn_idle_monitor(&state, &sock);
        spawn_tcp_listener(&handler, &state, &sock);
        eprintln!(
            "waggled: listening on {} (pid {})",
            sock.display(),
            std::process::id()
        );
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let handler = Arc::clone(&handler);
                    let state = Arc::clone(&state);
                    let sock = sock.clone();
                    tokio::spawn(async move {
                        let (read, write) = tokio::io::split(stream);
                        let lines = BufReader::new(read).lines();
                        serve_client(lines, write, &handler, &state, &sock).await;
                    });
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

async fn serve_client<R, W>(
    mut lines: tokio::io::Lines<BufReader<R>>,
    mut write: W,
    handler: &Handler<SqliteStore, waggle_store_sqlite::BlobStore>,
    state: &DaemonState,
    sock: &Path,
) where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    state.active_connections.fetch_add(1, Ordering::SeqCst);
    state.last_activity_secs.store(now_secs(), Ordering::SeqCst);
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        state.last_activity_secs.store(now_secs(), Ordering::SeqCst);
        // Daemon-level management methods are intercepted BEFORE the MCP
        // dispatcher — management is not a tool agents see (16 appendix).
        // Tier 2: a forwardable frame whose token this store doesn't own
        // goes upstream when one is configured (16 §3, 08 §0) — the
        // computation runs at the owner; only the answer comes back.
        if let Some(token) = crate::remote::forwardable_token(&line) {
            let known = handler
                .store()
                .manifest(token)
                .await
                .ok()
                .flatten()
                .is_some();
            if !known && std::env::var("WAGGLE_UPSTREAM").is_ok() {
                let id = serde_json::from_str::<serde_json::Value>(&line)
                    .ok()
                    .and_then(|m| m.get("id").cloned())
                    .unwrap_or(serde_json::Value::Null);
                // G-8: eventual serves a cached resolution inside its own
                // revalidate window; strict always consults the owner.
                let cache = crate::remote::resolve_cache_key(&line);
                if let Some((key, level)) = &cache {
                    if level != "strict" {
                        let now_ms = now_secs() * 1000;
                        let hit = state
                            .resolutions
                            .lock()
                            .ok()
                            .and_then(|m| m.get(key).cloned())
                            .filter(|(_, expires)| now_ms < *expires);
                        if let Some((envelope, _)) = hit {
                            let response = crate::remote::rewrap(&envelope, &id);
                            if write.write_all(response.as_bytes()).await.is_err()
                                || write.write_all(b"\n").await.is_err()
                            {
                                break;
                            }
                            continue;
                        }
                    }
                }
                let forwarded = crate::remote::forward(&line).await;
                if let (Some((key, _)), Some(r)) = (&cache, &forwarded) {
                    if let Some((envelope, expires)) = crate::remote::cacheable_resolution(r) {
                        if let Ok(mut m) = state.resolutions.lock() {
                            m.insert(key.clone(), (envelope, expires));
                        }
                    }
                }
                let response = match forwarded {
                    Some(r) => r,
                    None => crate::remote::unreachable_response(&id, token),
                };
                if write.write_all(response.as_bytes()).await.is_err()
                    || write.write_all(b"\n").await.is_err()
                {
                    break;
                }
                continue;
            }
        }
        let response = if let Some(mgmt) = manage_message(&line, state, sock) {
            let shutdown = mgmt.1;
            if write.write_all(mgmt.0.as_bytes()).await.is_ok() {
                let _ = write.write_all(b"\n").await;
            }
            if shutdown {
                eprintln!("waggled: shutdown requested — exiting");
                cleanup(sock);
                std::process::exit(0);
            }
            None
        } else {
            handle_message(handler, &line, now(), &mut os_entropy).await
        };
        if let Some(response) = response {
            if write.write_all(response.as_bytes()).await.is_err()
                || write.write_all(b"\n").await.is_err()
            {
                break; // client hung up
            }
        }
    }
    state.active_connections.fetch_sub(1, Ordering::SeqCst);
    state.last_activity_secs.store(now_secs(), Ordering::SeqCst);
}

/// Handle `waggled/status` and `waggled/shutdown`. Returns the response
/// line and whether to exit after sending it.
fn manage_message(line: &str, state: &DaemonState, sock: &Path) -> Option<(String, bool)> {
    let msg: serde_json::Value = serde_json::from_str(line).ok()?;
    let method = msg.get("method")?.as_str()?;
    if !method.starts_with("waggled/") {
        return None;
    }
    let id = msg.get("id").cloned().unwrap_or(serde_json::Value::Null);
    match method {
        "waggled/status" => {
            let result = serde_json::json!({
                "pid": std::process::id(),
                "version": env!("CARGO_PKG_VERSION"),
                "store": crate::run::store_path_display(),
                "socket": sock.display().to_string(),
                "started_unix_ms": state.started_unix_ms,
                "uptime_secs": now_secs().saturating_sub(state.started_unix_ms / 1000),
                "active_connections": state.active_connections.load(Ordering::SeqCst),
            });
            Some((
                serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
                false,
            ))
        }
        "waggled/shutdown" => Some((
            serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": { "stopping": true, "pid": std::process::id() } })
                .to_string(),
            true,
        )),
        _ => Some((
            serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "unknown waggled method" } })
                .to_string(),
            false,
        )),
    }
}

/// Token-gated TCP (F-2 tier 2): listen only when BOTH `WAGGLE_TCP` and
/// `WAGGLE_TCP_TOKEN` are set — an unauthenticated network listener is
/// not a mode this daemon has. Every connection must open with the
/// `waggled/hello` bearer frame or it is dropped without a byte served.
fn spawn_tcp_listener(
    handler: &Arc<Handler<SqliteStore, waggle_store_sqlite::BlobStore>>,
    state: &Arc<DaemonState>,
    sock: &Path,
) {
    let Ok(addr) = std::env::var("WAGGLE_TCP") else {
        return;
    };
    let Ok(gate) = std::env::var("WAGGLE_TCP_TOKEN") else {
        eprintln!(
            "waggled: WAGGLE_TCP set without WAGGLE_TCP_TOKEN — refusing to listen unauthenticated"
        );
        return;
    };
    if gate.len() < 16 {
        eprintln!("waggled: WAGGLE_TCP_TOKEN is under 16 chars — refusing (generate one: openssl rand -hex 16)");
        return;
    }
    let handler = Arc::clone(handler);
    let state = Arc::clone(state);
    let sock = sock.to_path_buf();
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("waggled: tcp bind {addr}: {e}");
                return;
            }
        };
        eprintln!("waggled: tcp listening on {addr} (token-gated)");
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                break;
            };
            let handler = Arc::clone(&handler);
            let state = Arc::clone(&state);
            let gate = gate.clone();
            let sock = sock.clone();
            tokio::spawn(async move {
                let (read, write) = tokio::io::split(stream);
                let mut lines = BufReader::new(read).lines();
                // The gate: first line must be a valid hello.
                match lines.next_line().await {
                    Ok(Some(first)) if crate::remote::hello_ok(&first, &gate) => {}
                    _ => {
                        eprintln!("waggled: tcp {peer}: rejected (bad hello)");
                        return;
                    }
                }
                serve_client(lines, write, &handler, &state, &sock).await;
            });
        }
    });
}

/// Idle exit (`WAGGLE_IDLE_SECS`): a daemon with zero connections and no
/// activity for the window cleans up and leaves; the next shim start
/// revives it. Orphans do not outlive their usefulness.
fn spawn_idle_monitor(state: &Arc<DaemonState>, sock: &Path) {
    let Ok(idle) = std::env::var("WAGGLE_IDLE_SECS").map(|s| s.parse::<u64>().unwrap_or(0)) else {
        return;
    };
    if idle == 0 {
        return;
    }
    let state = Arc::clone(state);
    let sock = sock.to_path_buf();
    let interval = std::time::Duration::from_secs((idle / 4).max(1));
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            let quiet = now_secs().saturating_sub(state.last_activity_secs.load(Ordering::SeqCst));
            if state.active_connections.load(Ordering::SeqCst) == 0 && quiet >= idle {
                eprintln!("waggled: idle {idle}s with no connections — exiting");
                cleanup(&sock);
                std::process::exit(0);
            }
        }
    });
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
    // F-4: verify the daemon owns the store THIS session expects — a
    // stale daemon bound to a different (or deleted) store must fail
    // loudly here, not serve silently wrong answers.
    if let Err(e) = verify_store(&sock) {
        eprintln!("waggle shim: {e}");
        return 1;
    }
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

/// F-4 (16 §6): compare the daemon's advertised store with our own
/// expectation; mismatch is a configuration skew, named with its fix.
fn verify_store(sock: &Path) -> Result<(), String> {
    let Some(status) = rpc_once(sock, "waggled/status") else {
        return Ok(()); // daemon vanished between connect and check; pumping will surface it
    };
    let daemon_store = status["store"].as_str().unwrap_or_default().to_owned();
    let expected = crate::run::store_path_display();
    if daemon_store != expected {
        return Err(format!(
            "store skew: waggled (pid {}) owns `{daemon_store}` but this session expects `{expected}` — run `waggle daemon restart` with the right WAGGLE_STORE, or unset the override",
            status["pid"]
        ));
    }
    Ok(())
}

fn connect_or_start(sock: &Path) -> Result<std::os::unix::net::UnixStream, String> {
    if let Ok(s) = std::os::unix::net::UnixStream::connect(sock) {
        return Ok(s);
    }
    // Dead or absent: start waggled detached and wait for the socket.
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let idle = std::env::var("WAGGLE_IDLE_SECS").unwrap_or_else(|_| "1800".into());
    std::process::Command::new(exe)
        .args(["serve", "--daemon"])
        .env("WAGGLE_IDLE_SECS", idle)
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

// ─── the management client: waggle daemon <action> ─────────────────────

fn rpc_once(sock: &Path, method: &str) -> Option<serde_json::Value> {
    use std::io::{BufRead, BufReader as StdBufReader, Write as _};
    let mut stream = std::os::unix::net::UnixStream::connect(sock).ok()?;
    let frame = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method }).to_string();
    writeln!(stream, "{frame}").ok()?;
    let mut line = String::new();
    StdBufReader::new(stream).read_line(&mut line).ok()?;
    serde_json::from_str::<serde_json::Value>(&line)
        .ok()?
        .get("result")
        .cloned()
}

fn orphan_pid(sock: &Path) -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(pid_path(sock))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    // Alive? Signal 0 probes without touching.
    let alive = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|s| s.success());
    alive.then_some(pid)
}

fn start_detached(idle_secs: Option<u64>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let idle = idle_secs
        .map(|s| s.to_string())
        .or_else(|| std::env::var("WAGGLE_IDLE_SECS").ok())
        .unwrap_or_else(|| "0".into());
    std::process::Command::new(exe)
        .args(["serve", "--daemon"])
        .env("WAGGLE_IDLE_SECS", idle)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("start waggled: {e}"))?;
    let sock = socket_path();
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if rpc_once(&sock, "waggled/status").is_some() {
            return Ok(());
        }
    }
    Err("waggled did not come up within 5s — run `waggle serve --daemon` to see why".into())
}

fn do_stop(sock: &Path) -> i32 {
    if let Some(result) = rpc_once(sock, "waggled/shutdown") {
        println!(
            "{}",
            serde_json::json!({ "stopped": true, "pid": result["pid"] })
        );
        // Give it a beat to release the socket.
        std::thread::sleep(std::time::Duration::from_millis(200));
        return 0;
    }
    if let Some(pid) = orphan_pid(sock) {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        cleanup(sock);
        println!(
            "{}",
            serde_json::json!({ "stopped": true, "pid": pid, "was_orphan": true })
        );
        return 0;
    }
    cleanup(sock); // sweep any stale files
    eprintln!("waggle daemon stop: not running");
    1
}

/// The last resort for zombies: find EVERY `waggle serve --daemon`
/// process owned by this user — including ones whose sockets and
/// pidfiles were deleted (test debris, removed temp dirs) that `stop`
/// cannot reach — TERM them, escalate to KILL for survivors. Stale
/// socket files are handled by the next start's rebind.
fn purge() -> i32 {
    let out = std::process::Command::new("pgrep")
        .args([
            "-u",
            &std::env::var("USER").unwrap_or_default(),
            "-f",
            "waggle serve --daemon",
        ])
        .output();
    let pids: Vec<u32> = match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .filter(|p| *p != std::process::id())
            .collect(),
        Err(e) => {
            eprintln!("waggle daemon purge: pgrep: {e}");
            return 1;
        }
    };
    if pids.is_empty() {
        println!("{}", serde_json::json!({ "purged": [], "count": 0 }));
        return 0;
    }
    for pid in &pids {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut killed = Vec::new();
    for pid in &pids {
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|s| s.success());
        if alive {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
            killed.push(*pid);
        }
    }
    println!(
        "{}",
        serde_json::json!({ "purged": pids, "count": pids.len(), "needed_sigkill": killed })
    );
    0
}

/// `waggle daemon <status|start|stop|restart>` — exit 0 on success/true,
/// 1 when status finds nothing running or stop had nothing to stop.
pub fn manage(action: &str, idle_secs: Option<u64>) -> i32 {
    let sock = socket_path();
    match action {
        "status" => {
            if let Some(result) = rpc_once(&sock, "waggled/status") {
                // One line, scriptable: `waggle daemon status | jq .pid`.
                println!("{}", serde_json::to_string(&result).unwrap_or_default());
                0
            } else if let Some(pid) = orphan_pid(&sock) {
                eprintln!(
                    "waggle daemon status: ORPHAN — pid {pid} is alive but its socket is dead; run `waggle daemon stop`"
                );
                1
            } else {
                eprintln!("waggle daemon status: not running");
                1
            }
        }
        "start" => {
            if let Some(result) = rpc_once(&sock, "waggled/status") {
                println!(
                    "{}",
                    serde_json::json!({ "already_running": true, "pid": result["pid"] })
                );
                return 0;
            }
            match start_detached(idle_secs) {
                Ok(()) => manage("status", None),
                Err(e) => {
                    eprintln!("waggle daemon start: {e}");
                    1
                }
            }
        }
        "stop" => do_stop(&sock),
        "purge" => purge(),
        "restart" => {
            let _ = do_stop(&sock);
            match start_detached(idle_secs) {
                Ok(()) => manage("status", None),
                Err(e) => {
                    eprintln!("waggle daemon restart: {e}");
                    1
                }
            }
        }
        other => {
            eprintln!(
                "waggle daemon: `{other}` — actions are status | start | stop | restart | purge"
            );
            2
        }
    }
}
