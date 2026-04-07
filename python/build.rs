fn main() {
    // Only adjust linker args when building the actual Python extension.
    if std::env::var_os("CARGO_FEATURE_PYTHON_BINDINGS").is_some() {
        // Ensure macOS builds use -undefined dynamic_lookup so Python symbols
        // are resolved at import time instead of link time.
        pyo3_build_config::add_extension_module_link_args();
    }
}
