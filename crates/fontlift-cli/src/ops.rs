use clap::CommandFactory;
use clap_complete::{generate, Shell};
use fontlift_core::{
    journal::{self, JournalAction, RecoveryPolicy},
    protection, validation,
    validation_ext::{self, ValidatorConfig},
    FontError, FontManager, FontScope, FontliftFontFaceInfo, FontliftFontSource,
};
use serde_json::to_string_pretty;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::args::{Cli, ValidationStrictness};

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

fn scope_order(preferred: FontScope) -> [FontScope; 2] {
    match preferred {
        FontScope::User => [FontScope::User, FontScope::System],
        FontScope::System => [FontScope::System, FontScope::User],
    }
}

fn describe_scope_chain(preferred: FontScope) -> String {
    scope_order(preferred)
        .iter()
        .map(|s| s.description())
        .collect::<Vec<_>>()
        .join(" then ")
}

fn uninstall_across_scopes(
    manager: &Arc<dyn FontManager>,
    path: &Path,
    preferred_scope: FontScope,
) -> Result<FontScope, FontError> {
    let mut last_error: Option<FontError> = None;

    for scope in scope_order(preferred_scope) {
        let source = FontliftFontSource::new(path.to_path_buf()).with_scope(Some(scope));
        match manager.uninstall_font(&source) {
            Ok(()) => return Ok(scope),
            Err(err) => last_error = Some(err),
        }
    }

    if let Some(err) = last_error {
        Err(err)
    } else {
        Err(FontError::RegistrationFailed(format!(
            "Failed to uninstall font {} in any scope",
            path.display()
        )))
    }
}

/// Prepare list output according to options (sorting/deduplication is deterministic)
pub fn render_list_output(
    mut fonts: Vec<FontliftFontFaceInfo>,
    opts: ListRenderOptions,
) -> Result<ListRender, FontError> {
    // JSON and explicitly sorted output should dedupe the underlying font records first
    let must_dedupe_fonts = opts.sorted || opts.json;

    if must_dedupe_fonts {
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
            lines.push(format!(
                "{}::{}",
                font.source.path.display(),
                font.postscript_name
            ));
        } else if show_path {
            lines.push(font.source.path.display().to_string());
        } else {
            lines.push(font.postscript_name);
        }
    }

    // Always present the list in deterministic order; dedupe path-only output by default
    lines.sort();

    if (opts.show_path && !opts.show_name) || opts.sorted {
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

/// Convert CLI strictness to core config
fn to_core_strictness(s: ValidationStrictness) -> validation_ext::ValidationStrictness {
    match s {
        ValidationStrictness::Lenient => validation_ext::ValidationStrictness::Lenient,
        ValidationStrictness::Normal => validation_ext::ValidationStrictness::Normal,
        ValidationStrictness::Paranoid => validation_ext::ValidationStrictness::Paranoid,
    }
}

/// Handle the install command
pub async fn handle_install_command(
    manager: Arc<dyn FontManager>,
    font_inputs: Vec<PathBuf>,
    admin: bool,
    validate: bool,
    strictness: ValidationStrictness,
    opts: OperationOptions,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    let targets = collect_font_inputs(&font_inputs)?;

    // Optional pre-flight validation using out-of-process validator
    if validate {
        log_verbose(&opts, "Running out-of-process font validation...");
        let config = ValidatorConfig::from_strictness(to_core_strictness(strictness));

        match validation_ext::validate_and_introspect(&targets, &config) {
            Ok(results) => {
                for (i, result) in results.iter().enumerate() {
                    if let Err(e) = result {
                        log_status(
                            &opts,
                            &format!("⚠️  Validation failed for {}: {}", targets[i].display(), e),
                        );
                        if !opts.dry_run {
                            return Err(FontError::InvalidFormat(format!(
                                "Font validation failed: {}",
                                targets[i].display()
                            )));
                        }
                    } else {
                        log_verbose(&opts, &format!("✓ Validated: {}", targets[i].display()));
                    }
                }
            }
            Err(e) => {
                // Validator not available - warn but continue
                log_verbose(
                    &opts,
                    &format!("⚠️  Validation skipped (validator unavailable): {}", e),
                );
            }
        }
    }

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
        let source = FontliftFontSource::new(path.clone()).with_scope(Some(scope));
        manager.install_font(&source)?;
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
    let default_scope = if admin {
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
            let starting_scope = font.source.scope.unwrap_or(default_scope);

            if opts.dry_run {
                log_status(
                    &opts,
                    &format!(
                        "DRY-RUN: would uninstall '{}' at {} (checking {})",
                        font_name,
                        font.source.path.display(),
                        describe_scope_chain(starting_scope)
                    ),
                );
            } else {
                let used_scope =
                    uninstall_across_scopes(&manager, &font.source.path, starting_scope)?;
                log_status(
                    &opts,
                    &format!(
                        "✅ Successfully uninstalled font '{}' ({})",
                        font_name,
                        used_scope.description()
                    ),
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
                        "DRY-RUN: would uninstall font at {} (checking {})",
                        path.display(),
                        describe_scope_chain(default_scope)
                    ),
                );
                continue;
            }

            log_status(
                &opts,
                &format!("Uninstalling font from path: {}", path.display()),
            );

            let used_scope = uninstall_across_scopes(&manager, &path, default_scope)?;
            log_status(
                &opts,
                &format!(
                    "✅ Successfully uninstalled font ({})",
                    used_scope.description()
                ),
            );
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
                        font.source.path.display()
                    ),
                );
            } else {
                let source =
                    FontliftFontSource::new(font.source.path.clone()).with_scope(font.source.scope);
                manager.remove_font(&source)?;
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

            let source = FontliftFontSource::new(path.clone()).with_scope(Some(scope));
            manager.remove_font(&source)?;
            log_status(&opts, "✅ Successfully removed font");
        }
    }

    Ok(())
}

/// Handle the cleanup command
pub async fn handle_cleanup_command(
    manager: Arc<dyn FontManager>,
    admin: bool,
    prune_only: bool,
    cache_only: bool,
    opts: OperationOptions,
) -> Result<(), FontError> {
    let scope = if admin {
        FontScope::System
    } else {
        FontScope::User
    };

    let run_prune = !cache_only;
    let run_cache_clear = !prune_only;

    log_status(
        &opts,
        &format!(
            "Starting {} cleanup...",
            if admin { "system" } else { "user" }
        ),
    );

    if opts.dry_run {
        let mut planned = Vec::new();
        if run_prune {
            planned.push("prune stale registrations");
        }
        if run_cache_clear {
            planned.push("clear font caches");
        }
        log_status(
            &opts,
            &format!(
                "DRY-RUN: would {} ({})",
                planned.join(" and "),
                scope.description()
            ),
        );
        return Ok(());
    }

    if run_prune {
        let pruned = manager.prune_missing_fonts(scope)?;
        log_verbose(
            &opts,
            &format!("Pruned {} stale font registration(s)", pruned),
        );
    }

    if run_cache_clear {
        manager.clear_font_caches(scope)?;
        log_status(&opts, "✅ Successfully cleared font caches");
    }

    Ok(())
}

/// Handle the doctor command (recover from interrupted operations)
pub async fn handle_doctor_command(preview: bool, opts: OperationOptions) -> Result<(), FontError> {
    log_status(&opts, "Checking for interrupted operations...");

    let journal = journal::load_journal()?;
    let incomplete = journal.incomplete_entries();

    if incomplete.is_empty() {
        log_status(&opts, "✅ No interrupted operations found");
        return Ok(());
    }

    log_status(
        &opts,
        &format!("Found {} interrupted operation(s)", incomplete.len()),
    );

    for entry in &incomplete {
        log_status(
            &opts,
            &format!("\nOperation {} (started {:?}):", entry.id, entry.started_at),
        );
        if let Some(desc) = &entry.description {
            log_status(&opts, &format!("  Description: {}", desc));
        }
        log_status(
            &opts,
            &format!(
                "  Progress: step {} of {}",
                entry.current_step,
                entry.actions.len()
            ),
        );

        for (i, action) in entry.remaining_actions().iter().enumerate() {
            let step_num = entry.current_step + i + 1;
            log_status(&opts, &format!("  [{}] {}", step_num, action.description()));
        }
    }

    if preview || opts.dry_run {
        log_status(
            &opts,
            "\nDRY-RUN: would attempt recovery of above operations",
        );
        return Ok(());
    }

    log_status(&opts, "\nAttempting recovery...");

    let results = journal::recover_incomplete_operations(|action, policy| {
        log_verbose(&opts, &format!("  {:?}: {}", policy, action.description()));

        // Execute recovery based on policy
        match (action, policy) {
            (_, RecoveryPolicy::Skip) => Ok(true),
            (JournalAction::CopyFile { from, to }, RecoveryPolicy::RollForward) => {
                if to.exists() {
                    Ok(true)
                } else if from.exists() {
                    std::fs::copy(from, to)
                        .map(|_| true)
                        .map_err(FontError::IoError)
                } else {
                    Ok(false)
                }
            }
            (JournalAction::DeleteFile { path }, RecoveryPolicy::RollForward) => {
                if path.exists() {
                    std::fs::remove_file(path)
                        .map(|_| true)
                        .map_err(FontError::IoError)
                } else {
                    Ok(true)
                }
            }
            (JournalAction::RegisterFont { .. }, RecoveryPolicy::RollForward) => {
                // Font registration recovery needs the manager - skip for now
                log_verbose(
                    &opts,
                    "  (font registration recovery requires manual intervention)",
                );
                Ok(false)
            }
            (JournalAction::UnregisterFont { .. }, RecoveryPolicy::RollForward) => {
                // Font unregistration recovery needs the manager - skip for now
                log_verbose(
                    &opts,
                    "  (font unregistration recovery requires manual intervention)",
                );
                Ok(false)
            }
            (JournalAction::ClearCache { .. }, _) => Ok(true),
            _ => Ok(false),
        }
    })?;

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.len() - succeeded;

    if failed > 0 {
        log_status(
            &opts,
            &format!(
                "⚠️  Recovery completed with {} success, {} failure(s)",
                succeeded, failed
            ),
        );
    } else if succeeded > 0 {
        log_status(
            &opts,
            &format!("✅ Successfully recovered {} action(s)", succeeded),
        );
    } else {
        log_status(&opts, "✅ No recovery actions needed");
    }

    Ok(())
}
