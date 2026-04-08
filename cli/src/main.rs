//! Binary entry point for `fontlift`, the cross-platform font management CLI.
//!
//! This file is intentionally tiny. Its only job is to hand control to
//! [`fontlift_cli::main`], which lives in `lib.rs` so the real logic stays
//! testable without spinning up a process.
//!
//! # Installation
//!
//! ```sh
//! cargo install fontlift-cli
//! ```
//!
//! After that, `fontlift --help` shows available subcommands. Everything
//! interesting — argument parsing, platform dispatch, command handlers — lives
//! in `lib.rs` and `ops.rs`.

#[tokio::main]
async fn main() {
    fontlift_cli::main().await;
}
