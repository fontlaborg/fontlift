//! Windows platform implementation for fontlift
//!
//! This module provides Windows-specific font management using Windows APIs,
//! implementing the same functionality as the C++ CLI but in Rust.

#[cfg(windows)]
use fontlift_core::conflicts;
use fontlift_core::validation;
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

#[cfg(windows)]
const FONTS_REGISTRY_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";
#[cfg(windows)]
const FONT_CACHE_DIR: &str = r"ServiceProfiles\\LocalService\\AppData\\Local\\FontCache";

/// Common Adobe font cache roots under Program Files variants
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

/// Windows font manager using Windows Registry and GDI APIs
pub struct WinFontManager {
    _private: (),
}

impl WinFontManager {
    /// Create a new Windows font manager
    pub fn new() -> Self {
        Self { _private: () }
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
}

#[cfg(windows)]
impl WinFontManager {
    /// Get Windows fonts directory
    fn get_fonts_directory(&self) -> FontResult<PathBuf> {
        Ok(self.system_root().join("Fonts"))
    }

    fn user_fonts_directory(&self) -> FontResult<PathBuf> {
        let local_appdata = std::env::var("LOCALAPPDATA").map_err(|_| {
            FontError::PermissionDenied(
                "Cannot determine LOCALAPPDATA directory for per-user fonts".to_string(),
            )
        })?;

        Ok(PathBuf::from(local_appdata).join(r"Microsoft\Windows\Fonts"))
    }

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
                entries.push((name, PathBuf::from(path_str)));
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

        Err(FontError::FontNotFound(candidate.clone()))
    }

    fn stop_font_cache_service(&self) -> FontResult<()> {
        let output = Command::new("sc")
            .args(["stop", "FontCache"])
            .output()
            .map_err(FontError::IoError)?;

        if !output.status.success() {
            return Err(FontError::RegistrationFailed(
                "Failed to stop FontCache service (requires administrator privileges)".to_string(),
            ));
        }

        Ok(())
    }

    fn start_font_cache_service(&self) -> FontResult<()> {
        let output = Command::new("sc")
            .args(["start", "FontCache"])
            .output()
            .map_err(FontError::IoError)?;

        if !output.status.success() {
            return Err(FontError::RegistrationFailed(
                "Failed to start FontCache service after cache clear".to_string(),
            ));
        }

        Ok(())
    }

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

    /// Copy font to target directory based on scope
    fn copy_font_to_target_directory(
        &self,
        source_path: &Path,
        scope: FontScope,
    ) -> FontResult<PathBuf> {
        let target_dir = match scope {
            FontScope::User => self.user_fonts_directory()?,
            FontScope::System => {
                if !self.has_admin_privileges() {
                    return Err(FontError::PermissionDenied(
                        "System-level font installation requires administrator privileges. Run with --admin or as Administrator.".to_string(),
                    ));
                }
                self.get_fonts_directory()?
            }
        };

        if !target_dir.exists() {
            fs::create_dir_all(&target_dir).map_err(FontError::IoError)?;
        }

        let target_path = target_dir.join(source_path.file_name().unwrap());
        if target_path.exists() {
            if self.is_system_font_path(&target_path) {
                return Err(FontError::SystemFontProtection(target_path.clone()));
            }

            fs::remove_file(&target_path).map_err(FontError::IoError)?;
        }

        fs::copy(source_path, &target_path).map_err(FontError::IoError)?;

        Ok(target_path)
    }

    /// Register font with Windows GDI
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

        unsafe {
            SendMessageW(HWND_BROADCAST, WM_FONTCHANGE, WPARAM(0), LPARAM(0));
        }

        Ok(())
    }

    /// Unregister font from Windows GDI
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

    /// Register font in Windows Registry
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

        let path_str = path.to_string_lossy().to_string();
        registry_key
            .set_value(&registry_name, &path_str)
            .map_err(|e| {
                FontError::RegistrationFailed(format!("Cannot set registry value: {}", e))
            })?;

        Ok(())
    }

    /// Unregister font from Windows Registry
    fn unregister_font_from_registry(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        let registry_key = self.registry_key(scope, KEY_SET_VALUE)?;

        let path_str = path.to_string_lossy().to_string();

        for value_name in registry_key.enum_values().filter_map(|(name, _)| name.ok()) {
            if let Ok(existing_path) = registry_key.get_value::<String, _>(&value_name) {
                if existing_path == path_str {
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

        let target_path = self.copy_font_to_target_directory(path, scope)?;

        if self.registry_entries(scope)?.iter().any(|(_, existing)| {
            existing
                .to_string_lossy()
                .eq_ignore_ascii_case(&target_path.to_string_lossy())
        }) {
            return Err(FontError::AlreadyInstalled(target_path));
        }

        self.register_font_with_gdi(&target_path)?;
        self.register_font_in_registry(&target_path, &font_info, scope)?;

        Ok(())
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

        let resolved_source =
            FontliftFontSource::new(installed_path.clone()).with_scope(Some(installed_scope));
        self.uninstall_font(&resolved_source)?;

        std::fs::remove_file(installed_path).map_err(FontError::IoError)?;

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
                let path = PathBuf::from(path_str);
                if !path.exists() || !validation::is_valid_font_extension(&path) {
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
    use tempfile::TempDir;

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
            .join("../../tests/fixtures/fonts/AtkinsonHyperlegible-Regular.ttf");

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
            .join("../../tests/fixtures/fonts/AtkinsonHyperlegible-Regular.otf");

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
            .join("../../tests/fixtures/fonts/AtkinsonHyperlegible-Regular.ttc");

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
