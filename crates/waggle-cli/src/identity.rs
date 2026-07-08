//! `waggle identity` — the host's Ed25519 signing identity (CP-11).
//! A 32-byte seed at `~/.waggle/identity` (0600); when present, every
//! mint is signed over its immutable core.

use ed25519_dalek::SigningKey;

fn identity_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("WAGGLE_IDENTITY") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home)
        .join(".waggle")
        .join("identity")
}

/// Load the signing key if an identity exists.
pub fn load() -> Option<SigningKey> {
    let hex = std::fs::read_to_string(identity_path()).ok()?;
    let hex = hex.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut seed = [0u8; 32];
    for (i, chunk) in seed.iter_mut().enumerate() {
        *chunk = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(SigningKey::from_bytes(&seed))
}

fn pubkey_hex(key: &SigningKey) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(64);
    for b in key.verifying_key().as_bytes() {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// `waggle identity <show|init>`.
pub fn run(action: &str) -> i32 {
    match action {
        "show" => match load() {
            Some(key) => {
                println!(
                    "{}",
                    serde_json::json!({ "public_key": pubkey_hex(&key), "path": identity_path() })
                );
                0
            }
            None => {
                eprintln!("waggle identity: none — `waggle identity init` creates one");
                1
            }
        },
        "init" => {
            let path = identity_path();
            if path.exists() {
                eprintln!(
                    "waggle identity: already exists at {} — refusing to overwrite",
                    path.display()
                );
                return 1;
            }
            let mut seed = [0u8; 32];
            if let Err(e) = getrandom::getrandom(&mut seed) {
                eprintln!("waggle identity: entropy: {e}");
                return 1;
            }
            let key = SigningKey::from_bytes(&seed);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            use std::fmt::Write as _;
            let mut hex = String::with_capacity(64);
            for b in &seed {
                let _ = write!(hex, "{b:02x}");
            }
            if let Err(e) = std::fs::write(&path, hex) {
                eprintln!("waggle identity: write: {e}");
                return 1;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            println!(
                "{}",
                serde_json::json!({
                    "created": path,
                    "public_key": pubkey_hex(&key),
                    "hint": "every mint from now on is signed; back this file up — it IS your authorship",
                })
            );
            0
        }
        other => {
            eprintln!("waggle identity: `{other}` — actions are show | init");
            2
        }
    }
}
