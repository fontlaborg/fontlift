//! Top-level orchestrator for the `fontlift` CLI.
//!
//! This crate wires together two modules:
//!
//! - **`args`** — argument definitions via `clap` derive macros. Every flag,
//!   subcommand, and enum variant lives there.
//! - **`ops`** — the actual command implementations: install, uninstall, list,
//!   remove, cleanup, doctor, completions.
//!
//! # Entry points
//!
//! | Function | Purpose |
//! |---|---|
//! | [`run_cli`] | Parse-then-dispatch, returns `Result`. Use this in tests. |
//! | [`main`] | Binary entry point: calls `run_cli`, maps errors to exit codes. |
//!
//! Keeping `run_cli` separate from `main` means integration tests can drive the
//! full command dispatch without forking a process or catching `process::exit`.

mod args;
mod ops;

pub use args::{exit_code_for_clap_error, Cli, Commands, ValidationStrictness};
pub use ops::{
    collect_font_inputs, create_font_manager, handle_cleanup_command, handle_doctor_command,
    handle_install_command, handle_list_command, handle_remove_command, handle_uninstall_command,
    render_list_output, write_completions, ListRender, ListRenderOptions, OperationOptions,
    OutputOptions,
};

use clap::Parser;
use fontlift_core::FontError;

/// Parse a fully constructed [`Cli`] and dispatch to the right command handler.
///
/// Returns `Ok(())` on success or a [`FontError`] on failure. The caller
/// decides what to do with the error — the binary entry point ([`main`]) prints
/// it and exits with code 1; tests can inspect it directly.
///
/// This is the function to call from integration tests:
///
/// ```rust,no_run
/// # use fontlift_cli::{Cli, run_cli};
/// # use clap::Parser;
/// let cli = Cli::parse_from(["fontlift", "list"]);
/// // run_cli(cli).await?;
/// ```
pub async fn run_cli(cli: Cli) -> Result<(), FontError> {
    let manager = create_font_manager();
    let op_opts = OperationOptions::new(cli.dry_run, cli.quiet, cli.verbose);

    match cli.command {
        Commands::List { path, name, sorted } => {
            handle_list_command(manager, path, name, sorted, cli.json).await?;
        }
        Commands::Install {
            font_inputs,
            admin,
            no_validate,
            validation_strictness,
            copy: _,
            inplace,
        } => {
            handle_install_command(
                manager,
                font_inputs,
                admin,
                !no_validate,
                validation_strictness,
                inplace,
                op_opts,
            )
            .await?;
        }
        Commands::Uninstall {
            name,
            font_inputs,
            admin,
        } => {
            handle_uninstall_command(manager, name, font_inputs, admin, op_opts).await?;
        }
        Commands::Remove {
            name,
            font_inputs,
            admin,
        } => {
            handle_remove_command(manager, name, font_inputs, admin, op_opts).await?;
        }
        Commands::Cleanup {
            admin,
            prune_only,
            cache_only,
        } => {
            handle_cleanup_command(manager, admin, prune_only, cache_only, op_opts).await?;
        }
        Commands::Completions { shell } => {
            write_completions(shell, std::io::stdout())?;
        }
        Commands::Doctor { preview } => {
            handle_doctor_command(preview, op_opts).await?;
        }
    }

    Ok(())
}

/// Binary entry point: initialize logging, parse args, run, exit.
///
/// `env_logger::init()` activates the `RUST_LOG` environment variable for
/// log-level filtering. Set `RUST_LOG=debug` to see verbose internal traces,
/// or `RUST_LOG=fontlift_core=trace` to scope it to the core library.
///
/// Clap parse errors are handled here rather than in [`run_cli`] because they
/// need special exit code treatment: `--help` and `--version` exit 0 (success),
/// while genuine argument errors exit 1. See [`exit_code_for_clap_error`].
pub async fn main() {
    env_logger::init();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let code = exit_code_for_clap_error(err.kind());
            let _ = err.print();
            std::process::exit(code);
        }
    };

    if let Err(e) = run_cli(cli).await {
        eprintln!("❌ Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
