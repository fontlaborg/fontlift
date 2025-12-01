//! CLI interface for fontlift
//!
//! This module provides the command-line interface that mirrors the functionality
//! of the existing Swift and CLI implementations.

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{generate, Shell};
use fontlift_core::{protection, validation, FontError, FontInfo, FontManager, FontScope};
use serde_json::to_string_pretty;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

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

        #[arg(short, long, help = "Sort output and remove duplicates")]
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

/// List rendering options resolved from CLI flags
#[derive(Debug, Clone, Copy)]
pub struct ListRenderOptions {
    pub show_path: bool,
    pub show_name: bool,
    pub sorted: bool,
    pub json: bool,
}

/// Possible render outputs for list command
#[derive(Debug, PartialEq)]
pub enum ListRender {
    Lines(Vec<String>),
    Json(String),
}

/// Output controls for CLI commands
#[derive(Debug, Clone, Copy)]
pub struct OutputOptions {
    pub quiet: bool,
    pub verbose: bool,
}

impl OutputOptions {
    pub fn should_print(&self) -> bool {
        !self.quiet
    }

    pub fn should_print_verbose(&self) -> bool {
        self.verbose && !self.quiet
    }
}

/// Execution controls shared by mutating commands
#[derive(Debug, Clone, Copy)]
pub struct OperationOptions {
    pub dry_run: bool,
    pub output: OutputOptions,
}

impl OperationOptions {
    pub fn new(dry_run: bool, quiet: bool, verbose: bool) -> Self {
        Self {
            dry_run,
            output: OutputOptions { quiet, verbose },
        }
    }
}

fn log_status(opts: &OperationOptions, message: &str) {
    if opts.output.should_print() {
        println!("{}", message);
    }
}

fn log_verbose(opts: &OperationOptions, message: &str) {
    if opts.output.should_print_verbose() {
        eprintln!("{}", message);
    }
}

/// Prepare list output according to options (sorting/deduplication is deterministic)
pub fn render_list_output(
    mut fonts: Vec<FontInfo>,
    opts: ListRenderOptions,
) -> Result<ListRender, FontError> {
    let must_sort = opts.sorted || opts.json;

    if must_sort {
        fonts = protection::dedupe_fonts(fonts);
    }

    if opts.json {
        let json = to_string_pretty(&fonts).map_err(|e| {
            FontError::InvalidFormat(format!("Failed to serialize font list to JSON: {}", e))
        })?;
        return Ok(ListRender::Json(json));
    }

    // Default to showing paths if no flags specified
    let show_path = opts.show_path || !opts.show_name;
    let show_name = opts.show_name;

    let mut lines = Vec::new();
    for font in fonts {
        if show_path && show_name {
            lines.push(format!("{}::{}", font.path.display(), font.postscript_name));
        } else if show_path {
            lines.push(font.path.display().to_string());
        } else {
            lines.push(font.postscript_name);
        }
    }

    if must_sort {
        lines.sort();
        lines.dedup();
    }

    Ok(ListRender::Lines(lines))
}

/// Expand user-provided font inputs (files or directories) into a unique, sorted list of font files
pub fn collect_font_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, FontError> {
    if inputs.is_empty() {
        return Err(FontError::InvalidFormat(
            "At least one font path or directory is required".to_string(),
        ));
    }

    let mut found: BTreeSet<PathBuf> = BTreeSet::new();

    for input in inputs {
        if input.is_dir() {
            for entry in fs::read_dir(input).map_err(FontError::IoError)? {
                let entry = entry.map_err(FontError::IoError)?;
                let path = entry.path();
                if path.is_file() && validation::is_valid_font_extension(&path) {
                    found.insert(path);
                }
            }
        } else if input.is_file() {
            if validation::is_valid_font_extension(input) {
                found.insert(input.clone());
            } else {
                return Err(FontError::InvalidFormat(format!(
                    "Invalid font extension: {}",
                    input.display()
                )));
            }
        } else {
            return Err(FontError::FontNotFound(input.clone()));
        }
    }

    if found.is_empty() {
        return Err(FontError::InvalidFormat(
            "No font files found in provided paths".to_string(),
        ));
    }

    Ok(found.into_iter().collect())
}

/// Create the appropriate font manager for the current platform
pub fn create_font_manager() -> Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(fontlift_platform_mac::MacFontManager::new())
    }

    #[cfg(target_os = "windows")]
    {
        Arc::new(fontlift_platform_win::WinFontManager::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Linux support not yet implemented");
    }
}

/// Generate shell completion script for the given shell
pub fn write_completions<W: Write>(shell: Shell, mut writer: W) -> Result<(), FontError> {
    let mut command = Cli::command();
    let bin_name = command.get_name().to_string();

    generate(shell, &mut command, bin_name.as_str(), &mut writer);

    Ok(())
}

/// Handle the list command
pub async fn handle_list_command(
    manager: Arc<dyn FontManager>,
    path: bool,
    name: bool,
    sorted: bool,
    json: bool,
) -> Result<(), FontError> {
    let fonts = manager.list_installed_fonts()?;
    let opts = ListRenderOptions {
        show_path: path,
        show_name: name,
        sorted,
        json,
    };

    match render_list_output(fonts, opts)? {
        ListRender::Lines(lines) => {
            for line in lines {
                println!("{}", line);
            }
        }
        ListRender::Json(json) => {
            println!("{}", json);
        }
    }

    Ok(())
}

/// Handle the install command
pub async fn handle_install_command(
    manager: Arc<dyn FontManager>,
    font_inputs: Vec<PathBuf>,
    admin: bool,
    opts: OperationOptions,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    let targets = collect_font_inputs(&font_inputs)?;

    for path in targets {
        log_verbose(&opts, &format!("Scope: {}", scope.description()));
        if opts.dry_run {
            log_status(
                &opts,
                &format!(
                    "DRY-RUN: would install font {} ({})",
                    path.display(),
                    scope.description()
                ),
            );
            continue;
        }

        log_status(&opts, &format!("Installing font from: {}", path.display()));
        manager.install_font(&path, scope)?;
        log_status(&opts, "✅ Successfully installed font");
    }

    Ok(())
}

/// Handle the uninstall command
pub async fn handle_uninstall_command(
    manager: Arc<dyn FontManager>,
    name: Option<String>,
    font_inputs: Vec<PathBuf>,
    admin: bool,
    opts: OperationOptions,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    if let Some(font_name) = name {
        log_status(&opts, &format!("Uninstalling font by name: {}", font_name));

        // Find font by name in installed fonts
        let installed_fonts = manager.list_installed_fonts()?;
        if let Some(font) = installed_fonts
            .iter()
            .find(|f| f.postscript_name == font_name || f.full_name == font_name)
        {
            if opts.dry_run {
                log_status(
                    &opts,
                    &format!(
                        "DRY-RUN: would uninstall '{}' at {}",
                        font_name,
                        font.path.display()
                    ),
                );
            } else {
                manager.uninstall_font(&font.path, scope)?;
                log_status(
                    &opts,
                    &format!("✅ Successfully uninstalled font '{}'", font_name),
                );
            }
        } else {
            return Err(FontError::FontNotFound(PathBuf::from(font_name)));
        }
    } else {
        let targets = collect_font_inputs(&font_inputs)?;
        for path in targets {
            if opts.dry_run {
                log_status(
                    &opts,
                    &format!(
                        "DRY-RUN: would uninstall font at {} ({})",
                        path.display(),
                        scope.description()
                    ),
                );
                continue;
            }

            log_status(
                &opts,
                &format!("Uninstalling font from path: {}", path.display()),
            );

            manager.uninstall_font(&path, scope)?;
            log_status(&opts, "✅ Successfully uninstalled font");
        }
    }

    Ok(())
}

/// Handle the remove command
pub async fn handle_remove_command(
    manager: Arc<dyn FontManager>,
    name: Option<String>,
    font_inputs: Vec<PathBuf>,
    admin: bool,
    opts: OperationOptions,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    if let Some(font_name) = name {
        log_status(&opts, &format!("Removing font by name: {}", font_name));

        // Find font by name in installed fonts
        let installed_fonts = manager.list_installed_fonts()?;
        if let Some(font) = installed_fonts
            .iter()
            .find(|f| f.postscript_name == font_name || f.full_name == font_name)
        {
            if opts.dry_run {
                log_status(
                    &opts,
                    &format!(
                        "DRY-RUN: would remove '{}' at {}",
                        font_name,
                        font.path.display()
                    ),
                );
            } else {
                manager.remove_font(&font.path, scope)?;
                log_status(
                    &opts,
                    &format!("✅ Successfully removed font '{}'", font_name),
                );
            }
        } else {
            return Err(FontError::FontNotFound(PathBuf::from(font_name)));
        }
    } else {
        let targets = collect_font_inputs(&font_inputs)?;
        for path in targets {
            if opts.dry_run {
                log_status(
                    &opts,
                    &format!(
                        "DRY-RUN: would remove font at {} ({})",
                        path.display(),
                        scope.description()
                    ),
                );
                continue;
            }

            log_status(
                &opts,
                &format!("Removing font from path: {}", path.display()),
            );

            manager.remove_font(&path, scope)?;
            log_status(&opts, "✅ Successfully removed font");
        }
    }

    Ok(())
}

/// Handle the cleanup command
pub async fn handle_cleanup_command(
    manager: Arc<dyn FontManager>,
    admin: bool,
    opts: OperationOptions,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    log_status(
        &opts,
        &format!(
            "Starting {} cleanup...",
            if admin { "system" } else { "user" }
        ),
    );

    if opts.dry_run {
        log_status(
            &opts,
            &format!("DRY-RUN: would clear font caches ({})", scope.description()),
        );
        return Ok(());
    }

    manager.clear_font_caches(scope)?;
    log_status(&opts, "✅ Successfully cleared font caches");

    Ok(())
}

/// Main CLI handler
pub async fn run_cli(cli: Cli) -> Result<(), FontError> {
    let manager = create_font_manager();
    let op_opts = OperationOptions::new(cli.dry_run, cli.quiet, cli.verbose);

    match cli.command {
        Commands::List { path, name, sorted } => {
            handle_list_command(manager, path, name, sorted, cli.json).await?;
        }
        Commands::Install { font_inputs, admin } => {
            handle_install_command(manager, font_inputs, admin, op_opts).await?;
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
        Commands::Cleanup { admin } => {
            handle_cleanup_command(manager, admin, op_opts).await?;
        }
        Commands::Completions { shell } => {
            write_completions(shell, std::io::stdout())?;
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
mod tests {
    use super::*;
    use fontlift_core::FontInfo;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tokio::runtime::Runtime;

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;

        let cli = Cli::try_parse_from(&["fontlift", "list", "-p"]).unwrap();
        match cli.command {
            Commands::List { path, name, sorted } => {
                assert!(path);
                assert!(!name);
                assert!(!sorted);
            }
            _ => panic!("Expected list command"),
        }
    }

    fn sample_font(path: &str, postscript: &str) -> FontInfo {
        FontInfo::new(
            PathBuf::from(path),
            postscript.to_string(),
            postscript.to_string(),
            "Family".to_string(),
            "Regular".to_string(),
        )
    }

    #[test]
    fn list_renders_json_sorted_and_deduped() {
        let fonts = vec![
            sample_font("/fonts/Zeta.ttf", "Zeta"),
            sample_font("/fonts/Alpha.ttf", "Alpha-Regular"),
            sample_font("/fonts/Alpha.ttf", "Alpha-Regular"), // duplicate
            sample_font("/fonts/Beta.ttf", "Beta-Bold"),
        ];

        let opts = ListRenderOptions {
            show_path: true,
            show_name: true,
            sorted: true,
            json: true,
        };

        let output = render_list_output(fonts, opts).expect("render");

        let json = match output {
            ListRender::Json(s) => s,
            _ => panic!("expected json output"),
        };

        let parsed: Vec<Value> = serde_json::from_str(&json).expect("valid json");
        let names: Vec<&str> = parsed
            .iter()
            .map(|v| v["postscript_name"].as_str().unwrap())
            .collect();

        assert_eq!(
            names,
            vec!["Alpha-Regular", "Beta-Bold", "Zeta"],
            "sorted deterministically with duplicates removed"
        );
    }

    #[test]
    fn list_renders_lines_sorted_and_deduped() {
        let fonts = vec![
            sample_font("/fonts/Beta.ttf", "Beta-Bold"),
            sample_font("/fonts/Alpha.ttf", "Alpha-Regular"),
            sample_font("/fonts/Alpha.ttf", "Alpha-Regular"),
        ];

        let opts = ListRenderOptions {
            show_path: true,
            show_name: false,
            sorted: true,
            json: false,
        };

        let output = render_list_output(fonts, opts).expect("render");
        let lines = match output {
            ListRender::Lines(lines) => lines,
            _ => panic!("expected line output"),
        };

        assert_eq!(
            lines,
            vec![
                "/fonts/Alpha.ttf".to_string(),
                "/fonts/Beta.ttf".to_string()
            ],
            "dedupes identical paths and sorts deterministically"
        );
    }

    #[test]
    fn collect_font_inputs_scans_directories_and_dedupes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let alpha = tmp.path().join("Alpha.ttf");
        let beta = tmp.path().join("Beta.otf");
        fs::write(&alpha, b"test").expect("write alpha");
        fs::write(&beta, b"test").expect("write beta");

        // Provide both a directory and a direct file reference to ensure deduplication
        let inputs = vec![tmp.path().to_path_buf(), beta.clone()];
        let collected = collect_font_inputs(&inputs).expect("collect");

        assert_eq!(collected, vec![alpha.clone(), beta.clone()]);
    }

    #[derive(Default)]
    struct RecordingManager {
        installs: Mutex<Vec<(PathBuf, FontScope)>>,
    }

    impl FontManager for RecordingManager {
        fn install_font(
            &self,
            path: &std::path::Path,
            scope: FontScope,
        ) -> fontlift_core::FontResult<()> {
            self.installs
                .lock()
                .expect("lock")
                .push((path.to_path_buf(), scope));
            Ok(())
        }

        fn uninstall_font(
            &self,
            _path: &std::path::Path,
            _scope: FontScope,
        ) -> fontlift_core::FontResult<()> {
            Ok(())
        }

        fn remove_font(
            &self,
            _path: &std::path::Path,
            _scope: FontScope,
        ) -> fontlift_core::FontResult<()> {
            Ok(())
        }

        fn is_font_installed(&self, _path: &std::path::Path) -> fontlift_core::FontResult<bool> {
            Ok(false)
        }

        fn list_installed_fonts(&self) -> fontlift_core::FontResult<Vec<FontInfo>> {
            Ok(Vec::new())
        }

        fn clear_font_caches(&self, _scope: FontScope) -> fontlift_core::FontResult<()> {
            Ok(())
        }
    }

    #[test]
    fn dry_run_install_skips_invoking_manager() {
        let runtime = Runtime::new().expect("runtime");
        let tmp = tempfile::tempdir().expect("tempdir");
        let font = tmp.path().join("DryRun.ttf");
        fs::write(&font, b"test").expect("write font");

        let manager = Arc::new(RecordingManager::default());
        let opts = OperationOptions::new(true, true, false);

        runtime
            .block_on(handle_install_command(
                manager.clone(),
                vec![font.clone()],
                false,
                opts,
            ))
            .expect("dry run install");

        assert!(
            manager.installs.lock().expect("lock").is_empty(),
            "dry-run should not call install_font"
        );
    }

    #[test]
    fn completions_include_core_commands() {
        let mut buffer = Vec::new();

        write_completions(Shell::Bash, &mut buffer).expect("completions");

        let script = String::from_utf8(buffer).expect("utf8");
        assert!(
            script.contains("list"),
            "expected list command in completions"
        );
        assert!(
            script.contains("install"),
            "expected install command in completions"
        );
    }

    #[test]
    fn subcommand_aliases_match_legacy() {
        // list alias
        let cli = Cli::try_parse_from(["fontlift", "l"]).expect("alias l");
        assert!(matches!(cli.command, Commands::List { .. }));

        // install alias
        let cli = Cli::try_parse_from(["fontlift", "i", "font.ttf"]).expect("alias i");
        assert!(matches!(cli.command, Commands::Install { .. }));

        // uninstall alias
        let cli = Cli::try_parse_from(["fontlift", "u", "-n", "FontName"]).expect("alias u");
        assert!(matches!(cli.command, Commands::Uninstall { .. }));

        // remove alias
        let cli = Cli::try_parse_from(["fontlift", "rm", "-n", "FontName"]).expect("alias rm");
        assert!(matches!(cli.command, Commands::Remove { .. }));

        // cleanup alias
        let cli = Cli::try_parse_from(["fontlift", "c"]).expect("alias c");
        assert!(matches!(cli.command, Commands::Cleanup { .. }));
    }

    #[test]
    fn clap_error_exit_codes_match_legacy() {
        use clap::error::ErrorKind;

        assert_eq!(exit_code_for_clap_error(ErrorKind::DisplayHelp), 0);
        assert_eq!(exit_code_for_clap_error(ErrorKind::DisplayVersion), 0);
        assert_eq!(exit_code_for_clap_error(ErrorKind::UnknownArgument), 1);
        assert_eq!(
            exit_code_for_clap_error(ErrorKind::MissingRequiredArgument),
            1
        );
    }
}
