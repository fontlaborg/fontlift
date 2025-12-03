//! Stub implementation used when the `python-bindings` feature is disabled.
//! This keeps workspace builds working on hosts without a Python toolchain.

pub const PYTHON_BINDINGS_ENABLED: bool = false;

/// Explain why the bindings are unavailable in this build.
pub fn bindings_disabled_reason() -> &'static str {
    "fontlift-python built without `python-bindings`; enable the feature (used by maturin) to compile the PyO3 extension."
}
