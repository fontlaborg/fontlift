//! Entrypoint for the fontlift Python extension crate.
//!
//! # Two build modes
//!
//! This crate compiles differently depending on whether the `python-bindings`
//! Cargo feature is active:
//!
//! | Feature on? | What compiles | Who sets it |
//! |-------------|---------------|-------------|
//! | Yes | `bindings.rs` — real PyO3 extension, produces the `_native` Python module | `maturin` |
//! | No  | `stub.rs` — a tiny stand-in with no Python dependency | `cargo test --workspace` |
//!
//! ## Why the stub exists
//!
//! PyO3 needs a live Python installation to link against. CI machines and
//! developer laptops running `cargo test --workspace` often have neither Python
//! nor the matching `libpython`. The stub satisfies the Rust type-checker and
//! lets workspace tests pass without requiring a Python toolchain.
//!
//! ## How maturin uses the real bindings
//!
//! When you run `maturin develop` or `maturin build`, maturin passes
//! `--features python-bindings` to Cargo. That switches this crate to compile
//! `bindings.rs` and link against the active Python interpreter, producing the
//! `.so` / `.pyd` file that Python imports as `fontlift._native`.

#![allow(dead_code)]

#[cfg(feature = "python-bindings")]
mod bindings;
#[cfg(not(feature = "python-bindings"))]
mod stub;

#[cfg(feature = "python-bindings")]
pub use bindings::*;
#[cfg(not(feature = "python-bindings"))]
pub use stub::*;

#[cfg(test)]
mod feature_flags {
    use super::*;

    #[cfg(feature = "python-bindings")]
    #[test]
    fn bindings_feature_flag_true() {
        assert!(PYTHON_BINDINGS_ENABLED);
    }

    #[cfg(not(feature = "python-bindings"))]
    #[test]
    fn bindings_feature_flag_false() {
        assert!(!PYTHON_BINDINGS_ENABLED);
    }
}
