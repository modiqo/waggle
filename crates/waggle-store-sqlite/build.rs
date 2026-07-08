//! Build script: declares the ad-hoc `loom` cfg (set by `just loom` via
//! `RUSTFLAGS="--cfg loom"`) so `unexpected_cfgs` stays honest.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(loom)");
}
