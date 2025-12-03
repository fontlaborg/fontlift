#![cfg(target_os = "macos")]

use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Mutex to serialize tests that modify FONTLIFT_FAKE_REGISTRY_ROOT
static ENV_LOCK: Mutex<()> = Mutex::new(());

use fontlift_cli::{
    handle_doctor_command, handle_install_command, handle_uninstall_command, ListRender,
    ListRenderOptions, OperationOptions, ValidationStrictness,
};
use fontlift_core::{
    journal, validation_ext::ValidatorConfig, FontManager, FontScope, FontliftFontSource,
};
use fontlift_platform_mac::MacFontManager;
use serde_json::Value;
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points to crates/fontlift-cli, go up two levels
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .expect("Failed to find workspace root")
}

fn fixture_font() -> PathBuf {
    workspace_root().join("tests/fixtures/fonts/AtkinsonHyperlegible-Regular.ttf")
}

fn fixture_font_otf() -> PathBuf {
    workspace_root().join("tests/fixtures/fonts/AtkinsonHyperlegible-Regular.otf")
}

fn fixture_font_ttc() -> PathBuf {
    workspace_root().join("tests/fixtures/fonts/AtkinsonHyperlegible-Regular.ttc")
}

fn malformed_fixture() -> PathBuf {
    workspace_root().join("tests/fixtures/fonts/malformed.ttf")
}

fn quiet_opts() -> OperationOptions {
    OperationOptions::new(false, true, false)
}

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        let previous = env::var_os(key);
        env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.previous.take() {
            env::set_var(self.key, prev);
        } else {
            env::remove_var(self.key);
        }
    }
}

#[tokio::test]
async fn mac_fake_registry_user_scope_round_trip() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let mac_manager = MacFontManager::new();
    assert!(mac_manager.is_fake_registry_enabled());
    let manager: Arc<dyn FontManager> = Arc::new(mac_manager);

    let source_path = fixture_font();
    let target_path = temp_root
        .path()
        .join("Library/Fonts/AtkinsonHyperlegible-Regular.ttf");

    handle_install_command(
        manager.clone(),
        vec![source_path.clone()],
        false,
        false, // validate
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await
    .expect("install into fake registry should succeed");

    assert!(target_path.exists(), "font copied into fake registry");
    assert!(
        manager
            .is_font_installed(
                &FontliftFontSource::new(source_path.clone()).with_scope(Some(FontScope::User))
            )
            .expect("is_font_installed should read fake registry"),
        "fake registry reports installation"
    );

    let fonts = manager
        .list_installed_fonts()
        .expect("list should read fake registry");
    assert!(
        fonts
            .iter()
            .any(|f| f.source.path == target_path && f.source.scope == Some(FontScope::User)),
        "listed fonts include the installed user font"
    );

    let rendered = fontlift_cli::render_list_output(
        fonts.clone(),
        ListRenderOptions {
            show_path: true,
            show_name: true,
            sorted: true,
            json: true,
        },
    )
    .expect("render list to JSON");

    if let ListRender::Json(json) = rendered {
        let parsed: Value = serde_json::from_str(&json).expect("valid JSON output");
        let array = parsed.as_array().expect("list renders to array");
        assert!(
            array.iter().any(|entry| {
                entry["source"]["path"]
                    .as_str()
                    .map(|p| p.ends_with("AtkinsonHyperlegible-Regular.ttf"))
                    .unwrap_or(false)
            }),
            "JSON output includes installed font"
        );
    } else {
        panic!("expected JSON render");
    }

    handle_uninstall_command(
        manager.clone(),
        None,
        vec![source_path.clone()],
        false,
        quiet_opts(),
    )
    .await
    .expect("uninstall should remove from fake registry");

    assert!(
        !target_path.exists(),
        "font file removed from fake registry after uninstall"
    );
    assert!(
        !manager
            .is_font_installed(
                &FontliftFontSource::new(source_path).with_scope(Some(FontScope::User))
            )
            .expect("is_font_installed should read fake registry"),
        "fake registry reports font removed"
    );
}

#[tokio::test]
async fn mac_fake_registry_system_scope_without_admin() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let manager: Arc<dyn FontManager> = Arc::new(MacFontManager::new());

    let source_path = fixture_font();
    let system_target = temp_root
        .path()
        .join("System/Library/Fonts/AtkinsonHyperlegible-Regular.ttf");

    handle_install_command(
        manager.clone(),
        vec![source_path.clone()],
        true,
        false, // validate
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await
    .expect("system-scope install should be allowed in fake registry");
    assert!(
        system_target.exists(),
        "system font copied into fake registry"
    );

    handle_uninstall_command(
        manager.clone(),
        None,
        vec![source_path.clone()],
        true,
        quiet_opts(),
    )
    .await
    .expect("system-scope uninstall should clean fake registry");
    assert!(
        !system_target.exists(),
        "system font removed from fake registry"
    );
}

/// Test that malformed fonts are rejected when CLI validation is enabled
#[tokio::test]
async fn mac_fake_registry_rejects_malformed_font_with_validation() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let manager: Arc<dyn FontManager> = Arc::new(MacFontManager::new());

    let malformed_path = malformed_fixture();
    assert!(malformed_path.exists(), "malformed fixture must exist");

    // Install with validation enabled should fail
    let result = handle_install_command(
        manager.clone(),
        vec![malformed_path.clone()],
        false,
        true, // validate=true
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await;

    assert!(
        result.is_err(),
        "installing malformed font with validation should fail"
    );

    let err_msg = result.unwrap_err().to_string();
    // The validator should reject the font
    assert!(
        err_msg.contains("Invalid") || err_msg.contains("validation") || err_msg.contains("parse"),
        "error should indicate validation failure: {err_msg}"
    );

    // Font should NOT be installed
    let target_path = temp_root.path().join("Library/Fonts/malformed.ttf");
    assert!(
        !target_path.exists(),
        "malformed font should not be copied to fake registry"
    );
}

/// Test that malformed fonts CAN be installed when validation is disabled
#[tokio::test]
async fn mac_fake_registry_accepts_malformed_font_without_validation() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let manager: Arc<dyn FontManager> = Arc::new(MacFontManager::new());

    let malformed_path = malformed_fixture();
    assert!(malformed_path.exists(), "malformed fixture must exist");

    // Install with validation disabled should succeed (at CLI level, Core Text may still reject)
    let result = handle_install_command(
        manager.clone(),
        vec![malformed_path.clone()],
        false,
        false, // validate=false
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await;

    // The fake registry will accept the file even if it's malformed
    // (Real Core Text would reject it, but fake registry just copies files)
    assert!(
        result.is_ok(),
        "installing malformed font without validation should succeed in fake registry: {:?}",
        result.err()
    );

    let target_path = temp_root.path().join("Library/Fonts/malformed.ttf");
    assert!(
        target_path.exists(),
        "malformed font should be copied to fake registry when validation disabled"
    );
}

/// Test that the manager-level validation rejects malformed fonts
#[tokio::test]
async fn mac_manager_with_validation_rejects_malformed_font() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());

    // Create manager WITH validation enabled
    let manager = MacFontManager::with_validation(ValidatorConfig::default());

    let malformed_path = malformed_fixture();
    let source = FontliftFontSource::new(malformed_path.clone()).with_scope(Some(FontScope::User));

    let result = manager.install_font(&source);

    assert!(
        result.is_err(),
        "manager with validation should reject malformed font"
    );

    let target_path = temp_root.path().join("Library/Fonts/malformed.ttf");
    assert!(
        !target_path.exists(),
        "malformed font should not be installed"
    );
}

/// Golden output test: validates exact JSON schema for list --json output
#[tokio::test]
async fn mac_fake_registry_list_json_golden_output() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let mac_manager = MacFontManager::new();
    let manager: Arc<dyn FontManager> = Arc::new(mac_manager);

    // Install the fixture font
    let source_path = fixture_font();
    handle_install_command(
        manager.clone(),
        vec![source_path.clone()],
        false,
        false,
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await
    .expect("install should succeed");

    // List and render as JSON
    let fonts = manager.list_installed_fonts().expect("list");
    let rendered = fontlift_cli::render_list_output(
        fonts,
        ListRenderOptions {
            show_path: true,
            show_name: true,
            sorted: true,
            json: true,
        },
    )
    .expect("render");

    let ListRender::Json(json) = rendered else {
        panic!("expected JSON output");
    };

    // Parse and validate schema
    let parsed: Vec<Value> = serde_json::from_str(&json).expect("valid JSON array");
    assert_eq!(parsed.len(), 1, "should have exactly one font installed");

    let font = &parsed[0];

    // Validate required fields exist and have correct types
    assert!(
        font["postscript_name"].is_string(),
        "postscript_name should be string"
    );
    assert!(font["full_name"].is_string(), "full_name should be string");
    assert!(
        font["family_name"].is_string(),
        "family_name should be string"
    );
    assert!(font["style"].is_string(), "style should be string");
    assert!(font["source"].is_object(), "source should be object");
    assert!(
        font["source"]["path"].is_string(),
        "source.path should be string"
    );

    // Validate source.path points to correct location
    let path = font["source"]["path"].as_str().unwrap();
    assert!(
        path.ends_with("AtkinsonHyperlegible-Regular.ttf"),
        "path should end with font filename, got: {path}"
    );
    assert!(
        path.contains("Library/Fonts"),
        "path should be in Library/Fonts, got: {path}"
    );

    // Validate scope is present and correct (capitalized per Rust enum serialization)
    assert_eq!(
        font["source"]["scope"].as_str(),
        Some("User"),
        "scope should be 'User'"
    );

    // Validate format field if present (capitalized per Rust enum serialization)
    if let Some(format) = font["source"]["format"].as_str() {
        assert!(
            ["TTF", "OTF", "TTC", "OTC", "WOFF", "WOFF2", "DFont", "Unknown"].contains(&format),
            "format should be valid font format, got: {format}"
        );
    }

    // Validate optional numeric fields have correct types when present
    if !font["weight"].is_null() {
        assert!(
            font["weight"].is_number(),
            "weight should be number if present"
        );
    }
    if !font["italic"].is_null() {
        assert!(
            font["italic"].is_boolean(),
            "italic should be boolean if present"
        );
    }
}

/// Test OTF format installation and JSON output
#[tokio::test]
async fn mac_fake_registry_otf_format_golden_output() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let mac_manager = MacFontManager::new();
    let manager: Arc<dyn FontManager> = Arc::new(mac_manager);

    let source_path = fixture_font_otf();
    assert!(source_path.exists(), "OTF fixture must exist");

    handle_install_command(
        manager.clone(),
        vec![source_path.clone()],
        false,
        false,
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await
    .expect("OTF install should succeed");

    let fonts = manager.list_installed_fonts().expect("list");
    let rendered = fontlift_cli::render_list_output(
        fonts,
        ListRenderOptions {
            show_path: true,
            show_name: true,
            sorted: true,
            json: true,
        },
    )
    .expect("render");

    let ListRender::Json(json) = rendered else {
        panic!("expected JSON output");
    };

    let parsed: Vec<Value> = serde_json::from_str(&json).expect("valid JSON array");
    assert_eq!(
        parsed.len(),
        1,
        "should have exactly one OTF font installed"
    );

    let font = &parsed[0];
    let path = font["source"]["path"].as_str().unwrap();
    assert!(
        path.ends_with("AtkinsonHyperlegible-Regular.otf"),
        "path should end with .otf, got: {path}"
    );

    // OTF format should be detected
    assert_eq!(
        font["source"]["format"].as_str(),
        Some("OTF"),
        "format should be OTF"
    );
}

/// Test TTC (font collection) format installation and JSON output
#[tokio::test]
async fn mac_fake_registry_ttc_format_golden_output() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let mac_manager = MacFontManager::new();
    let manager: Arc<dyn FontManager> = Arc::new(mac_manager);

    let source_path = fixture_font_ttc();
    assert!(source_path.exists(), "TTC fixture must exist");

    handle_install_command(
        manager.clone(),
        vec![source_path.clone()],
        false,
        false,
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await
    .expect("TTC install should succeed");

    let fonts = manager.list_installed_fonts().expect("list");
    let rendered = fontlift_cli::render_list_output(
        fonts,
        ListRenderOptions {
            show_path: true,
            show_name: true,
            sorted: true,
            json: true,
        },
    )
    .expect("render");

    let ListRender::Json(json) = rendered else {
        panic!("expected JSON output");
    };

    let parsed: Vec<Value> = serde_json::from_str(&json).expect("valid JSON array");
    // TTC may contain multiple fonts, should have at least one
    assert!(!parsed.is_empty(), "should have at least one font from TTC");

    let font = &parsed[0];
    let path = font["source"]["path"].as_str().unwrap();
    assert!(
        path.ends_with("AtkinsonHyperlegible-Regular.ttc"),
        "path should end with .ttc, got: {path}"
    );

    // TTC format should be detected
    assert_eq!(
        font["source"]["format"].as_str(),
        Some("TTC"),
        "format should be TTC"
    );
}

/// Test doctor command finds incomplete journal entries
#[tokio::test]
async fn mac_fake_registry_doctor_finds_incomplete_operations() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());

    // Create an incomplete journal entry
    let mut test_journal = journal::Journal::new();
    let actions = vec![
        journal::JournalAction::CopyFile {
            from: fixture_font(),
            to: temp_root.path().join("Library/Fonts/test-font.ttf"),
        },
        journal::JournalAction::RegisterFont {
            path: temp_root.path().join("Library/Fonts/test-font.ttf"),
            scope: FontScope::User,
        },
    ];
    test_journal.record_operation(actions, Some("Test install operation".to_string()));

    // Save the incomplete journal
    journal::save_journal(&test_journal).expect("save journal");

    // Verify doctor command succeeds in preview mode (dry-run)
    let result = handle_doctor_command(true, quiet_opts()).await;
    assert!(
        result.is_ok(),
        "doctor command preview should succeed: {:?}",
        result.err()
    );

    // Verify the journal still has the incomplete entry (preview doesn't modify)
    let reloaded = journal::load_journal().expect("reload journal");
    assert_eq!(
        reloaded.incomplete_entries().len(),
        1,
        "preview mode should not modify journal"
    );
}

/// Test doctor command reports no issues when journal is clean
#[tokio::test]
async fn mac_fake_registry_doctor_clean_journal() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());

    // Create an empty journal (or just don't create one at all)
    let result = handle_doctor_command(false, quiet_opts()).await;
    assert!(
        result.is_ok(),
        "doctor command on clean system should succeed: {:?}",
        result.err()
    );

    // Verify journal is empty or doesn't exist
    let loaded = journal::load_journal().expect("load journal");
    assert!(
        loaded.incomplete_entries().is_empty(),
        "no incomplete entries expected"
    );
}

/// Test doctor command recovers incomplete file copy operation (simulated crash)
#[tokio::test]
async fn mac_fake_registry_doctor_recovers_incomplete_copy() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());

    // Setup: create target directory
    let target_dir = temp_root.path().join("Library/Fonts");
    std::fs::create_dir_all(&target_dir).expect("create fonts dir");

    let source_font = fixture_font();
    let target_font = target_dir.join("recovered-font.ttf");

    // Simulate crash: journal says to copy file, but copy never happened
    let mut test_journal = journal::Journal::new();
    let actions = vec![journal::JournalAction::CopyFile {
        from: source_font.clone(),
        to: target_font.clone(),
    }];
    test_journal.record_operation(actions, Some("Simulated interrupted install".to_string()));
    journal::save_journal(&test_journal).expect("save journal");

    // Verify file doesn't exist yet
    assert!(
        !target_font.exists(),
        "target should not exist before recovery"
    );

    // Run doctor (non-preview mode) to trigger recovery
    let result = handle_doctor_command(false, quiet_opts()).await;
    assert!(
        result.is_ok(),
        "doctor command should succeed: {:?}",
        result.err()
    );

    // Verify file was copied by recovery
    assert!(
        target_font.exists(),
        "doctor should have recovered the file copy"
    );

    // Verify journal entry is now complete
    let reloaded = journal::load_journal().expect("reload journal");
    assert!(
        reloaded.incomplete_entries().is_empty(),
        "journal should have no incomplete entries after recovery"
    );
}

/// Test that validates JSON output can be captured and compared for regression testing
#[tokio::test]
async fn mac_fake_registry_golden_output_capture() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());
    let mac_manager = MacFontManager::new();
    let manager: Arc<dyn FontManager> = Arc::new(mac_manager);

    // Install a known fixture font
    let source_path = fixture_font();
    handle_install_command(
        manager.clone(),
        vec![source_path.clone()],
        false,
        false,
        ValidationStrictness::Normal,
        quiet_opts(),
    )
    .await
    .expect("install should succeed");

    // Capture JSON output
    let fonts = manager.list_installed_fonts().expect("list");
    let rendered = fontlift_cli::render_list_output(
        fonts,
        ListRenderOptions {
            show_path: true,
            show_name: true,
            sorted: true,
            json: true,
        },
    )
    .expect("render");

    let ListRender::Json(json) = rendered else {
        panic!("expected JSON output");
    };

    // Write golden output to temp file for inspection
    let golden_path = temp_root.path().join("golden_output.json");
    std::fs::write(&golden_path, &json).expect("write golden output");

    // Parse to verify structure
    let parsed: Vec<Value> = serde_json::from_str(&json).expect("valid JSON array");
    assert_eq!(parsed.len(), 1, "should have exactly one font");

    // Verify deterministic output (same input produces same output)
    let fonts2 = manager.list_installed_fonts().expect("list again");
    let rendered2 = fontlift_cli::render_list_output(
        fonts2,
        ListRenderOptions {
            show_path: true,
            show_name: true,
            sorted: true,
            json: true,
        },
    )
    .expect("render again");

    let ListRender::Json(json2) = rendered2 else {
        panic!("expected JSON output");
    };

    assert_eq!(
        json, json2,
        "JSON output should be deterministic across calls"
    );
}

/// Test doctor command handles incomplete delete operation (rollforward)
#[tokio::test]
async fn mac_fake_registry_doctor_recovers_incomplete_delete() {
    let _env_lock = ENV_LOCK.lock().expect("env lock");
    let temp_root = TempDir::new().expect("temp dir for fake registry");
    let _guard = EnvGuard::set_path("FONTLIFT_FAKE_REGISTRY_ROOT", temp_root.path());

    // Setup: create a file that should have been deleted
    let target_dir = temp_root.path().join("Library/Fonts");
    std::fs::create_dir_all(&target_dir).expect("create fonts dir");
    let orphan_file = target_dir.join("orphan-font.ttf");
    std::fs::copy(fixture_font(), &orphan_file).expect("create orphan file");

    // Simulate crash: journal says to delete file, but delete never happened
    let mut test_journal = journal::Journal::new();
    let actions = vec![journal::JournalAction::DeleteFile {
        path: orphan_file.clone(),
    }];
    test_journal.record_operation(actions, Some("Simulated interrupted remove".to_string()));
    journal::save_journal(&test_journal).expect("save journal");

    // Verify file exists before recovery
    assert!(
        orphan_file.exists(),
        "orphan file should exist before recovery"
    );

    // Run doctor to trigger recovery
    let result = handle_doctor_command(false, quiet_opts()).await;
    assert!(
        result.is_ok(),
        "doctor command should succeed: {:?}",
        result.err()
    );

    // Verify file was deleted by recovery
    assert!(
        !orphan_file.exists(),
        "doctor should have deleted the orphan file"
    );

    // Verify journal entry is now complete
    let reloaded = journal::load_journal().expect("reload journal");
    assert!(
        reloaded.incomplete_entries().is_empty(),
        "journal should have no incomplete entries after recovery"
    );
}
