fn main() {
    // Set GIT_VERSION from git tags for the Python __version__ attribute
    let version = std::process::Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().trim_start_matches('v').to_string())
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".into()));

    println!("cargo:rustc-env=GIT_VERSION={version}");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/tags");

    // Only adjust linker args when building the actual Python extension.
    if std::env::var_os("CARGO_FEATURE_PYTHON_BINDINGS").is_some() {
        // Ensure macOS builds use -undefined dynamic_lookup so Python symbols
        // are resolved at import time instead of link time.
        pyo3_build_config::add_extension_module_link_args();
    }
}
