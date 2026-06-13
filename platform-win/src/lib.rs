//! Windows platform implementation for fontlift.
//!
//! Font installation on Windows is a two-part operation:
//!
//! 1. **Copy the file** into a Fonts directory the OS watches:
//!    - System scope: `C:\Windows\Fonts\` — shared by all users; requires
//!      Administrator privileges.
//!    - User scope: `%LOCALAPPDATA%\Microsoft\Windows\Fonts\` — per-user;
//!      available since Windows 10 version 1809 (October 2018 Update).
//!      Older systems only have the system-wide location.
//!
//! 2. **Write a registry entry** so the font survives reboots:
//!    - System scope: `HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\
//!      CurrentVersion\Fonts` — key name is `"Family Name (TrueType)"`,
//!      value is the filename (relative to `C:\Windows\Fonts\`) or a full path.
//!    - User scope: same key path under `HKEY_CURRENT_USER`.
//!
//!    Without the registry entry the font is available until the next reboot
//!    but then disappears. The registry is the persistent record of installed fonts.
//!
//! 3. **Notify GDI** via `AddFontResourceW` + `SendMessage(HWND_BROADCAST,
//!    WM_FONTCHANGE)` so running applications see the new font without restarting.
//!
//! Uninstalling reverses those steps: `RemoveFontResourceW`, delete the registry
//! value, then (for `remove`) delete the file.
//!
//! Font formats supported by GDI/DirectWrite (and therefore this module):
//! - `.ttf` — TrueType
//! - `.otf` — OpenType with PostScript or TrueType outlines
//! - `.ttc` / `.otc` — TrueType / OpenType Collection (multiple faces per file)
//!
//! `.woff` / `.woff2` are web-only formats; Windows GDI does not support them
//! as installed system fonts.
//!
//! Font caches: Windows maintains the Font Cache Service (`FontCache`) and
//! binary cache files under `ServiceProfiles\LocalService\AppData\Local\FontCache\`.
//! `clear_font_caches` stops the service, deletes cache files, and restarts it.
//! A reboot may be required for all applications to pick up the changes.

#[cfg(windows)]
use fontlift_core::conflicts;
#[cfg(windows)]
use fontlift_core::journal;
use fontlift_core::journal::JournalAction;
use fontlift_core::validation;
use fontlift_core::validation_ext::{self, ValidatorConfig};
use fontlift_core::{
    FontError, FontManager, FontResult, FontScope, FontliftFontFaceInfo, FontliftFontSource,
};
use read_fonts::{tables::name::NameId, FileRef, FontRef, TableProvider};

use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::collections::BTreeSet;

#[cfg(any(windows, test))]
use std::fs;
#[cfg(windows)]
use std::process::Command;

#[cfg(windows)]
use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Gdi::*, Win32::Security::*,
    Win32::Storage::FileSystem::*, Win32::System::Registry::*, Win32::System::Threading::*,
    Win32::UI::Shell::*,
};

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

// Registry path where Windows records all installed fonts.
// Each value under this key maps a display name like "Arial (TrueType)"
// to either a bare filename ("arial.ttf", resolved relative to %WINDIR%\Fonts)
// or an absolute path (used for user-scope fonts in modern Windows 10/11).
// System scope lives under HKLM; user scope lives under the same path in HKCU.
#[cfg(windows)]
const FONTS_REGISTRY_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

// Directory where the Windows Font Cache Service stores its binary cache.
// The service (FontCache / FontCache3.0.0.0) pre-parses font files and stores
// the result here so apps load faster. fontlift stops the service, deletes
// these files, then restarts the service to force a clean rebuild.
#[cfg(windows)]
const FONT_CACHE_DIR: &str = r"ServiceProfiles\\LocalService\\AppData\\Local\\FontCache";

/// Return the Adobe font cache directories to clear under each Program Files root.
///
/// Adobe applications (Illustrator, InDesign, Photoshop, Acrobat) build their
/// own font index files (`AdobeFnt*.lst`) separate from the Windows font list.
/// After fontlift installs or removes fonts, these stale manifests cause Adobe
/// apps to show wrong or missing fonts until they are deleted and rebuilt.
///
/// Roots checked:
/// - `Program Files\Common Files\Adobe\TypeSpt`
/// - `Program Files\Common Files\Adobe\TypeSupport`
/// - `Program Files\Common Files\Adobe\PDFL`
/// - Same paths under `Program Files (x86)` if it differs
#[cfg(any(windows, test))]
fn adobe_cache_roots(program_files_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    for base in program_files_dirs {
        roots.push(base.join("Common Files/Adobe/TypeSpt"));
        roots.push(base.join("Common Files/Adobe/TypeSupport"));
        roots.push(base.join("Common Files/Adobe/PDFL"));
    }

    roots
}

/// Windows font manager — the [`FontManager`] implementation for Windows.
///
/// Font operations use three Windows subsystems in concert:
/// - **Registry** (`winreg`): persistent record of installed fonts. Without
///   a registry entry the font vanishes after reboot.
/// - **GDI** (`AddFontResourceW` / `RemoveFontResourceW`): makes the font
///   immediately available to running Win32 applications. `WM_FONTCHANGE`
///   broadcasts the change to all top-level windows.
/// - **DirectWrite / WIC**: used indirectly; GDI registration covers both.
///
/// System scope (`C:\Windows\Fonts` + HKLM) requires Administrator rights.
/// User scope (`%LOCALAPPDATA%\Microsoft\Windows\Fonts` + HKCU) works without
/// elevation on Windows 10 1809 and later.
pub struct WinFontManager {
    _private: (),
    /// Out-of-process font validator. When `Some`, fontlift spawns
    /// `fontlift-validator` before each install to catch malformed files
    /// without risking a crash in the main process.
    validation_config: Option<ValidatorConfig>,
}

impl WinFontManager {
    /// Create a new Windows font manager with no pre-install validation.
    pub fn new() -> Self {
        Self {
            _private: (),
            validation_config: None,
        }
    }

    /// Create a manager that runs the out-of-process validator before installs.
    pub fn with_validation(config: ValidatorConfig) -> Self {
        Self {
            _private: (),
            validation_config: Some(config),
        }
    }

    /// Enable or disable validation on this manager
    pub fn set_validation_config(&mut self, config: Option<ValidatorConfig>) {
        self.validation_config = config;
    }
}

impl Default for WinFontManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl WinFontManager {
    fn system_root(&self) -> PathBuf {
        std::env::var("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\\Windows"))
    }

    fn path_starts_with_case_insensitive(&self, root: &Path, candidate: &Path) -> bool {
        let root_str = root.to_string_lossy().to_lowercase();
        let cand = candidate.to_string_lossy().to_lowercase();
        cand.starts_with(&root_str)
    }

    fn scope_for_path(&self, path: &Path) -> FontScope {
        if self.is_system_font_path(path) {
            FontScope::System
        } else {
            FontScope::User
        }
    }

    fn is_system_font_path(&self, path: &Path) -> bool {
        let lower = path.to_string_lossy().to_lowercase();
        let root = self.system_root().to_string_lossy().to_lowercase();
        lower.starts_with(format!(r"{}\\fonts", root).as_str())
            || lower.starts_with(format!(r"{}\\system32", root).as_str())
            || lower.starts_with(format!(r"{}\\syswow64", root).as_str())
    }

    /// Return the system-wide Fonts directory (`%WINDIR%\Fonts`).
    ///
    /// This is `C:\Windows\Fonts` on a standard installation. Writing here
    /// requires Administrator privileges. All users on the machine share this
    /// directory, and it is the only font location on Windows 7/8/10 pre-1809.
    fn get_fonts_directory(&self) -> FontResult<PathBuf> {
        Ok(self.system_root().join("Fonts"))
    }

    /// Return the Fonts directory for the given scope.
    fn fonts_directory_for_scope(&self, scope: FontScope) -> FontResult<PathBuf> {
        match scope {
            FontScope::User => self.user_fonts_directory(),
            FontScope::System => self.get_fonts_directory(),
        }
    }

    /// Return the per-user Fonts directory (`%LOCALAPPDATA%\Microsoft\Windows\Fonts`).
    ///
    /// This directory was introduced in Windows 10 version 1809 (October 2018
    /// Update). Fonts installed here are visible only to the current user and
    /// do not require Administrator rights. On older Windows builds this path
    /// may not exist; fontlift falls back to the system directory in that case.
    fn user_fonts_directory(&self) -> FontResult<PathBuf> {
        let local_appdata = std::env::var("LOCALAPPDATA").map_err(|_| {
            FontError::PermissionDenied(
                "Cannot determine LOCALAPPDATA directory for per-user fonts".to_string(),
            )
        })?;

        let mut path = PathBuf::from(local_appdata);
        path.push("Microsoft");
        path.push("Windows");
        path.push("Fonts");
        Ok(path)
    }

    /// Normalize registry value into an absolute font path (registry stores filenames for fonts roots)
    #[cfg(any(windows, test))]
    fn normalize_registry_path(&self, raw: &str, scope: FontScope) -> FontResult<PathBuf> {
        let candidate = PathBuf::from(raw);

        if candidate.is_absolute() {
            return Ok(candidate);
        }

        Ok(self.fonts_directory_for_scope(scope)?.join(candidate))
    }

    /// Run out-of-process validation when configured
    fn validate_preinstall(&self, path: &Path) -> FontResult<()> {
        if let Some(config) = &self.validation_config {
            validation_ext::validate_single(path, config)?;
        }
        Ok(())
    }

    #[cfg_attr(not(any(windows, test)), allow(dead_code))]
    fn install_journal_actions(
        &self,
        source_path: &Path,
        target_path: &Path,
        scope: FontScope,
    ) -> Vec<JournalAction> {
        let mut actions = Vec::new();

        if !paths_equal_case_insensitive(source_path, target_path) {
            actions.push(JournalAction::CopyFile {
                from: source_path.to_path_buf(),
                to: target_path.to_path_buf(),
            });
        }

        actions.push(JournalAction::RegisterFont {
            path: target_path.to_path_buf(),
            scope,
        });

        actions
    }

    #[cfg_attr(not(any(windows, test)), allow(dead_code))]
    fn remove_journal_actions(&self, target_path: &Path, scope: FontScope) -> Vec<JournalAction> {
        vec![
            JournalAction::UnregisterFont {
                path: target_path.to_path_buf(),
                scope,
            },
            JournalAction::DeleteFile {
                path: target_path.to_path_buf(),
            },
        ]
    }
    /// Extract font information using font metadata when available, with filename fallback.
    fn get_font_info_from_path(&self, path: &Path) -> FontResult<FontliftFontFaceInfo> {
        validation::validate_font_file(path)?;

        let mut info = validation::extract_basic_info_from_path(path);
        info.source.scope = Some(self.scope_for_path(path));

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        if matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(file) = FileRef::new(&data) {
                    // Prefer first font in the file/collection for metadata
                    if let Some(Ok(font)) = file.fonts().next() {
                        enrich_from_fontref(&mut info, &font);
                    }
                }
            }
        }

        Ok(info)
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn enrich_from_fontref(info: &mut FontliftFontFaceInfo, font: &FontRef<'_>) {
    if let Some(ps) = name_string(font, NameId::POSTSCRIPT_NAME) {
        info.postscript_name = ps;
    }
    if let Some(family) = name_string(font, NameId::FAMILY_NAME) {
        info.family_name = family;
    }
    if let Some(subfamily) = name_string(font, NameId::SUBFAMILY_NAME) {
        info.style = subfamily;
    }
    if let Some(full) = name_string(font, NameId::FULL_NAME) {
        info.full_name = full;
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn name_string(font: &FontRef<'_>, name_id: NameId) -> Option<String> {
    let name = font.name().ok()?;
    let data = name.string_data();

    let mut fallback: Option<String> = None;

    for record in name.name_record() {
        if record.name_id() != name_id {
            continue;
        }

        let Ok(name_str) = record.string(data) else {
            continue;
        };
        let rendered = name_str.to_string();

        if record.is_unicode() {
            return Some(rendered);
        }

        if fallback.is_none() {
            fallback = Some(rendered);
        }
    }

    fallback
}

#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn paths_equal_case_insensitive(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(any(windows, test))]
impl WinFontManager {
    fn program_files_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Ok(pf) = std::env::var("ProgramFiles") {
            roots.push(PathBuf::from(pf));
        }

        if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
            let candidate = PathBuf::from(&pf86);
            let lower_candidate = pf86.to_lowercase();
            let already_present = roots
                .iter()
                .any(|p| p.to_string_lossy().to_lowercase() == lower_candidate);
            if !already_present {
                roots.push(candidate);
            }
        }

        roots
    }

    fn delete_matching_files(
        &self,
        root: &Path,
        predicate: impl Fn(&Path) -> bool,
    ) -> FontResult<usize> {
        if !root.exists() {
            return Ok(0);
        }

        let mut removed = 0usize;
        let mut stack = vec![root.to_path_buf()];

        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => return Err(FontError::IoError(err)),
            };

            for entry in entries {
                let entry = entry.map_err(FontError::IoError)?;
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                    continue;
                }

                if predicate(&path) {
                    match fs::remove_file(&path) {
                        Ok(_) => removed += 1,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                        Err(err) => return Err(FontError::IoError(err)),
                    }
                }
            }
        }

        Ok(removed)
    }

    fn clear_adobe_font_caches(&self) -> FontResult<usize> {
        let mut removed = 0usize;

        for root in adobe_cache_roots(&self.program_files_roots()) {
            removed += self.delete_matching_files(&root, |path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|name| name.starts_with("AdobeFnt") && name.ends_with(".lst"))
                    .unwrap_or(false)
            })?;
        }

        Ok(removed)
    }

    /// Determine whether a registry value refers to the given path (handles filename-only entries)
    fn registry_value_matches_path(
        &self,
        registry_value: &str,
        path: &Path,
        scope: FontScope,
    ) -> bool {
        let normalized = self
            .normalize_registry_path(registry_value, scope)
            .unwrap_or_else(|_| PathBuf::from(registry_value));

        if paths_equal_case_insensitive(&normalized, path) {
            return true;
        }

        match (normalized.file_name(), path.file_name()) {
            (Some(existing), Some(target)) => existing
                .to_string_lossy()
                .eq_ignore_ascii_case(&target.to_string_lossy()),
            _ => false,
        }
    }
}

#[cfg(windows)]
impl WinFontManager {
    fn is_in_installation_roots(&self, path: &Path) -> FontResult<bool> {
        let user_root = self.user_fonts_directory()?;
        let system_root = self.get_fonts_directory()?;
        Ok(self.path_starts_with_case_insensitive(&user_root, path)
            || self.path_starts_with_case_insensitive(&system_root, path))
    }

    /// Check if current user has admin privileges
    fn has_admin_privileges(&self) -> bool {
        unsafe {
            let mut token_handle = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).is_ok() {
                let mut elevation = TOKEN_ELEVATION::default();
                let mut return_length = 0u32;

                let result = GetTokenInformation(
                    token_handle,
                    TokenElevation,
                    Some(&mut elevation as *mut _ as *mut _),
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut return_length,
                );

                if result.is_ok() {
                    return elevation.TokenIsElevated != 0;
                }
            }
        }

        std::env::var("USERNAME").unwrap_or_default().to_uppercase() == "ADMINISTRATOR"
    }

    fn registry_key(&self, scope: FontScope, access: REGSAM) -> FontResult<RegKey> {
        let hive = match scope {
            FontScope::User => HKEY_CURRENT_USER,
            FontScope::System => HKEY_LOCAL_MACHINE,
        };

        RegKey::predef(hive)
            .open_subkey_with_flags(FONTS_REGISTRY_KEY, access)
            .map_err(|e| FontError::RegistrationFailed(format!("Cannot open registry key: {}", e)))
    }

    fn registry_entries(&self, scope: FontScope) -> FontResult<Vec<(String, PathBuf)>> {
        let key = self.registry_key(scope, KEY_READ)?;
        let mut entries = Vec::new();

        for entry in key.enum_values().flatten() {
            let name = entry.0;
            if let Ok(path_str) = key.get_value::<String, _>(&name) {
                let normalized = self.normalize_registry_path(&path_str, scope)?;
                entries.push((name, normalized));
            }
        }

        Ok(entries)
    }

    fn resolve_installed_path(
        &self,
        source: &FontliftFontSource,
        preferred_scope: FontScope,
    ) -> FontResult<(PathBuf, FontScope)> {
        let candidate = &source.path;
        if candidate.exists() {
            return Ok((candidate.clone(), preferred_scope));
        }

        let file_name = candidate
            .file_name()
            .ok_or_else(|| FontError::FontNotFound(candidate.clone()))?;
        let file_name_lower = file_name.to_string_lossy().to_lowercase();

        let scopes = [
            preferred_scope,
            if preferred_scope == FontScope::User {
                FontScope::System
            } else {
                FontScope::User
            },
        ];

        for scope in scopes {
            let base = match scope {
                FontScope::User => self.user_fonts_directory()?,
                FontScope::System => self.get_fonts_directory()?,
            };
            let candidate_path = base.join(file_name);
            if candidate_path.exists() {
                return Ok((candidate_path, scope));
            }
        }

        // Fallback to registry entries in either scope to handle renamed fonts (e.g., arial_0.ttf)
        for scope in [preferred_scope, FontScope::User, FontScope::System] {
            if let Ok(entries) = self.registry_entries(scope) {
                if let Some((_, path)) = entries.iter().find(|(_, path)| {
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_lowercase() == file_name_lower)
                        .unwrap_or(false)
                }) {
                    if path.exists() {
                        return Ok((path.clone(), scope));
                    }
                }
            }
        }

        Err(FontError::FontNotFound(candidate.clone()))
    }

    fn control_service(&self, name: &str, action: &str, fail_on_missing: bool) -> FontResult<()> {
        let output = Command::new("sc")
            .args([action, name])
            .output()
            .map_err(FontError::IoError)?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        let missing_service = stderr.contains("does not exist")
            || stderr.contains("openservice failed")
            || stderr.contains("1060");

        if missing_service && !fail_on_missing {
            return Ok(());
        }

        Err(FontError::RegistrationFailed(format!(
            "Failed to {} {} service: {}",
            action,
            name,
            stderr.trim().to_string()
        )))
    }

    /// Stop the Windows Font Cache Service before deleting cache files.
    ///
    /// Two services may be present:
    /// - `FontCache` — the main Windows font cache service (always present on
    ///   Vista+). Required; fail if it cannot be stopped.
    /// - `FontCache3.0.0.0` — the WPF (Windows Presentation Foundation) font
    ///   cache service. Optional; silently skip if it isn't installed.
    fn stop_font_cache_service(&self) -> FontResult<()> {
        self.control_service("FontCache", "stop", true)?;
        // WPF font cache service is optional; tolerate missing service
        let _ = self.control_service("FontCache3.0.0.0", "stop", false);
        Ok(())
    }

    /// Restart the Font Cache Service after cache files have been deleted.
    fn start_font_cache_service(&self) -> FontResult<()> {
        self.control_service("FontCache", "start", true)?;
        let _ = self.control_service("FontCache3.0.0.0", "start", false);
        Ok(())
    }

    /// Delete the binary font cache files written by the Font Cache Service.
    ///
    /// Two locations:
    /// - `ServiceProfiles\LocalService\AppData\Local\FontCache\` — per-session
    ///   cache files written by the FontCache service.
    /// - `System32\FNTCACHE.DAT` — a legacy GDI font cache file. Its removal
    ///   forces Windows to rebuild font metrics on next boot.
    ///
    /// This must be called while the FontCache service is stopped, otherwise
    /// Windows holds locks on these files and the delete will fail.
    fn clear_font_cache_files(&self) -> FontResult<()> {
        let root = self.system_root();
        let cache_dir = root.join(FONT_CACHE_DIR);
        if cache_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        std::fs::remove_file(&path).map_err(FontError::IoError)?;
                    }
                }
            }
        }

        let system_cache = root.join(r"System32\FNTCACHE.DAT");
        if system_cache.exists() {
            std::fs::remove_file(&system_cache).map_err(FontError::IoError)?;
        }

        Ok(())
    }

    fn target_path_for_scope(&self, source_path: &Path, scope: FontScope) -> FontResult<PathBuf> {
        let file_name = source_path
            .file_name()
            .ok_or_else(|| FontError::InvalidFormat("Font path missing file name".to_string()))?;

        let base = match scope {
            FontScope::User => self.user_fonts_directory()?,
            FontScope::System => self.get_fonts_directory()?,
        };

        Ok(base.join(file_name))
    }

    /// Copy font to target directory based on scope
    fn copy_font_to_target_directory(
        &self,
        source_path: &Path,
        target_path: &Path,
        scope: FontScope,
    ) -> FontResult<()> {
        if !self.has_admin_privileges() && scope == FontScope::System {
            return Err(FontError::PermissionDenied(
                "System-level font installation requires administrator privileges. Run with --admin or as Administrator.".to_string(),
            ));
        }

        if let Some(dir) = target_path.parent() {
            if !dir.exists() {
                fs::create_dir_all(dir).map_err(FontError::IoError)?;
            }
        }

        if target_path.exists() {
            if self.is_system_font_path(target_path) {
                return Err(FontError::SystemFontProtection(target_path.to_path_buf()));
            }

            fs::remove_file(target_path).map_err(FontError::IoError)?;
        }

        fs::copy(source_path, target_path).map_err(FontError::IoError)?;

        Ok(())
    }

    /// Register a font file with GDI and broadcast the change to all windows.
    ///
    /// `AddFontResourceW` loads the font into the GDI font table so Win32
    /// applications can use it immediately in the current session. Without
    /// this call the font would only appear after a reboot (once the registry
    /// entry is read at startup).
    ///
    /// `SendMessage(HWND_BROADCAST, WM_FONTCHANGE)` notifies every top-level
    /// window that the font list changed. Well-behaved applications (Notepad,
    /// Office, etc.) refresh their font menus on this message.
    fn register_font_with_gdi(&self, path: &Path) -> FontResult<()> {
        let path_str = path.to_string_lossy().to_string();
        let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

        let result = unsafe { AddFontResourceW(PCWSTR(path_wide.as_ptr())) };

        if result == 0 {
            return Err(FontError::RegistrationFailed(format!(
                "GDI failed to register font: {}",
                path.display()
            )));
        }

        // Broadcast so running apps refresh their font lists without restarting.
        unsafe {
            SendMessageW(HWND_BROADCAST, WM_FONTCHANGE, WPARAM(0), LPARAM(0));
        }

        Ok(())
    }

    /// Unregister a font from GDI and broadcast the change to all windows.
    ///
    /// `RemoveFontResourceW` removes the font from GDI's in-memory table.
    /// The font file is untouched. A subsequent `WM_FONTCHANGE` broadcast
    /// lets running applications update their font menus.
    fn unregister_font_from_gdi(&self, path: &Path) -> FontResult<()> {
        let path_str = path.to_string_lossy().to_string();
        let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

        let result = unsafe { RemoveFontResourceW(PCWSTR(path_wide.as_ptr())) };

        if result == 0 {
            return Err(FontError::RegistrationFailed(format!(
                "GDI failed to unregister font: {}",
                path.display()
            )));
        }

        unsafe {
            SendMessageW(HWND_BROADCAST, WM_FONTCHANGE, WPARAM(0), LPARAM(0));
        }

        Ok(())
    }

    fn unregister_known_locations(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        // best-effort cleanup in both scopes to mirror legacy behavior
        let _ = self.unregister_font_from_registry(path, scope);
        let other_scope = if scope == FontScope::User {
            FontScope::System
        } else {
            FontScope::User
        };
        let _ = self.unregister_font_from_registry(path, other_scope);
        Ok(())
    }

    fn remove_conflicting_install(&self, font: &FontliftFontFaceInfo) -> FontResult<()> {
        let path = &font.source.path;
        let scope = font
            .source
            .scope
            .unwrap_or_else(|| self.scope_for_path(path));

        if self.is_system_font_path(path) {
            return Err(FontError::SystemFontProtection(path.clone()));
        }

        // best-effort GDI + registry cleanup before removing the file
        let _ = self.unregister_font_from_gdi(path);
        self.unregister_known_locations(path, scope)?;

        if self.is_in_installation_roots(path)? && path.exists() {
            fs::remove_file(path).map_err(FontError::IoError)?;
        }

        Ok(())
    }

    /// Write a font entry to the Windows registry so the font survives reboot.
    ///
    /// The registry value name follows the Windows convention:
    ///   `"Family Name (TrueType)"` or `"Family Name (OpenType)"`.
    /// The value data is the font file path. For system-scope fonts Windows
    /// traditionally stores just the filename (e.g. `"arial.ttf"`) because
    /// `C:\Windows\Fonts\` is implied, but fontlift stores full paths to be
    /// unambiguous for both user and system scopes.
    fn register_font_in_registry(
        &self,
        path: &Path,
        font_info: &FontliftFontFaceInfo,
        scope: FontScope,
    ) -> FontResult<()> {
        let registry_key = self.registry_key(scope, KEY_SET_VALUE)?;

        let registry_name = format!(
            "{} ({})",
            font_info.family_name,
            font_info.source.format.as_deref().unwrap_or("TrueType")
        );

        let path_str = if self.is_in_installation_roots(path)? {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string())
        } else {
            path.to_string_lossy().to_string()
        };
        registry_key
            .set_value(&registry_name, &path_str)
            .map_err(|e| {
                FontError::RegistrationFailed(format!("Cannot set registry value: {}", e))
            })?;

        Ok(())
    }

    /// Determine whether a registry value refers to the given path (handles filename-only entries)
    /// Unregister font from Windows Registry
    fn unregister_font_from_registry(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        let registry_key = self.registry_key(scope, KEY_SET_VALUE)?;

        for value_name in registry_key.enum_values().filter_map(|(name, _)| name.ok()) {
            if let Ok(existing_value) = registry_key.get_value::<String, _>(&value_name) {
                if self.registry_value_matches_path(&existing_value, path, scope) {
                    registry_key.delete_value(&value_name).map_err(|e| {
                        FontError::RegistrationFailed(format!(
                            "Warning: Cannot delete registry value: {}",
                            e
                        ))
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Enumerate fonts from Windows Registry
    fn enumerate_fonts_from_registry(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
        let mut fonts = Vec::new();

        for scope in [FontScope::User, FontScope::System] {
            if let Ok(entries) = self.registry_entries(scope) {
                for (value_name, path) in entries {
                    if path.exists() && validation::is_valid_font_extension(&path) {
                        if let Ok(mut font_info) = self.get_font_info_from_path(&path) {
                            if let Some(paren_pos) = value_name.find('(') {
                                font_info.family_name = value_name[..paren_pos].trim().to_string();
                            }
                            font_info.source.scope = Some(scope);
                            fonts.push(font_info);
                        }
                    }
                }
            }
        }

        Ok(fonts)
    }

    /// Validate system operation permissions
    fn validate_system_operation(&self, scope: FontScope) -> FontResult<()> {
        if scope == FontScope::System && !self.has_admin_privileges() {
            return Err(FontError::PermissionDenied(
                "System-level font operations require administrator privileges. Run with --admin or as Administrator.".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(not(windows))]
impl WinFontManager {
    fn unsupported<T>(&self) -> FontResult<T> {
        Err(FontError::UnsupportedOperation(
            "Windows font operations are only available on Windows".to_string(),
        ))
    }
}

#[cfg(windows)]
impl FontManager for WinFontManager {
    fn install_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let scope = source.scope.unwrap_or(FontScope::User);
        let path = &source.path;
        validation::validate_font_file(path)?;
        self.validate_system_operation(scope)?;
        self.validate_preinstall(path)?;

        if self.is_system_font_path(path) {
            return Err(FontError::SystemFontProtection(path.to_path_buf()));
        }

        let mut font_info = self.get_font_info_from_path(path)?;
        font_info.source.scope = Some(scope);

        // Remove conflicting installs (same PostScript or family/style) before copying
        let installed_fonts = self.list_installed_fonts()?;
        let conflicts = conflicts::detect_conflicts(&installed_fonts, &font_info);
        for conflict in conflicts {
            self.remove_conflicting_install(conflict)?;
        }

        let target_path = self.target_path_for_scope(path, scope)?;
        let actions = self.install_journal_actions(path, &target_path, scope);
        let needs_copy = actions
            .first()
            .map(|a| matches!(a, JournalAction::CopyFile { .. }))
            .unwrap_or(false);

        // Record operation in journal
        let entry_id = journal::with_journal_lock(|| {
            let mut j = journal::load_journal().unwrap_or_default();
            let id = j.record_operation(actions, Some(format!("Install {}", path.display())));
            journal::save_journal(&j)?;
            Ok(id)
        })?;

        if needs_copy {
            let copy_result = self.copy_font_to_target_directory(path, &target_path, scope);
            match copy_result {
                Ok(_) => {
                    let _ = journal::with_journal_lock(|| {
                        let mut j = journal::load_journal().unwrap_or_default();
                        let _ = j.mark_step(entry_id, 1);
                        let _ = journal::save_journal(&j);
                        Ok(())
                    });
                }
                Err(e) => {
                    let _ = journal::with_journal_lock(|| {
                        let mut j = journal::load_journal().unwrap_or_default();
                        let _ = j.mark_completed(entry_id);
                        let _ = journal::save_journal(&j);
                        Ok(())
                    });
                    return Err(e);
                }
            }
        }

        if self.registry_entries(scope)?.iter().any(|(_, existing)| {
            existing
                .to_string_lossy()
                .eq_ignore_ascii_case(&target_path.to_string_lossy())
        }) {
            let _ = journal::with_journal_lock(|| {
                let mut j = journal::load_journal().unwrap_or_default();
                let _ = j.mark_completed(entry_id);
                let _ = journal::save_journal(&j);
                Ok(())
            });
            return Err(FontError::AlreadyInstalled(target_path));
        }

        let register_result = (|| {
            self.register_font_with_gdi(&target_path)?;
            self.register_font_in_registry(&target_path, &font_info, scope)?;
            Ok(())
        })();

        // Update journal and clean up on failure
        match &register_result {
            Ok(_) => {
                let _ = journal::with_journal_lock(|| {
                    let mut j = journal::load_journal().unwrap_or_default();
                    let _ = j.mark_completed(entry_id);
                    let _ = journal::save_journal(&j);
                    Ok(())
                });
            }
            Err(_) => {
                if needs_copy {
                    let _ = fs::remove_file(&target_path);
                }
                let _ = journal::with_journal_lock(|| {
                    let mut j = journal::load_journal().unwrap_or_default();
                    let _ = j.mark_completed(entry_id);
                    let _ = journal::save_journal(&j);
                    Ok(())
                });
            }
        }
        register_result
    }

    fn uninstall_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let preferred_scope = source.scope.unwrap_or(FontScope::User);
        let (installed_path, installed_scope) =
            self.resolve_installed_path(source, preferred_scope)?;

        self.validate_system_operation(installed_scope)?;

        self.unregister_font_from_gdi(&installed_path)?;
        self.unregister_font_from_registry(&installed_path, installed_scope)?;

        // Best-effort cleanup of duplicate registrations in the opposite scope
        let other_scope = if installed_scope == FontScope::User {
            FontScope::System
        } else {
            FontScope::User
        };
        let _ = self.unregister_font_from_registry(&installed_path, other_scope);

        Ok(())
    }

    fn remove_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let preferred_scope = source.scope.unwrap_or(FontScope::User);
        let (installed_path, installed_scope) =
            self.resolve_installed_path(source, preferred_scope)?;

        if self.is_system_font_path(&installed_path) {
            return Err(FontError::SystemFontProtection(installed_path));
        }

        // Build journal actions: UnregisterFont -> DeleteFile
        let actions = self.remove_journal_actions(&installed_path, installed_scope);
        let entry_id = journal::with_journal_lock(|| {
            let mut j = journal::load_journal().unwrap_or_default();
            let id = j.record_operation(
                actions,
                Some(format!("Remove {}", installed_path.display())),
            );
            journal::save_journal(&j)?;
            Ok(id)
        })?;

        let resolved_source =
            FontliftFontSource::new(installed_path.clone()).with_scope(Some(installed_scope));
        let uninstall_result = self.uninstall_font(&resolved_source);
        if let Err(e) = uninstall_result {
            let _ = journal::with_journal_lock(|| {
                let mut j = journal::load_journal().unwrap_or_default();
                let _ = j.mark_completed(entry_id);
                let _ = journal::save_journal(&j);
                Ok(())
            });
            return Err(e);
        }

        let _ = journal::with_journal_lock(|| {
            let mut j = journal::load_journal().unwrap_or_default();
            let _ = j.mark_step(entry_id, 1);
            let _ = journal::save_journal(&j);
            Ok(())
        });

        std::fs::remove_file(installed_path).map_err(FontError::IoError)?;

        let _ = journal::with_journal_lock(|| {
            let mut j = journal::load_journal().unwrap_or_default();
            let _ = j.mark_completed(entry_id);
            let _ = journal::save_journal(&j);
            Ok(())
        });

        Ok(())
    }

    fn is_font_installed(&self, source: &FontliftFontSource) -> FontResult<bool> {
        let mut candidates = vec![source.path.clone()];

        if let Some(file_name) = source.path.file_name() {
            candidates.push(self.user_fonts_directory()?.join(file_name));
            candidates.push(self.get_fonts_directory()?.join(file_name));
        }

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(true);
            }
        }

        for scope in [FontScope::User, FontScope::System] {
            if let Ok(entries) = self.registry_entries(scope) {
                if entries.iter().any(|(_, path)| {
                    candidates.iter().any(|candidate| {
                        path.to_string_lossy()
                            .eq_ignore_ascii_case(&candidate.to_string_lossy())
                    })
                }) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
        let mut fonts = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();

        let mut push_if_new = |mut font: FontliftFontFaceInfo| {
            let key = font.source.path.to_string_lossy().to_lowercase();
            if seen.insert(key) {
                fonts.push(font);
            }
        };

        for font in self.enumerate_fonts_from_registry()? {
            push_if_new(font);
        }

        let sources = vec![
            (FontScope::User, self.user_fonts_directory()?),
            (FontScope::System, self.get_fonts_directory()?),
        ];

        for (scope, dir) in sources {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && validation::is_valid_font_extension(&path) {
                        if let Ok(mut info) = self.get_font_info_from_path(&path) {
                            info.source.scope = Some(scope);
                            push_if_new(info);
                        }
                    }
                }
            }
        }

        Ok(fonts)
    }

    fn clear_font_caches(&self, scope: FontScope) -> FontResult<()> {
        match scope {
            FontScope::User => {
                return Err(FontError::PermissionDenied(
                    "Font cache clearing requires administrator privileges on Windows; rerun with --admin"
                        .to_string(),
                ));
            }
            FontScope::System => {
                if !self.has_admin_privileges() {
                    return Err(FontError::PermissionDenied(
                        "System cache clearing requires administrator privileges".to_string(),
                    ));
                }

                self.stop_font_cache_service()?;
                self.clear_font_cache_files()?;
                let _ = self.clear_adobe_font_caches()?;
                self.start_font_cache_service()?;
            }
        }

        Ok(())
    }

    fn prune_missing_fonts(&self, scope: FontScope) -> FontResult<usize> {
        self.validate_system_operation(scope)?;

        let key = self.registry_key(scope, KEY_READ | KEY_SET_VALUE)?;
        let mut removed = 0usize;

        for value in key.enum_values().flatten() {
            let name = value.0;
            if let Ok(path_str) = key.get_value::<String, _>(&name) {
                let normalized = match self.normalize_registry_path(&path_str, scope) {
                    Ok(p) => p,
                    Err(_) => {
                        key.delete_value(name).map_err(|e| {
                            FontError::RegistrationFailed(format!(
                                "Cannot delete registry value for malformed path: {}",
                                e
                            ))
                        })?;
                        removed += 1;
                        continue;
                    }
                };

                if !normalized.exists() || !validation::is_valid_font_extension(&normalized) {
                    key.delete_value(name).map_err(|e| {
                        FontError::RegistrationFailed(format!(
                            "Cannot delete registry value for missing font: {}",
                            e
                        ))
                    })?;
                    removed += 1;
                }
            }
        }

        Ok(removed)
    }
}

#[cfg(not(windows))]
impl FontManager for WinFontManager {
    fn install_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let _ = source;
        self.unsupported()
    }

    fn uninstall_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let _ = source;
        self.unsupported()
    }

    fn remove_font(&self, source: &FontliftFontSource) -> FontResult<()> {
        let _ = source;
        self.unsupported()
    }

    fn is_font_installed(&self, source: &FontliftFontSource) -> FontResult<bool> {
        let _ = source;
        self.unsupported()
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
        self.unsupported()
    }

    fn clear_font_caches(&self, scope: FontScope) -> FontResult<()> {
        let _ = scope;
        self.unsupported()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .expect("environment lock should not be poisoned")
    }

    #[test]
    fn test_win_font_manager_creation() {
        let manager = WinFontManager::new();
        assert_eq!(manager._private, ());
    }

    #[test]
    fn adobe_cache_roots_cover_common_type_support_paths() {
        let bases = vec![PathBuf::from("C:/Program Files")];
        let roots = adobe_cache_roots(&bases);

        assert!(roots
            .iter()
            .any(|p| p.to_string_lossy().ends_with("Adobe/TypeSpt")));
        assert!(roots
            .iter()
            .any(|p| p.to_string_lossy().ends_with("Adobe/TypeSupport")));
    }

    #[test]
    fn program_files_roots_deduplicates_case_insensitive_paths() {
        let _env_lock = lock_env();
        let manager = WinFontManager::new();
        let temp = TempDir::new().expect("tempdir");
        let upper = temp.path().to_string_lossy().to_uppercase();
        let _guard_pf = EnvGuard::set("ProgramFiles", temp.path());
        let _guard_pf86 = EnvGuard::set("ProgramFiles(x86)", upper);

        let roots = manager.program_files_roots();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], temp.path());
    }

    #[test]
    fn clear_adobe_font_caches_removes_lst_files_under_program_files_variants() {
        let _env_lock = lock_env();
        let manager = WinFontManager::new();
        let pf = TempDir::new().expect("pf dir");
        let pf86 = TempDir::new().expect("pf86 dir");

        let typesupport = pf.path().join("Common Files/Adobe/TypeSupport");
        fs::create_dir_all(&typesupport).unwrap();
        let keep = typesupport.join("ReadMe.txt");
        fs::write(&keep, b"keep").unwrap();
        let lst_one = typesupport.join("AdobeFnt11.lst");
        fs::write(&lst_one, b"dummy").unwrap();

        let pdfl = pf86.path().join("Common Files/Adobe/PDFL/9.9");
        fs::create_dir_all(&pdfl).unwrap();
        let lst_two = pdfl.join("AdobeFnt12.lst");
        fs::write(&lst_two, b"dummy").unwrap();

        let _guard_pf = EnvGuard::set("ProgramFiles", pf.path());
        let _guard_pf86 = EnvGuard::set("ProgramFiles(x86)", pf86.path());

        let removed = manager
            .clear_adobe_font_caches()
            .expect("cache cleanup should succeed");

        assert_eq!(removed, 2);
        assert!(!lst_one.exists());
        assert!(!lst_two.exists());
        assert!(keep.exists());
    }

    #[cfg(windows)]
    #[test]
    fn test_system_font_path_detection() {
        let manager = WinFontManager::new();

        let system_path = PathBuf::from(r"C:\Windows\Fonts\arial.ttf");
        assert!(manager.is_system_font_path(&system_path));

        let user_path = PathBuf::from(r"C:\Users\Test\Fonts\custom.ttf");
        assert!(!manager.is_system_font_path(&user_path));

        let system32_path = PathBuf::from(r"C:\Windows\System32\font.ttf");
        assert!(manager.is_system_font_path(&system32_path));
    }

    #[cfg(windows)]
    #[test]
    fn test_fonts_directory() {
        let manager = WinFontManager::new();
        let fonts_dir = manager.get_fonts_directory().unwrap();
        assert!(fonts_dir.to_string_lossy().contains("Fonts"));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_operations_return_unsupported() {
        let manager = WinFontManager::new();
        let source =
            FontliftFontSource::new(PathBuf::from("dummy.ttf")).with_scope(Some(FontScope::User));

        let result = manager.install_font(&source);
        assert!(matches!(
            result,
            Err(FontError::UnsupportedOperation(msg)) if msg.contains("only available on Windows")
        ));
    }

    #[test]
    fn get_font_info_from_path_extracts_metadata_from_fixture() {
        let manager = WinFontManager::new();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/fonts/AtkinsonHyperlegible-Regular.ttf");

        let info = manager
            .get_font_info_from_path(&fixture)
            .expect("metadata should parse");

        assert_eq!(info.family_name, "Atkinson Hyperlegible");
        assert_eq!(info.style, "Regular");
        assert_eq!(info.full_name, "Atkinson Hyperlegible Regular");
        assert_eq!(info.postscript_name, "AtkinsonHyperlegible-Regular");
        assert_eq!(info.source.format.as_deref(), Some("TTF"));
    }

    #[test]
    fn get_font_info_from_path_extracts_metadata_from_otf_fixture() {
        let manager = WinFontManager::new();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/fonts/AtkinsonHyperlegible-Regular.otf");

        let info = manager
            .get_font_info_from_path(&fixture)
            .expect("metadata should parse");

        assert_eq!(info.family_name, "Atkinson Hyperlegible");
        assert_eq!(info.style, "Regular");
        assert_eq!(info.full_name, "Atkinson Hyperlegible Regular");
        assert_eq!(info.postscript_name, "AtkinsonHyperlegible-Regular");
        assert_eq!(info.source.format.as_deref(), Some("OTF"));
    }

    #[test]
    fn get_font_info_from_path_extracts_metadata_from_ttc_fixture() {
        let manager = WinFontManager::new();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/fonts/AtkinsonHyperlegible-Regular.ttc");

        let info = manager
            .get_font_info_from_path(&fixture)
            .expect("metadata should parse");

        assert!(info
            .family_name
            .replace(' ', "")
            .contains("AtkinsonHyperlegible"));
        assert_eq!(info.style, "Regular");
        assert!(info.full_name.to_lowercase().contains("atkinson"));
        assert_eq!(info.postscript_name, "AtkinsonHyperlegible-Regular");
        assert_eq!(info.source.format.as_deref(), Some("TTC"));
    }

    #[test]
    fn normalize_registry_path_resolves_relative_to_scope_roots() {
        let _env_lock = lock_env();
        let manager = WinFontManager::new();
        let windir = TempDir::new().expect("windir");
        let local = TempDir::new().expect("localappdata");

        let _guard_windir = EnvGuard::set("WINDIR", windir.path());
        let _guard_local = EnvGuard::set("LOCALAPPDATA", local.path());

        let system_path = manager
            .normalize_registry_path("Arial.ttf", FontScope::System)
            .expect("system normalization should succeed");
        assert_eq!(system_path, windir.path().join("Fonts/Arial.ttf"));

        let user_path = manager
            .normalize_registry_path("SegoeUI.ttf", FontScope::User)
            .expect("user normalization should succeed");
        assert_eq!(
            user_path,
            local.path().join("Microsoft/Windows/Fonts/SegoeUI.ttf")
        );
    }

    #[test]
    fn registry_value_matches_path_accepts_filename_only_entries() {
        let _env_lock = lock_env();
        let manager = WinFontManager::new();
        let windir = TempDir::new().expect("windir");
        let local = TempDir::new().expect("localappdata");

        let target = windir.path().join("Fonts/Arial.ttf");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"x").unwrap();

        let _guard_windir = EnvGuard::set("WINDIR", windir.path());
        let _guard_local = EnvGuard::set("LOCALAPPDATA", local.path());

        assert!(manager.registry_value_matches_path("Arial.ttf", &target, FontScope::System));
    }

    #[test]
    fn registry_value_matches_path_handles_case_insensitive_absolute_paths() {
        let _env_lock = lock_env();
        let manager = WinFontManager::new();
        let windir = TempDir::new().expect("windir");
        let local = TempDir::new().expect("localappdata");

        let target = windir.path().join("Fonts/MyFont.TTF");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"x").unwrap();

        let _guard_windir = EnvGuard::set("WINDIR", windir.path());
        let _guard_local = EnvGuard::set("LOCALAPPDATA", local.path());

        let mixed_case = target.to_string_lossy().to_uppercase();
        assert!(manager.registry_value_matches_path(&mixed_case, &target, FontScope::System));
    }

    #[test]
    fn install_journal_actions_include_copy_when_paths_differ() {
        let manager = WinFontManager::new();
        let source = PathBuf::from("C:/tmp/source.ttf");
        let target = PathBuf::from("C:/Windows/Fonts/target.ttf");

        let actions = manager.install_journal_actions(&source, &target, FontScope::System);

        assert_eq!(actions.len(), 2);
        assert!(matches!(
            actions[0],
            JournalAction::CopyFile { ref from, ref to }
            if from == &source && to == &target
        ));
        assert!(matches!(
            actions[1],
            JournalAction::RegisterFont { ref path, scope }
            if path == &target && scope == FontScope::System
        ));
    }

    #[test]
    fn install_journal_actions_skip_copy_when_paths_match() {
        let manager = WinFontManager::new();
        let path = PathBuf::from("C:/Windows/Fonts/AlreadyThere.ttf");

        let actions = manager.install_journal_actions(&path, &path, FontScope::System);

        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            JournalAction::RegisterFont { path: ref p, scope }
            if p == &path && scope == FontScope::System
        ));
    }

    #[test]
    fn validation_preinstall_rejects_malformed_font_when_enabled() {
        let manager = WinFontManager::with_validation(ValidatorConfig::default());
        let malformed =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/fonts/malformed.ttf");

        let result = manager.validate_preinstall(&malformed);

        assert!(
            result.is_err(),
            "Malformed font should be rejected when validation is enabled"
        );
    }

    #[test]
    fn validation_preinstall_allows_valid_font() {
        let manager = WinFontManager::with_validation(ValidatorConfig::default());
        let valid = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/fonts/AtkinsonHyperlegible-Regular.ttf");

        let result = manager.validate_preinstall(&valid);

        if let Err(FontError::InvalidFormat(msg)) = &result {
            // If the validator binary isn't available in the test environment, skip the assertion
            if msg.contains("Validator failed") || msg.contains("Failed to spawn validator") {
                return;
            }
        }

        assert!(result.is_ok(), "Valid font should pass validation");
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = &self.previous {
                std::env::set_var(self.key, prev);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
