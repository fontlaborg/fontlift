use super::*;
use fontlift_core::FontInfo;
use fontlift_core::{FontManager, FontScope};
use clap_complete::Shell;
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
