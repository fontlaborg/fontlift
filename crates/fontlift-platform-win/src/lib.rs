//! Windows platform implementation for fontlift
//!
//! This module provides Windows-specific font management using Windows APIs,
//! implementing the same functionality as the C++ CLI but in Rust.

use fontlift_core::{FontError, FontInfo, FontManager, FontResult, FontScope};
use std::path::Path;

#[cfg(windows)]
use fontlift_core::validation;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
use std::fs;

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

#[cfg(windows)]
impl WinFontManager {
    /// Get Windows fonts directory
    fn get_fonts_directory(&self) -> FontResult<PathBuf> {
        if let Ok(windir) = std::env::var("WINDIR") {
            return Ok(PathBuf::from(windir).join("Fonts"));
        }

        Ok(PathBuf::from(r"C:\Windows\Fonts"))
    }

    /// Check if path is in system font directory
    fn is_system_font_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        path_str.starts_with(r"c:\windows\fonts")
            || path_str.contains("system32")
            || path_str.contains("syswow64")
            || path_str.starts_with(r"c:\windows\system32")
    }

    /// Extract font information using basic filename parsing as fallback
    fn get_font_info_from_path(&self, path: &Path) -> FontResult<FontInfo> {
        validation::validate_font_file(path)?;

        let info = validation::extract_basic_info_from_path(path);
        Ok(info)
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

    /// Copy font to target directory based on scope
    fn copy_font_to_target_directory(
        &self,
        source_path: &Path,
        scope: FontScope,
    ) -> FontResult<PathBuf> {
        let target_dir = match scope {
            FontScope::User => {
                let local_appdata = std::env::var("LOCALAPPDATA").map_err(|_| {
                    FontError::PermissionDenied(
                        "Cannot determine LOCALAPPDATA directory".to_string(),
                    )
                })?;
                PathBuf::from(local_appdata).join("Microsoft\\Windows\\Fonts")
            }
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
            return Err(FontError::AlreadyInstalled(target_path.clone()));
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

    /// Register font in Windows Registry
    fn register_font_in_registry(
        &self,
        path: &Path,
        font_info: &FontInfo,
        scope: FontScope,
    ) -> FontResult<()> {
        let hive = match scope {
            FontScope::User => HKEY_CURRENT_USER,
            FontScope::System => HKEY_LOCAL_MACHINE,
        };

        let key_path = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

        let registry_key = RegKey::predef(hive)
            .open_subkey_with_flags(key_path, KEY_SET_VALUE)
            .map_err(|e| {
                FontError::RegistrationFailed(format!("Cannot open registry key: {}", e))
            })?;

        let registry_name = format!(
            "{} ({})",
            font_info.family_name,
            font_info.format.as_deref().unwrap_or("TrueType")
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
        let hive = match scope {
            FontScope::User => HKEY_CURRENT_USER,
            FontScope::System => HKEY_LOCAL_MACHINE,
        };

        let key_path = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

        let registry_key = RegKey::predef(hive)
            .open_subkey_with_flags(key_path, KEY_SET_VALUE)
            .map_err(|e| {
                FontError::RegistrationFailed(format!("Cannot open registry key: {}", e))
            })?;

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
    fn enumerate_fonts_from_registry(&self) -> FontResult<Vec<FontInfo>> {
        let mut fonts = Vec::new();

        let locations = vec![
            (
                HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows NT\CurrentVersion\Fonts",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"Software\Microsoft\Windows NT\CurrentVersion\Fonts",
            ),
        ];

        for (hive, key_path) in locations {
            if let Ok(registry_key) =
                RegKey::predef(hive).open_subkey_with_flags(key_path, KEY_READ)
            {
                for (value_name, value_data) in registry_key
                    .enum_values()
                    .filter_map(|(name, data)| data.ok().map(|d| (name, d)))
                {
                    if let Ok(font_path) = registry_key.get_value::<String, _>(&value_name) {
                        let path = PathBuf::from(font_path);
                        if path.exists() && validation::is_valid_font_extension(&path) {
                            if let Ok(mut font_info) = self.get_font_info_from_path(&path) {
                                if let Some(paren_pos) = value_name.find('(') {
                                    font_info.family_name =
                                        value_name[..paren_pos].trim().to_string();
                                }
                                fonts.push(font_info);
                            }
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
    fn install_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        validation::validate_font_file(path)?;
        self.validate_system_operation(scope)?;

        if self.is_system_font_path(path) {
            return Err(FontError::SystemFontProtection(path.to_path_buf()));
        }

        let font_info = self.get_font_info_from_path(path)?;
        let target_path = self.copy_font_to_target_directory(path, scope)?;
        self.register_font_with_gdi(&target_path)?;
        self.register_font_in_registry(&target_path, &font_info, scope)?;

        Ok(())
    }

    fn uninstall_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        validation::validate_font_file(path)?;
        self.validate_system_operation(scope)?;

        if !path.exists() {
            return Err(FontError::FontNotFound(path.to_path_buf()));
        }

        self.unregister_font_from_gdi(path)?;
        self.unregister_font_from_registry(path, scope)?;

        Ok(())
    }

    fn remove_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        self.uninstall_font(path, scope)?;

        if self.is_system_font_path(path) {
            return Err(FontError::SystemFontProtection(path.to_path_buf()));
        }

        std::fs::remove_file(path).map_err(FontError::IoError)?;

        Ok(())
    }

    fn is_font_installed(&self, path: &Path) -> FontResult<bool> {
        Ok(path.exists())
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontInfo>> {
        let mut fonts = Vec::new();

        if let Ok(reg_fonts) = self.enumerate_fonts_from_registry() {
            fonts.extend(reg_fonts);
        }

        let fonts_dir = self.get_fonts_directory()?;
        if let Ok(entries) = std::fs::read_dir(&fonts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && validation::is_valid_font_extension(&path) {
                    if !fonts.iter().any(|f| f.path == path) {
                        if let Ok(font_info) = self.get_font_info_from_path(&path) {
                            fonts.push(font_info);
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
                let user_cache_dir = PathBuf::from(
                    std::env::var("LOCALAPPDATA")
                        .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string()),
                )
                .join("Microsoft\\Windows\\Fonts");

                if user_cache_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&user_cache_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let is_cache = path
                                .extension()
                                .map_or(false, |ext| ext == "cache" || ext == "dat");
                            if path.is_file() && is_cache {
                                std::fs::remove_file(path).map_err(FontError::IoError)?;
                            }
                        }
                    }
                }

                let _ = std::process::Command::new("net")
                    .args(&["stop", "fontcache"])
                    .output();

                let _ = std::process::Command::new("net")
                    .args(&["start", "fontcache"])
                    .output();
            }
            FontScope::System => {
                if !self.has_admin_privileges() {
                    return Err(FontError::PermissionDenied(
                        "System cache clearing requires administrator privileges".to_string(),
                    ));
                }

                let output = std::process::Command::new("net")
                    .args(&["stop", "fontcache"])
                    .output()
                    .map_err(FontError::IoError)?;

                if !output.status.success() {
                    return Err(FontError::RegistrationFailed(
                        "Failed to stop font cache service".to_string(),
                    ));
                }

                let system_cache_dir = PathBuf::from(r"C:\Windows\System32\FntCache");
                if system_cache_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&system_cache_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() {
                                std::fs::remove_file(path).map_err(FontError::IoError)?;
                            }
                        }
                    }
                }

                let output = std::process::Command::new("net")
                    .args(&["start", "fontcache"])
                    .output()
                    .map_err(FontError::IoError)?;

                if !output.status.success() {
                    return Err(FontError::RegistrationFailed(
                        "Failed to start font cache service".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(not(windows))]
impl FontManager for WinFontManager {
    fn install_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        let _ = (path, scope);
        self.unsupported()
    }

    fn uninstall_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        let _ = (path, scope);
        self.unsupported()
    }

    fn remove_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        let _ = (path, scope);
        self.unsupported()
    }

    fn is_font_installed(&self, path: &Path) -> FontResult<bool> {
        let _ = path;
        self.unsupported()
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontInfo>> {
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
    use std::path::PathBuf;

    #[test]
    fn test_win_font_manager_creation() {
        let manager = WinFontManager::new();
        assert_eq!(manager._private, ());
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
        let path = PathBuf::from("dummy.ttf");

        let result = manager.install_font(&path, FontScope::User);
        assert!(matches!(
            result,
            Err(FontError::UnsupportedOperation(msg)) if msg.contains("only available on Windows")
        ));
    }
}
