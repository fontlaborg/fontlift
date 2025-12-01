//! CLI interface for fontlift
//!
//! This module provides the command-line interface that mirrors the functionality
//! of the existing Swift and CLI implementations.

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use fontlift_core::{FontError, FontInfo, FontManager, FontScope};
use serde_json::to_string_pretty;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

/// Font management CLI tool
#[derive(Parser)]
#[command(name = "fontlift")]
#[command(about = "Install, uninstall, list, and remove fonts cross-platform", long_about = None)]
#[command(version = "2.0.0-dev")]
pub struct Cli {
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
        /// Font file path to install
        font_path: PathBuf,

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

        /// Font file path to uninstall
        font_path: Option<PathBuf>,

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

        /// Font file path to remove
        font_path: Option<PathBuf>,

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

/// Prepare list output according to options (sorting/deduplication is deterministic)
pub fn render_list_output(
    mut fonts: Vec<FontInfo>,
    opts: ListRenderOptions,
) -> Result<ListRender, FontError> {
    let must_sort = opts.sorted || opts.json;

    if must_sort {
        fonts.sort_by(|a, b| {
            let name_a = a.postscript_name.to_lowercase();
            let name_b = b.postscript_name.to_lowercase();
            let path_a = a.path.to_string_lossy().to_string();
            let path_b = b.path.to_string_lossy().to_string();
            (name_a, path_a).cmp(&(name_b, path_b))
        });

        fonts.dedup_by(|a, b| a.postscript_name == b.postscript_name && a.path == b.path);
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
    font_path: PathBuf,
    admin: bool,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    println!("Installing font from: {}", font_path.display());
    println!(
        "Scope: {}",
        if admin {
            "system-level (all users)"
        } else {
            "user-level"
        }
    );

    manager.install_font(&font_path, scope)?;

    println!("✅ Successfully installed font");

    Ok(())
}

/// Handle the uninstall command
pub async fn handle_uninstall_command(
    manager: Arc<dyn FontManager>,
    name: Option<String>,
    font_path: Option<PathBuf>,
    admin: bool,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    if let Some(font_name) = name {
        println!("Uninstalling font by name: {}", font_name);

        // Find font by name in installed fonts
        let installed_fonts = manager.list_installed_fonts()?;
        if let Some(font) = installed_fonts
            .iter()
            .find(|f| f.postscript_name == font_name || f.full_name == font_name)
        {
            manager.uninstall_font(&font.path, scope)?;
            println!("✅ Successfully uninstalled font '{}'", font_name);
        } else {
            return Err(FontError::FontNotFound(PathBuf::from(font_name)));
        }
    } else if let Some(path) = font_path {
        println!("Uninstalling font from path: {}", path.display());

        manager.uninstall_font(&path, scope)?;
        println!("✅ Successfully uninstalled font");
    } else {
        return Err(FontError::RegistrationFailed(
            "Must specify --name or a font path".to_string(),
        ));
    }

    Ok(())
}

/// Handle the remove command
pub async fn handle_remove_command(
    manager: Arc<dyn FontManager>,
    name: Option<String>,
    font_path: Option<PathBuf>,
    admin: bool,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    if let Some(font_name) = name {
        println!("Removing font by name: {}", font_name);

        // Find font by name in installed fonts
        let installed_fonts = manager.list_installed_fonts()?;
        if let Some(font) = installed_fonts
            .iter()
            .find(|f| f.postscript_name == font_name || f.full_name == font_name)
        {
            manager.remove_font(&font.path, scope)?;
            println!("✅ Successfully removed font '{}'", font_name);
        } else {
            return Err(FontError::FontNotFound(PathBuf::from(font_name)));
        }
    } else if let Some(path) = font_path {
        println!("Removing font from path: {}", path.display());

        manager.remove_font(&path, scope)?;
        println!("✅ Successfully removed font");
    } else {
        return Err(FontError::RegistrationFailed(
            "Must specify --name or a font path".to_string(),
        ));
    }

    Ok(())
}

/// Handle the cleanup command
pub async fn handle_cleanup_command(
    manager: Arc<dyn FontManager>,
    admin: bool,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    println!(
        "Starting {} cleanup...",
        if admin { "system" } else { "user" }
    );

    manager.clear_font_caches(scope)?;
    println!("✅ Successfully cleared font caches");

    Ok(())
}

/// Main CLI handler
pub async fn run_cli(cli: Cli) -> Result<(), FontError> {
    let manager = create_font_manager();

    match cli.command {
        Commands::List { path, name, sorted } => {
            handle_list_command(manager, path, name, sorted, cli.json).await?;
        }
        Commands::Install { font_path, admin } => {
            handle_install_command(manager, font_path, admin).await?;
        }
        Commands::Uninstall {
            name,
            font_path,
            admin,
        } => {
            handle_uninstall_command(manager, name, font_path, admin).await?;
        }
        Commands::Remove {
            name,
            font_path,
            admin,
        } => {
            handle_remove_command(manager, name, font_path, admin).await?;
        }
        Commands::Cleanup { admin } => {
            handle_cleanup_command(manager, admin).await?;
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
    use std::path::PathBuf;

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
