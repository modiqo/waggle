#![allow(missing_docs)] // criterion macros generate undocumented items

//! Guided-query budget (design doc `14 CP-7`): a slice under budget from
//! a large document, fast enough to be the default access path.

use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::json;
use std::hint::black_box;
use waggle_mcp::query::slice_at;

fn big_doc() -> serde_json::Value {
    let variants: Vec<_> = (0..50)
        .map(|i| {
            json!({ "match": { "model-family": format!("fam{i}") },
                          "body": "x".repeat(2000) })
        })
        .collect();
    let children: Vec<_> = (0..500).map(|i| format!("child{i:04}")).collect();
    json!({
        "manifest": { "token": "bench123", "variants": variants },
        "funnel": { "resolve": 421, "run": 77, "repeat": 12 },
        "children": children,
    })
}

fn bench_query(c: &mut Criterion) {
    let doc = big_doc();
    c.bench_function("query_slice_root_4k", |b| {
        b.iter(|| slice_at(black_box(&doc), "", 4096).unwrap());
    });
    c.bench_function("query_slice_deep", |b| {
        b.iter(|| slice_at(black_box(&doc), "/manifest/variants/25/body", 4096).unwrap());
    });
}

criterion_group!(benches, bench_query);
criterion_main!(benches);
