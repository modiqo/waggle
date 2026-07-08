fn main() {
    // `loom` is an ad-hoc cfg set by `just loom` (RUSTFLAGS="--cfg loom")
    // for the model-checked cache suite; declare it so unexpected_cfgs
    // stays honest everywhere else.
    println!("cargo::rustc-check-cfg=cfg(loom)");
}
