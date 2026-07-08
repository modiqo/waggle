//! Blob storage at the edge: R2 behind the same [`BlobSink`] seam every
//! tier uses — content-addressed, hash-verified reads (doc 18). Absent
//! binding degrades to the `NoBlobs` messages: the worker keeps serving
//! everything that doesn't need bytes.

use sha2::{Digest, Sha256};
use waggle_core::{CanonicalUrl, MediaRef, Sha256Hex};
use waggle_store::{BlobSink, StoreError};
use worker::Bucket;

/// R2-backed blobs, or a graceful refusal when the binding is absent.
pub enum EdgeBlobs {
    /// The BUCKET binding is configured.
    R2(Bucket),
    /// No binding: every call explains itself.
    Absent,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

impl BlobSink for EdgeBlobs {
    async fn put(&self, bytes: &[u8], content_type: &str) -> Result<MediaRef, StoreError> {
        let Self::R2(bucket) = self else {
            return Err(StoreError::Backend(
                "this edge has no R2 bucket bound — add the BUCKET binding to enable content"
                    .into(),
            ));
        };
        let sha = hex(&Sha256::digest(bytes));
        bucket
            .put(&sha, bytes.to_vec())
            .execute()
            .await
            .map_err(|e| StoreError::Backend(format!("r2 put: {e}")))?;
        Ok(MediaRef {
            uri: CanonicalUrl::new(&format!("blob://{sha}"))
                .map_err(|e| StoreError::Codec(format!("blob uri: {e}")))?,
            content_type: content_type.to_owned(),
            size: bytes.len() as u64,
            sha256: Sha256Hex::new(&sha).map_err(|e| StoreError::Codec(format!("sha: {e}")))?,
        })
    }

    async fn get(&self, media: &MediaRef) -> Result<Vec<u8>, StoreError> {
        let Self::R2(bucket) = self else {
            return Err(StoreError::Backend(
                "this edge has no R2 bucket bound — content cannot be fetched here".into(),
            ));
        };
        let sha = media.sha256.as_str();
        let object = bucket
            .get(sha)
            .execute()
            .await
            .map_err(|e| StoreError::Backend(format!("r2 get: {e}")))?
            .ok_or_else(|| {
                StoreError::Backend(format!(
                    "blob {sha} not replicated to this edge — `waggle edge push` uploads it"
                ))
            })?;
        let bytes = object
            .body()
            .ok_or_else(|| StoreError::Backend("r2 body missing".into()))?
            .bytes()
            .await
            .map_err(|e| StoreError::Backend(format!("r2 read: {e}")))?;
        let actual = hex(&Sha256::digest(&bytes));
        if actual != sha {
            return Err(StoreError::Codec(format!(
                "blob {sha} failed integrity at the edge (got {actual}) — re-push from the owner"
            )));
        }
        Ok(bytes)
    }
}
