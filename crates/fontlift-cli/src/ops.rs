use clap::CommandFactory;
use clap_complete::{generate, Shell};
use fontlift_core::{protection, validation, FontError, FontInfo, FontManager, FontScope};
use serde_json::to_string_pretty;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use crate::args::Cli;

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

pub(crate) fn log_status(opts: &OperationOptions, message: &str) {
    if opts.output.should_print() {
        println!("{}", message);
    }
}

pub(crate) fn log_verbose(opts: &OperationOptions, message: &str) {
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
