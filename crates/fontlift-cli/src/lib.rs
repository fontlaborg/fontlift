//! CLI interface for fontlift.
//! Lean modules keep the public API stable while trimming this crate into readable pieces.

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

/// Main CLI handler
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
        } => {
            handle_install_command(
                manager,
                font_inputs,
                admin,
                !no_validate,
                validation_strictness,
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

/// CLI entry point
#[tokio::main]
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
