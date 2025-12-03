//! Python bindings entrypoint
//!
//! The real PyO3 bindings live in `bindings.rs` and are only compiled when
//! the `python-bindings` feature is enabled. Without that feature, we build a
//! stub so `cargo test --workspace` can run on hosts without a Python toolchain.

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
