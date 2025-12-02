use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use std::path::PathBuf;

/// Font management CLI tool
#[derive(Parser)]
#[command(name = "fontlift")]
#[command(about = "Install, uninstall, list, and remove fonts cross-platform", long_about = None)]
#[command(version = "2.0.0-dev")]
pub struct Cli {
    /// Simulate actions without changing system state
    #[arg(
        global = true,
        long,
        help = "Print intended actions without mutating fonts"
    )]
    pub dry_run: bool,

    /// Reduce output to errors only
    #[arg(
        global = true,
        long,
        help = "Silence routine status output",
        conflicts_with = "verbose"
    )]
    pub quiet: bool,

    /// Show additional status output
    #[arg(
        global = true,
        long,
        help = "Show verbose status messages",
        conflicts_with = "quiet"
    )]
    pub verbose: bool,

    /// Output as JSON (deterministic ordering)
    #[arg(global = true, long, help = "Output results as JSON")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List installed fonts
    #[command(alias = "l")]
    List {
        #[arg(short, long, help = "Show font file paths")]
        path: bool,

        #[arg(short, long, help = "Show internal font names")]
        name: bool,

        #[arg(short, long, help = "Remove duplicates; output is always sorted")]
        sorted: bool,
    },

    /// Install fonts from file paths
    #[command(alias = "i")]
    Install {
        /// Font file path(s) or directory/ies containing fonts
        #[arg(
            value_name = "FONT|DIR",
            num_args = 1..,
            value_hint = ValueHint::AnyPath,
            help = "Font file(s) or directory/ies to install; directories are scanned for font files"
        )]
        font_inputs: Vec<PathBuf>,

        #[arg(
            short,
            long,
            help = "Install at system level (all users, requires admin)"
        )]
        admin: bool,
    },

    /// Uninstall fonts (keeping files)
    #[command(alias = "u")]
    Uninstall {
        #[arg(short, long, help = "Font name to uninstall")]
        name: Option<String>,

        /// Font file path(s) or directory/ies containing fonts
        #[arg(
            value_name = "FONT|DIR",
            num_args = 0..,
            value_hint = ValueHint::AnyPath,
            help = "Font file(s) or directory/ies to uninstall; directories are scanned for font files"
        )]
        font_inputs: Vec<PathBuf>,

        #[arg(
            short,
            long,
            help = "Uninstall at system level (all users, requires admin)"
        )]
        admin: bool,
    },

    /// Remove fonts (uninstall and delete files)
    #[command(alias = "rm")]
    Remove {
        #[arg(short, long, help = "Font name to remove")]
        name: Option<String>,

        /// Font file path(s) or directory/ies containing fonts
        #[arg(
            value_name = "FONT|DIR",
            num_args = 0..,
            value_hint = ValueHint::AnyPath,
            help = "Font file(s) or directory/ies to remove; directories are scanned for font files"
        )]
        font_inputs: Vec<PathBuf>,

        #[arg(
            short,
            long,
            help = "Remove at system level (all users, requires admin)"
        )]
        admin: bool,
    },

    /// Cleanup registry entries and font caches
    #[command(alias = "c")]
    Cleanup {
        #[arg(short, long, help = "Include system-wide cleanup (requires admin)")]
        admin: bool,

        #[arg(
            long,
            help = "Only prune stale registrations; skip cache clearing",
            conflicts_with = "cache_only"
        )]
        prune_only: bool,

        #[arg(
            long,
            help = "Only clear caches; skip pruning stale registrations",
            conflicts_with = "prune_only"
        )]
        cache_only: bool,
    },

    /// Generate shell completions
    Completions {
        /// Target shell (bash, zsh, fish, powershell, elvish)
        #[arg(value_enum, help = "Shell to generate completions for")]
        shell: Shell,
    },
}

/// Map clap error kinds to legacy exit codes (0 for help/version, 1 for other errors)
pub fn exit_code_for_clap_error(kind: ErrorKind) -> i32 {
    match kind {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
        _ => 1,
    }
}
