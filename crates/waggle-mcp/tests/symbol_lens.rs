#![cfg(feature = "code-lens")]
//! Doc-20 gates, end to end over the dispatcher and the real `SQLite`
//! store: the outline minted beside the snapshot, the symbols table of
//! contents in the overview, `read --symbol`, and `--require symbol:`
//! flowing through the P1 receipt machinery unchanged.

use serde_json::json;
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::Handler;
use waggle_store_sqlite::{BlobStore, SqliteStore};

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0x1E45_u32;
    move |buf: &mut [u8]| {
        for b in buf.iter_mut() {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *b = (state & 0xFF) as u8;
        }
        Ok(())
    }
}

const RUST_SRC: &str = r"//! A module worth auditing.

pub struct Ledger {
    total: u64,
}

impl Ledger {
    /// The load-bearing arithmetic.
    pub fn settle(&mut self, amount: u64) -> u64 {
        self.total += amount;
        self.total
    }
}

fn helper() -> u64 {
    41
}

pub fn main_entry() -> u64 {
    helper() + 1
}
";

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("waggle-symlens-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn handler(dir: &std::path::Path) -> Handler<SqliteStore, BlobStore> {
    Handler::new(
        SqliteStore::open_in_memory().unwrap(),
        Sharer::new("lead").unwrap(),
    )
    .with_blobs(BlobStore::open(&dir.join("blobs")).unwrap())
}

#[test]
fn snapshot_mint_carries_the_outline_and_the_overview_serves_it() {
    let dir = scratch("overview");
    let file = dir.join("ledger.rs");
    std::fs::write(&file, RUST_SRC).unwrap();
    let h = handler(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let minted = h
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", file.display()), "snapshot": true }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let token = minted.result["token"].as_str().unwrap().to_owned();

        // The overview: symbols precomputed at mint, budget-fitted, and
        // the symbol lens advertised — orientation before content.
        let over = h
            .dispatch(
                "read",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(over.hint.is_none(), "{over:?}");
        let symbols = over.result["symbols"]["symbols"].as_array().unwrap();
        assert!(
            symbols.iter().any(|s| s["name"] == "settle"),
            "symbols: {symbols:?}"
        );
        assert_eq!(over.result["symbols"]["omitted"], 0);
        assert!(over.result["lenses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "symbol"));

        // read --symbol serves the definition's window through the
        // ordinary lines path (region stamping applies unchanged).
        let read = h
            .dispatch(
                "read",
                &json!({ "token": token, "symbol": "settle" }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(read.hint.is_none(), "{read:?}");
        assert!(read.result["text"]
            .as_str()
            .unwrap()
            .contains("self.total += amount"));

        // Misses teach: unknown symbols name what exists.
        let miss = h
            .dispatch(
                "read",
                &json!({ "token": token, "symbol": "nope" }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(miss.hint.unwrap().contains("settle"));
    });
}

#[test]
fn symbol_contracts_flow_through_the_receipt_machinery() {
    let dir = scratch("contract");
    let file = dir.join("ledger.rs");
    std::fs::write(&file, RUST_SRC).unwrap();
    let h = handler(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let minted = h
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", file.display()),
                    "snapshot": true,
                    "require": ["symbol:settle"],
                }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let token = minted.result["token"].as_str().unwrap().to_owned();

        // Unmet until the definition is actually served; the miss is
        // NAMED with the symbol's label.
        let unmet = h
            .dispatch(
                "coverage",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert_eq!(unmet.result["met"], false, "{unmet:?}");
        assert_eq!(unmet.result["missed"][0]["label"], "settle");

        // Reading the symbol closes the contract — the symbol lens and
        // the P1 bitmask compose with zero new machinery.
        h.dispatch(
            "read",
            &json!({ "token": token, "symbol": "settle" }),
            Timestamp::from_unix_ms(3),
            &mut e,
        )
        .await;
        let met = h
            .dispatch(
                "coverage",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert_eq!(met.result["met"], true, "{met:?}");

        // The refusals teach: unknown symbol at mint fails AT MINT.
        let bad = h
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", file.display()),
                    "snapshot": true,
                    "require": ["symbol:nonexistent"],
                }),
                Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert!(bad.hint.unwrap().contains("no symbol"));
    });
}
