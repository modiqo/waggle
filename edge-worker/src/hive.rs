//! The Hive: one Durable Object per tenant — `waggled`'s single writer,
//! relocated (design doc `08 §8`). The DO's execution model serializes
//! every request, which is exactly the atomicity the storage contract
//! needs; the engine inside is the natively-certified
//! [`waggle_store_cloudflare::EdgeStore`], unchanged.

use waggle_core::Timestamp;
use waggle_mcp::Handler;
use waggle_store::StoreError;
use waggle_store_cloudflare::{EdgeStorage, EdgeStore};
use worker::*;

/// DO storage speaking the five-verb seam.
pub struct DoStorage {
    storage: Storage,
}

fn backend(e: worker::Error) -> StoreError {
    StoreError::Backend(format!("do-storage: {e}"))
}

impl EdgeStorage for DoStorage {
    async fn get(&self, key: &str) -> std::result::Result<Option<String>, StoreError> {
        self.storage.get::<String>(key).await.map_err(backend)
    }

    async fn put(&self, key: &str, value: &str) -> std::result::Result<(), StoreError> {
        self.storage.put(key, value).await.map_err(backend)
    }

    async fn delete(&self, key: &str) -> std::result::Result<(), StoreError> {
        self.storage.delete(key).await.map(|_| ()).map_err(backend)
    }

    async fn list(&self, prefix: &str) -> std::result::Result<Vec<(String, String)>, StoreError> {
        let map = self
            .storage
            .list_with_options(ListOptions::new().prefix(prefix))
            .await
            .map_err(backend)?;
        let mut out = Vec::with_capacity(map.size() as usize);
        let entries = js_sys::try_iter(&map)
            .map_err(|e| StoreError::Backend(format!("list iter: {e:?}")))?
            .ok_or_else(|| StoreError::Backend("list not iterable".into()))?;
        for entry in entries {
            let pair: js_sys::Array = entry
                .map_err(|e| StoreError::Backend(format!("list entry: {e:?}")))?
                .into();
            let key = pair.get(0).as_string().unwrap_or_default();
            let value = pair.get(1).as_string().unwrap_or_default();
            out.push((key, value));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}

/// The per-tenant hive.
#[durable_object]
pub struct Hive {
    state: State,
    env: Env,
}

impl DurableObject for Hive {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body = req.text().await?;
        let blobs = match self.env.bucket("BUCKET") {
            Ok(b) => crate::blobs::EdgeBlobs::R2(b),
            Err(_) => crate::blobs::EdgeBlobs::Absent,
        };
        let handler = Handler::new(
            EdgeStore::new(DoStorage {
                storage: self.state.storage(),
            }),
            waggle_core::Sharer::new("edge").expect("static slug"),
        )
        .with_blobs(blobs);
        let now = Timestamp::from_unix_ms(Date::now().as_millis());
        let mut entropy = |buf: &mut [u8]| {
            getrandom::getrandom(buf).map_err(|e| waggle_core::EntropyError(e.to_string()))
        };

        let url = req.url()?;
        match url.path() {
            "/mcp" => {
                let response = waggle_mcp::handle_message(&handler, &body, now, &mut entropy).await;
                match response {
                    Some(r) => Response::ok(r),
                    None => Response::ok(""), // notification: no reply
                }
            }
            "/store" => store_rpc(&handler, &body).await,
            other => Response::error(format!("hive: unknown path {other}"), 404),
        }
    }
}

/// The store RPC: certification + replay-migration's wire. One JSON
/// object per call: `{op: "ingest", record}` · `{op: "scan"}` ·
/// `{op: "scan-token", token, from_seq}` — plus the append ops the
/// conformance harness drives directly.
async fn store_rpc<S: waggle_store::Store, B: waggle_store::BlobSink>(
    handler: &Handler<S, B>,
    body: &str,
) -> Result<Response> {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return Response::error(format!("store: bad json: {e}"), 400),
    };
    let store = handler.store();
    let op = msg["op"].as_str().unwrap_or_default();
    let out: std::result::Result<serde_json::Value, StoreError> = match op {
        "ingest" => match serde_json::from_value(msg["record"].clone()) {
            Ok(record) => store
                .ingest(record)
                .await
                .map(|fresh| serde_json::json!({ "fresh": fresh })),
            Err(e) => return Response::error(format!("store: record: {e}"), 400),
        },
        "append" => match serde_json::from_value(msg["intent"].clone()) {
            Ok(intent) => store.append(intent).await.map(|receipt| {
                serde_json::to_value(ReceiptWire::from(receipt)).unwrap_or_default()
            }),
            Err(e) => return Response::error(format!("store: intent: {e}"), 400),
        },
        "scan" => store
            .scan_all()
            .await
            .map(|records| serde_json::to_value(records).unwrap_or_default()),
        "scan-token" => {
            let token = msg["token"].as_str().unwrap_or_default();
            let from = u32::try_from(msg["from_seq"].as_u64().unwrap_or(0)).unwrap_or(0);
            match waggle_core::Token::parse(token) {
                Ok(t) => store
                    .scan_token(t, waggle_core::Seq(from))
                    .await
                    .map(|records| serde_json::to_value(records).unwrap_or_default()),
                Err(e) => return Response::error(format!("store: token: {e}"), 400),
            }
        }
        "put-blob" => {
            // Replication: `waggle edge push` uploads snapshot blobs so
            // read/search work where the file never existed (doc 18 §3).
            use base64::Engine as _;
            let b64 = msg["b64"].as_str().unwrap_or_default();
            let content_type = msg["content_type"]
                .as_str()
                .unwrap_or("application/octet-stream");
            match base64::engine::general_purpose::STANDARD.decode(b64) {
                Ok(bytes) => handler
                    .blobs()
                    .put(&bytes, content_type)
                    .await
                    .map(|media| serde_json::to_value(media).unwrap_or_default()),
                Err(e) => return Response::error(format!("store: b64: {e}"), 400),
            }
        }
        other => return Response::error(format!("store: unknown op `{other}`"), 400),
    };
    match out {
        Ok(v) => Response::ok(serde_json::json!({ "ok": v }).to_string()),
        Err(e) => Response::ok(serde_json::json!({ "err": e.to_string() }).to_string()),
    }
}

/// `Appended` isn't serde (it carries an Arc'd view); the wire form is.
#[derive(serde::Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
enum ReceiptWire {
    Minted {
        token: String,
        replayed: bool,
        version: u32,
    },
    Mutated {
        seq: u32,
        version: u32,
    },
    Event {
        seq: u32,
    },
}

impl From<waggle_store::Appended> for ReceiptWire {
    fn from(a: waggle_store::Appended) -> Self {
        match a {
            waggle_store::Appended::Minted { view, replayed } => Self::Minted {
                token: view.manifest.token.as_str().to_owned(),
                replayed,
                version: view.version(),
            },
            waggle_store::Appended::Mutated { seq, version } => Self::Mutated {
                seq: seq.0,
                version,
            },
            waggle_store::Appended::Event { seq } => Self::Event { seq: seq.0 },
        }
    }
}
