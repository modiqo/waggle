#![allow(missing_docs)] // criterion macros generate undocumented items

//! The doc-20 §4 budget gates: extraction ≤ 5 ms/kLOC on one core;
//! rendering in microseconds. Run: `cargo bench -p waggle-lens-code`.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use waggle_lens_code::{extract, Lang};

/// ~1 kLOC of representative Rust: nested impls, methods, free functions.
fn rust_1kloc() -> String {
    use std::fmt::Write as _;
    let mut src = String::new();
    for i in 0..55 {
        let _ = write!(
            src,
            "pub struct Type{i} {{ field: u32 }}\n\n\
             impl Type{i} {{\n\
             \tpub fn method_a(&self) -> u32 {{ self.field + {i} }}\n\
             \tpub fn method_b(&self, x: u32) -> u32 {{\n\
             \t\tlet y = x * 2;\n\
             \t\tlet z = y + self.field;\n\
             \t\tz\n\
             \t}}\n\
             }}\n\n\
             fn helper_{i}(a: u32, b: u32) -> u32 {{\n\
             \tlet c = a + b;\n\
             \tlet d = c * 2;\n\
             \tif d > 10 {{ d }} else {{ c }}\n\
             }}\n\n\
             const CONST_{i}: u32 = {i};\n\n"
        );
    }
    src
}

fn benches(c: &mut Criterion) {
    let src = rust_1kloc();
    assert!(src.lines().count() >= 900, "bench corpus is ~1 kLOC");
    let outline = extract(&src, Lang::Rust);
    assert!(outline.len() >= 160, "corpus yields a real outline");

    // Gate: ≤ 5 ms per kLOC (doc 20 §4). Criterion reports; CI eyeballs.
    c.bench_function("extract_rs_1kloc", |b| {
        b.iter(|| extract(black_box(&src), Lang::Rust));
    });

    // The mint-side serialize (the serve side lives in waggle-mcp).
    c.bench_function("outline_to_wire", |b| {
        b.iter(|| black_box(&outline).to_wire());
    });
}

criterion_group!(lens, benches);
criterion_main!(lens);
