//! CLI argument definitions for `fontlift`.
//!
//! This module declares the command surface with `clap` derive macros.
//! It does not perform any font operations. `ops.rs` does that.
//!
//! Main types:
//! - [`Cli`] for global flags plus the chosen subcommand.
//! - [`Commands`] for the subcommands.
//! - [`ValidationStrictness`] for install-time validation presets.
//! - [`exit_code_for_clap_error`] for script-friendly clap exit codes.

use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::Shell;
use std::path::PathBuf;

/// How strictly `fontlift install` validates a font before touching the OS.
///
/// Validation runs in a separate process so a malformed font cannot take down
/// `fontlift` itself. These presets trade speed for caution depending on where
/// the font came from and how large it is.
///
/// | Preset | File size cap | Parse timeout | Good for |
/// |---|---|---|---|
/// | `lenient` | 128 MB | 10 s | CJK superfamilies, large variable fonts |
/// | `normal` | 64 MB | 5 s | Everyday use, the default |
/// | `paranoid` | 32 MB | 2 s | Fonts from untrusted sources |
///
/// Use `lenient` for legitimately large CJK families or heavy variable fonts.
/// Use `paranoid` for files you do not fully trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum ValidationStrictness {
    /// 128 MB, 10 s. Best for large CJK families and heavy variable fonts.
    Lenient,
    /// 64 MB, 5 s. Default for most fonts.
    #[default]
    Normal,
    /// 32 MB, 2 s. Best for untrusted files.
    Paranoid,
}

/// Cross-platform font installation and cleanup.
///
/// `install` registers a font with the OS. `uninstall` removes the OS
/// registration but keeps the file. `remove` deregisters the font and deletes
/// the file.
///
/// Global flags apply to every subcommand and can appear anywhere on the line:
///
/// ```sh
/// fontlift --dry-run install MyFont.otf
/// fontlift install --dry-run MyFont.otf   # same thing
/// ```
#[derive(Parser)]
#[command(name = "fontlift")]
#[command(about = "Install, uninstall, list, and remove fonts cross-platform", long_about = None)]
#[command(version = env!("GIT_VERSION"))]
pub struct Cli {
    /// Preview actions without changing files, registrations, or caches.
    #[arg(
        global = true,
        long,
        help = "Print intended actions without mutating fonts"
    )]
    pub dry_run: bool,

    /// Suppress routine output. Only errors are printed.
    #[arg(
        global = true,
        short = 'q',
        long,
        help = "Silence routine status output",
        conflicts_with = "verbose"
    )]
    pub quiet: bool,

    /// Print extra detail such as chosen scope and resolved paths.
    #[arg(
        global = true,
        short = 'v',
        long,
        help = "Show verbose status messages",
        conflicts_with = "quiet"
    )]
    pub verbose: bool,

    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(global = true, short = 'j', long, help = "Output results as JSON")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// The available subcommands.
///
/// Each variant maps to a handler in `ops.rs`. Short aliases such as
/// `fontlift l`, `fontlift i`, and `fontlift rm` are also supported.
#[derive(Subcommand)]
pub enum Commands {
    /// List installed fonts.
    ///
    /// By default this prints one file path per line. Add `--name` to print
    /// PostScript names instead, or combine `--path --name` for
    /// `path::PostScriptName` pairs. `--sorted` produces stable, deduplicated
    /// output for scripts and diffs.
    ///
    /// Examples:
    /// ```sh
    /// fontlift list                    # one path per line
    /// fontlift list --name             # PostScript names only
    /// fontlift list --path --name      # path::name pairs
    /// fontlift list --sorted --json    # deduplicated JSON snapshot
    /// ```
    #[command(alias = "l")]
    List {
        /// Show font file paths. This is the default when neither flag is set.
        #[arg(short, long, help = "Show font file paths")]
        path: bool,

        /// Show PostScript names such as `HelveticaNeue-BoldItalic`.
        ///
        /// This is the face identifier most applications and workflows use
        /// programmatically. It is not the filename or the family name.
        #[arg(short, long, help = "Show PostScript names of installed font faces")]
        name: bool,

        /// Sort output and remove duplicates for stable comparisons.
        #[arg(short, long, help = "Sort output and remove duplicates")]
        sorted: bool,
    },

    /// Install fonts into user or system scope.
    ///
    /// By default, `fontlift` copies each font into the OS font directory for
    /// the chosen scope and then registers it. With `--inplace`, it registers
    /// the file where it already lives. If that file later moves or disappears,
    /// the registration goes stale.
    ///
    /// Directories are scanned one level deep for supported font files.
    ///
    /// Examples:
    /// ```sh
    /// fontlift install MyFont.otf
    /// fontlift install ~/Downloads/fonts/          # install all fonts in dir
    /// fontlift install --admin MyFont.otf          # system-wide (needs sudo)
    /// fontlift install --inplace /opt/fonts/*.otf  # register without copying
    /// fontlift install --validation-strictness lenient BigCJKFamily.otf
    /// fontlift install --no-validate QuickTest.ttf # skip validation entirely
    /// ```
    #[command(alias = "i")]
    Install {
        /// One or more font files or directories to install.
        ///
        /// Directories are scanned one level deep, not recursively.
        #[arg(
            value_name = "FONT|DIR",
            num_args = 1..,
            value_hint = ValueHint::AnyPath,
            help = "Font file(s) or directories to install"
        )]
        font_inputs: Vec<PathBuf>,

        /// Install in system scope for all users.
        ///
        /// On macOS this targets `/Library/Fonts`. Without this flag, install
        /// targets the current user only.
        #[arg(
            short,
            long,
            help = "Install system-wide for all users (requires admin privileges)"
        )]
        admin: bool,

        /// Skip the out-of-process validator before install.
        #[arg(short = 'V', long, help = "Skip font validation before installing")]
        no_validate: bool,

        /// Validation preset to use before install.
        ///
        /// See [`ValidationStrictness`]. `lenient` suits very large fonts.
        /// `paranoid` suits untrusted files.
        #[arg(
            long,
            value_enum,
            default_value = "normal",
            help = "Validation strictness: lenient | normal | paranoid"
        )]
        validation_strictness: ValidationStrictness,

        /// Copy into the font directory before registering.
        ///
        /// This is the default even when the flag is omitted. The flag mainly
        /// exists so scripts can be explicit.
        #[arg(
            short = 'c',
            long,
            help = "Copy font to the fonts directory then register (default behaviour)",
            conflicts_with = "inplace"
        )]
        copy: bool,

        /// Register the font where it already lives, without copying it.
        ///
        /// If the file later moves or is deleted, the registration becomes
        /// stale. `fontlift cleanup` can prune those stale entries.
        #[arg(
            short = 'i',
            long,
            help = "Register font at its current path without copying",
            conflicts_with = "copy"
        )]
        inplace: bool,
    },

    /// Unregister a font while leaving the file on disk.
    ///
    /// Target by path, or by `--name`, which matches a PostScript name or a
    /// full name. `fontlift` tries the preferred scope first, then falls back
    /// to the other scope.
    ///
    /// Examples:
    /// ```sh
    /// fontlift uninstall ~/Library/Fonts/MyFont.otf
    /// fontlift uninstall --name HelveticaNeue-Bold
    /// fontlift uninstall --admin /Library/Fonts/MyFont.otf
    /// ```
    #[command(alias = "u")]
    Uninstall {
        /// Use a PostScript name or full name instead of a file path.
        #[arg(short, long, help = "PostScript or full name of the font to uninstall")]
        name: Option<String>,

        /// Font files or directories whose fonts should be uninstalled.
        #[arg(
            value_name = "FONT|DIR",
            num_args = 0..,
            value_hint = ValueHint::AnyPath,
            help = "Font file(s) or directories to uninstall"
        )]
        font_inputs: Vec<PathBuf>,

        #[arg(
            short,
            long,
            help = "Uninstall from system scope (requires admin privileges)"
        )]
        admin: bool,
    },

    /// Unregister a font and delete its file.
    ///
    /// This is the destructive counterpart to `uninstall`. If deregistration
    /// fails, `fontlift` still tries to delete the file so the font is gone
    /// from disk.
    ///
    /// Use `--dry-run` first to see exactly what will be deleted.
    ///
    /// Examples:
    /// ```sh
    /// fontlift remove ~/Library/Fonts/OldFont.otf
    /// fontlift remove --name OldFont-Regular
    /// fontlift --dry-run remove ~/Library/Fonts/OldFont.otf
    /// ```
    #[command(alias = "rm")]
    Remove {
        /// Use a PostScript name or full name instead of a file path.
        #[arg(short, long, help = "PostScript or full name of the font to remove")]
        name: Option<String>,

        /// Font files or directories whose fonts should be removed.
        #[arg(
            value_name = "FONT|DIR",
            num_args = 0..,
            value_hint = ValueHint::AnyPath,
            help = "Font file(s) or directories to remove"
        )]
        font_inputs: Vec<PathBuf>,

        #[arg(
            short,
            long,
            help = "Remove from system scope (requires admin privileges)"
        )]
        admin: bool,
    },

    /// Prune stale registrations, clear font caches, or both.
    ///
    /// Stale registrations point at files that no longer exist. Cache clearing
    /// asks the OS, and common font-heavy apps where supported, to rescan fonts.
    /// By default both steps run.
    ///
    /// Examples:
    /// ```sh
    /// fontlift cleanup                # prune + clear caches (user scope)
    /// fontlift cleanup --prune-only   # remove stale registrations only
    /// fontlift cleanup --cache-only   # rebuild caches only
    /// fontlift cleanup --admin        # include system-wide cleanup
    /// fontlift --dry-run cleanup      # preview without changing anything
    /// ```
    #[command(alias = "c")]
    Cleanup {
        /// Include system-wide registrations and caches.
        #[arg(
            short,
            long,
            help = "Include system-wide cleanup (requires admin privileges)"
        )]
        admin: bool,

        /// Prune stale registrations only.
        #[arg(
            short = 'p',
            long,
            help = "Prune stale registrations only; skip cache clearing",
            conflicts_with = "cache_only"
        )]
        prune_only: bool,

        /// Clear font caches only.
        #[arg(
            short = 'C',
            long,
            help = "Clear font caches only; skip pruning stale registrations",
            conflicts_with = "prune_only"
        )]
        cache_only: bool,
    },

    /// Print a shell completion script to stdout.
    ///
    /// Examples:
    /// ```sh
    /// # bash
    /// fontlift completions bash >> ~/.bashrc
    ///
    /// # zsh (with a completions directory on $fpath)
    /// fontlift completions zsh > ~/.zsh/completions/_fontlift
    ///
    /// # fish
    /// fontlift completions fish > ~/.config/fish/completions/fontlift.fish
    /// ```
    Completions {
        /// The shell to generate completions for.
        #[arg(value_enum, help = "Shell to generate completions for")]
        shell: Shell,
    },

    /// Inspect the crash-recovery journal and continue interrupted work.
    ///
    /// `fontlift` records multi-step operations, such as copy then register.
    /// If the process stops halfway through, `doctor` shows the unfinished
    /// steps and attempts recovery.
    ///
    /// Run with `--preview` (or `--dry-run`) first to see what recovery would
    /// do before committing to it.
    ///
    /// Examples:
    /// ```sh
    /// fontlift doctor             # show and attempt recovery
    /// fontlift doctor --preview   # show incomplete ops without recovering
    /// ```
    #[command(alias = "d")]
    Doctor {
        /// Show the recovery plan without changing anything.
        #[arg(short = 'P', long, help = "Show recovery plan without executing it")]
        preview: bool,
    },
}

/// Map clap outcomes to script-friendly exit codes.
///
/// `--help` and `--version` succeed with exit code 0. Other clap failures are
/// user errors and exit 1.
pub fn exit_code_for_clap_error(kind: ErrorKind) -> i32 {
    match kind {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
        _ => 1,
    }
}
