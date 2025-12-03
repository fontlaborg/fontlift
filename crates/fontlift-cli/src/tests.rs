use super::*;
use clap_complete::Shell;
use fontlift_core::{FontError, FontManager, FontScope, FontliftFontFaceInfo, FontliftFontSource};
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

fn sample_font(path: &str, postscript: &str) -> FontliftFontFaceInfo {
    FontliftFontFaceInfo::new(
        FontliftFontSource::new(PathBuf::from(path)),
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
fn list_renders_lines_sorted_and_deduped_by_default() {
    let fonts = vec![
        sample_font("/fonts/Beta.ttf", "Beta-Bold"),
        sample_font("/fonts/Alpha.ttf", "Alpha-Regular"),
        sample_font("/fonts/Alpha.ttf", "Alpha-Regular"),
    ];

    let opts = ListRenderOptions {
        show_path: true,
        show_name: false,
        sorted: false,
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
fn list_renders_name_only_sorted_by_default() {
    let fonts = vec![
        sample_font("/fonts/Delta.ttf", "Delta"),
        sample_font("/fonts/Alpha.ttf", "Alpha-Regular"),
        sample_font("/fonts/Beta.ttf", "Beta-Bold"),
    ];

    let opts = ListRenderOptions {
        show_path: false,
        show_name: true,
        sorted: false,
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
            "Alpha-Regular".to_string(),
            "Beta-Bold".to_string(),
            "Delta".to_string()
        ],
        "sorts names deterministically even without --sorted"
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
    prunes: Mutex<Vec<FontScope>>,
    cache_clears: Mutex<Vec<FontScope>>,
}

impl FontManager for RecordingManager {
    fn install_font(&self, source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        let scope = source.scope.unwrap_or(FontScope::User);
        self.installs
            .lock()
            .expect("lock")
            .push((source.path.clone(), scope));
        Ok(())
    }

    fn uninstall_font(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        Ok(())
    }

    fn remove_font(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        Ok(())
    }

    fn is_font_installed(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<bool> {
        Ok(false)
    }

    fn list_installed_fonts(&self) -> fontlift_core::FontResult<Vec<FontliftFontFaceInfo>> {
        Ok(Vec::new())
    }

    fn clear_font_caches(&self, _scope: FontScope) -> fontlift_core::FontResult<()> {
        self.cache_clears.lock().expect("lock").push(_scope);
        Ok(())
    }

    fn prune_missing_fonts(&self, scope: FontScope) -> fontlift_core::FontResult<usize> {
        self.prunes.lock().expect("lock").push(scope);
        Ok(0)
    }
}

#[derive(Default)]
struct ScopedUninstallManager {
    uninstall_scopes: Mutex<Vec<FontScope>>,
}

impl ScopedUninstallManager {
    fn scopes_called(&self) -> Vec<FontScope> {
        self.uninstall_scopes.lock().expect("lock").clone()
    }
}

impl FontManager for ScopedUninstallManager {
    fn install_font(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        Ok(())
    }

    fn uninstall_font(&self, source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        let scope = source.scope.unwrap_or(FontScope::User);
        self.uninstall_scopes.lock().expect("lock").push(scope);

        match scope {
            FontScope::System => Ok(()),
            FontScope::User => Err(FontError::RegistrationFailed(
                "not installed in user scope".to_string(),
            )),
        }
    }

    fn remove_font(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        Ok(())
    }

    fn is_font_installed(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<bool> {
        Ok(true)
    }

    fn list_installed_fonts(&self) -> fontlift_core::FontResult<Vec<FontliftFontFaceInfo>> {
        Ok(vec![FontliftFontFaceInfo::new(
            FontliftFontSource::new(PathBuf::from("/Library/Fonts/ScopedUninstall.ttf"))
                .with_scope(None),
            "ScopedUninstall".to_string(),
            "Scoped Uninstall".to_string(),
            "Scoped".to_string(),
            "Regular".to_string(),
        )])
    }

    fn clear_font_caches(&self, _scope: FontScope) -> fontlift_core::FontResult<()> {
        Ok(())
    }

    fn prune_missing_fonts(&self, _scope: FontScope) -> fontlift_core::FontResult<usize> {
        Ok(0)
    }
}

#[derive(Default)]
struct DenyCacheManager {
    prunes: Mutex<usize>,
    cache_attempts: Mutex<usize>,
}

impl FontManager for DenyCacheManager {
    fn install_font(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        Err(FontError::UnsupportedOperation("install not used in test".into()))
    }

    fn uninstall_font(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        Err(FontError::UnsupportedOperation("uninstall not used in test".into()))
    }

    fn remove_font(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<()> {
        Err(FontError::UnsupportedOperation("remove not used in test".into()))
    }

    fn is_font_installed(&self, _source: &FontliftFontSource) -> fontlift_core::FontResult<bool> {
        Ok(false)
    }

    fn list_installed_fonts(
        &self,
    ) -> fontlift_core::FontResult<Vec<FontliftFontFaceInfo>> {
        Ok(vec![])
    }

    fn clear_font_caches(&self, _scope: FontScope) -> fontlift_core::FontResult<()> {
        *self.cache_attempts.lock().expect("lock") += 1;
        Err(FontError::PermissionDenied(
            "cache clearing requires admin".to_string(),
        ))
    }

    fn prune_missing_fonts(&self, _scope: FontScope) -> fontlift_core::FontResult<usize> {
        *self.prunes.lock().expect("lock") += 1;
        Ok(1)
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
            false, // no validation
            ValidationStrictness::Normal,
            opts,
        ))
        .expect("dry run install");

    assert!(
        manager.installs.lock().expect("lock").is_empty(),
        "dry-run should not call install_font"
    );
}

#[test]
fn cleanup_respects_prune_and_cache_flags() {
    let runtime = Runtime::new().expect("runtime");
    let base_opts = OperationOptions::new(false, true, false);

    // default: both prune and cache clear
    let manager = Arc::new(RecordingManager::default());
    runtime
        .block_on(handle_cleanup_command(
            manager.clone(),
            false,
            false,
            false,
            base_opts,
        ))
        .expect("cleanup both");
    assert_eq!(manager.prunes.lock().expect("lock").len(), 1);
    assert_eq!(manager.cache_clears.lock().expect("lock").len(), 1);

    // prune-only
    let manager = Arc::new(RecordingManager::default());
    runtime
        .block_on(handle_cleanup_command(
            manager.clone(),
            false,
            true,
            false,
            base_opts,
        ))
        .expect("prune-only");
    assert_eq!(manager.prunes.lock().expect("lock").len(), 1);
    assert!(
        manager.cache_clears.lock().expect("lock").is_empty(),
        "cache clear should be skipped"
    );

    // cache-only
    let manager = Arc::new(RecordingManager::default());
    runtime
        .block_on(handle_cleanup_command(
            manager.clone(),
            false,
            false,
            true,
            base_opts,
        ))
        .expect("cache-only");
    assert!(
        manager.prunes.lock().expect("lock").is_empty(),
        "prune should be skipped"
    );
    assert_eq!(manager.cache_clears.lock().expect("lock").len(), 1);
}

#[test]
fn cleanup_skips_cache_clear_permission_denied_on_windows_user_scope() {
    let runtime = Runtime::new().expect("runtime");
    let manager = Arc::new(DenyCacheManager::default());
    let base_opts = OperationOptions::new(false, true, false);

    let result = runtime.block_on(handle_cleanup_command(
        manager.clone(),
        false, // admin
        false, // prune_only
        false, // cache_only
        base_opts,
    ));

    assert!(result.is_ok(), "cleanup should not fail when cache clear needs admin");
    assert_eq!(
        *manager.prunes.lock().expect("lock"),
        1,
        "prune should run"
    );
    assert_eq!(
        *manager.cache_attempts.lock().expect("lock"),
        1,
        "cache clear should be attempted once"
    );
}

#[test]
fn uninstall_by_name_checks_both_scopes() {
    let runtime = Runtime::new().expect("runtime");
    let manager = Arc::new(ScopedUninstallManager::default());
    let opts = OperationOptions::new(false, true, false);

    runtime
        .block_on(handle_uninstall_command(
            manager.clone(),
            Some("ScopedUninstall".to_string()),
            Vec::new(),
            false,
            opts,
        ))
        .expect("uninstall should succeed after checking both scopes");

    assert_eq!(
        manager.scopes_called(),
        vec![FontScope::User, FontScope::System],
        "should attempt user then system scope"
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

    // doctor alias
    let cli = Cli::try_parse_from(["fontlift", "d"]).expect("alias d");
    assert!(matches!(cli.command, Commands::Doctor { .. }));
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

#[test]
fn validation_strictness_presets_parse() {
    // Default is Normal
    let cli = Cli::try_parse_from(["fontlift", "install", "font.ttf"]).expect("default strictness");
    let Commands::Install {
        validation_strictness,
        ..
    } = cli.command
    else {
        panic!("expected Install");
    };
    assert!(matches!(
        validation_strictness,
        ValidationStrictness::Normal
    ));

    // Explicit lenient
    let cli = Cli::try_parse_from([
        "fontlift",
        "install",
        "font.ttf",
        "--validation-strictness",
        "lenient",
    ])
    .expect("lenient");
    let Commands::Install {
        validation_strictness,
        ..
    } = cli.command
    else {
        panic!("expected Install");
    };
    assert!(matches!(
        validation_strictness,
        ValidationStrictness::Lenient
    ));

    // Explicit paranoid
    let cli = Cli::try_parse_from([
        "fontlift",
        "install",
        "font.ttf",
        "--validation-strictness",
        "paranoid",
    ])
    .expect("paranoid");
    let Commands::Install {
        validation_strictness,
        ..
    } = cli.command
    else {
        panic!("expected Install");
    };
    assert!(matches!(
        validation_strictness,
        ValidationStrictness::Paranoid
    ));
}

#[test]
fn no_validate_flag_parses() {
    let cli =
        Cli::try_parse_from(["fontlift", "install", "font.ttf", "--no-validate"]).expect("parse");
    let Commands::Install { no_validate, .. } = cli.command else {
        panic!("expected Install");
    };
    assert!(no_validate, "--no-validate should set flag to true");
}

#[test]
fn help_text_includes_all_commands() {
    use clap::CommandFactory;

    let mut cmd = Cli::command();
    let help = cmd.render_help().to_string();

    // Verify all main commands are listed in help
    assert!(help.contains("list"), "help should mention list command");
    assert!(
        help.contains("install"),
        "help should mention install command"
    );
    assert!(
        help.contains("uninstall"),
        "help should mention uninstall command"
    );
    assert!(
        help.contains("remove"),
        "help should mention remove command"
    );
    assert!(
        help.contains("cleanup"),
        "help should mention cleanup command"
    );
    assert!(
        help.contains("doctor"),
        "help should mention doctor command"
    );
    assert!(
        help.contains("completions"),
        "help should mention completions command"
    );
}

#[test]
fn shell_completions_generate_for_all_shells() {
    use clap_complete::Shell;

    // Test all supported shells generate valid completions
    for shell in [
        Shell::Bash,
        Shell::Zsh,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Elvish,
    ] {
        let mut buffer = Vec::new();
        write_completions(shell, &mut buffer).expect(&format!("{:?} completions", shell));
        let script = String::from_utf8(buffer).expect("utf8");
        assert!(
            !script.is_empty(),
            "{:?} completions should not be empty",
            shell
        );
        // All shells should include the binary name
        assert!(
            script.contains("fontlift"),
            "{:?} completions should reference fontlift",
            shell
        );
    }
}
