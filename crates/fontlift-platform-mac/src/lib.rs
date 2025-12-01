//! macOS platform implementation for fontlift
//!
//! This module provides macOS-specific font management using Core Text APIs,
//! implementing the same functionality as the Swift CLI but in Rust.

use fontlift_core::{validation, FontError, FontInfo, FontManager, FontResult, FontScope};
use std::fs;
use std::path::{Path, PathBuf};

use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::error::{CFError, CFErrorRef};
use core_foundation::url::{CFURLRef, CFURL};
use core_text::font_manager as ct_font_manager;

type CTFontManagerScope = u32;
const K_CT_FONT_MANAGER_SCOPE_PERSISTENT: CTFontManagerScope = 2; // aligns with user scope
const K_CT_FONT_MANAGER_SCOPE_USER: CTFontManagerScope = 2;

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontManagerRegisterFontsForURL(
        font_url: CFURLRef,
        scope: CTFontManagerScope,
        error: *mut CFErrorRef,
    ) -> bool;

    fn CTFontManagerUnregisterFontsForURL(
        font_url: CFURLRef,
        scope: CTFontManagerScope,
        error: *mut CFErrorRef,
    ) -> bool;
}

fn ct_scope(scope: FontScope) -> CTFontManagerScope {
    match scope {
        FontScope::User => K_CT_FONT_MANAGER_SCOPE_USER,
        FontScope::System => K_CT_FONT_MANAGER_SCOPE_PERSISTENT,
    }
}

fn cf_error_to_string(err: CFErrorRef) -> String {
    if err.is_null() {
        return "unknown CoreText error".to_string();
    }

    let cf_err = unsafe { CFError::wrap_under_get_rule(err) };
    cf_err.description().to_string()
}

/// macOS font manager using Core Text APIs
pub struct MacFontManager {
    _private: (),
}

impl MacFontManager {
    /// Create a new macOS font manager
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Extract font information using basic filename parsing as fallback
    fn get_font_info_from_path(&self, path: &Path) -> FontResult<FontInfo> {
        validation::validate_font_file(path)?;

        let info = validation::extract_basic_info_from_path(path);
        Ok(info)
    }

    /// Check if path is in system font directory
    fn is_system_font_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.starts_with("/System/Library/Fonts/") || path_str.starts_with("/Library/Fonts/")
    }

    /// Check if current user has admin privileges
    fn has_admin_privileges(&self) -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    /// Copy font to target directory based on scope
    fn copy_font_to_target_directory(
        &self,
        source_path: &Path,
        scope: FontScope,
    ) -> FontResult<()> {
        let target_dir = match scope {
            FontScope::User => {
                let home_dir = std::env::var("HOME").map_err(|_| {
                    FontError::PermissionDenied("Cannot determine home directory".to_string())
                })?;
                PathBuf::from(home_dir).join("Library/Fonts")
            }
            FontScope::System => {
                if !self.has_admin_privileges() {
                    return Err(FontError::PermissionDenied(
                        "System-level font installation requires administrator privileges. Run with --admin or use sudo.".to_string()
                    ));
                }
                PathBuf::from("/Library/Fonts")
            }
        };

        // Create target directory if it doesn't exist
        if !target_dir.exists() {
            fs::create_dir_all(&target_dir).map_err(FontError::IoError)?;
        }

        // Check if font already exists in target location
        let target_path = target_dir.join(source_path.file_name().unwrap());
        if target_path.exists() {
            return Err(FontError::AlreadyInstalled(target_path));
        }

        // Copy font file
        fs::copy(source_path, &target_path).map_err(FontError::IoError)?;

        Ok(())
    }

    /// Validate system operation permissions
    fn validate_system_operation(&self, scope: FontScope) -> FontResult<()> {
        if scope == FontScope::System && !self.has_admin_privileges() {
            return Err(FontError::PermissionDenied(
                "System-level font operations require administrator privileges. Run with --admin or use sudo.".to_string()
            ));
        }
        Ok(())
    }
}

impl Default for MacFontManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FontManager for MacFontManager {
    fn install_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        // Validate inputs
        validation::validate_font_file(path)?;
        self.validate_system_operation(scope)?;

        if self.is_system_font_path(path) {
            return Err(FontError::SystemFontProtection(path.to_path_buf()));
        }

        // Get font info for validation
        let _font_info = self.get_font_info_from_path(path)?;

        // Convert path to CFURL for Core Text
        let cf_url = match CFURL::from_path(path, false) {
            Some(url) => url,
            None => {
                return Err(FontError::InvalidFormat(format!(
                    "Cannot create CFURL from path: {}",
                    path.display()
                )))
            }
        };

        let mut error: CFErrorRef = std::ptr::null_mut();
        let result = unsafe {
            CTFontManagerRegisterFontsForURL(
                cf_url.as_concrete_TypeRef(),
                ct_scope(scope),
                &mut error,
            )
        };

        if result {
            self.copy_font_to_target_directory(path, scope)?;
            Ok(())
        } else {
            let message = cf_error_to_string(error);
            Err(FontError::RegistrationFailed(format!(
                "Core Text failed to register font {}: {}",
                path.display(),
                message
            )))
        }
    }

    fn uninstall_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        validation::validate_font_file(path)?;
        self.validate_system_operation(scope)?;

        if !path.exists() {
            return Err(FontError::FontNotFound(path.to_path_buf()));
        }

        // Convert path to CFURL for Core Text
        let cf_url = match CFURL::from_path(path, false) {
            Some(url) => url,
            None => {
                return Err(FontError::InvalidFormat(format!(
                    "Cannot create CFURL from path: {}",
                    path.display()
                )))
            }
        };

        let mut error: CFErrorRef = std::ptr::null_mut();
        let result = unsafe {
            CTFontManagerUnregisterFontsForURL(
                cf_url.as_concrete_TypeRef(),
                ct_scope(scope),
                &mut error,
            )
        };

        if result {
            Ok(())
        } else {
            let message = cf_error_to_string(error);
            Err(FontError::RegistrationFailed(format!(
                "Core Text failed to unregister font {}: {}",
                path.display(),
                message
            )))
        }
    }

    fn remove_font(&self, path: &Path, scope: FontScope) -> FontResult<()> {
        // First uninstall the font
        self.uninstall_font(path, scope)?;

        // Then delete the file if it's not in a system directory
        if self.is_system_font_path(path) {
            return Err(FontError::SystemFontProtection(path.to_path_buf()));
        }

        std::fs::remove_file(path).map_err(FontError::IoError)?;

        Ok(())
    }

    fn is_font_installed(&self, path: &Path) -> FontResult<bool> {
        // For now, just check if the file exists
        // TODO: Implement actual installation checking
        Ok(path.exists())
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontInfo>> {
        // Get all available font URLs from Core Text
        let font_urls = unsafe { ct_font_manager::CTFontManagerCopyAvailableFontURLs() };

        if font_urls.is_null() {
            return Ok(Vec::new());
        }

        let font_array: CFArray<CFURL> = unsafe { CFArray::wrap_under_get_rule(font_urls) };
        let mut fonts = Vec::new();

        for i in 0..font_array.len() {
            if let Some(cf_url) = font_array.get(i) {
                if let Some(path) = cf_url.to_path() {
                    // Skip if the path doesn't exist or isn't a font file
                    if !path.exists() || !validation::is_valid_font_extension(&path) {
                        continue;
                    }

                    match self.get_font_info_from_path(&path) {
                        Ok(font_info) => fonts.push(font_info),
                        Err(_) => {
                            // Skip fonts we can't read, but don't fail the entire operation
                            continue;
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
                // Clear user font cache using atsutil
                let output = std::process::Command::new("atsutil")
                    .args(["databases", "-removeUser"])
                    .output()
                    .map_err(FontError::IoError)?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(FontError::RegistrationFailed(format!(
                        "Failed to clear user font cache: {}",
                        stderr
                    )));
                }

                // Restart ATS server for user session
                let _ = std::process::Command::new("atsutil")
                    .args(["server", "-shutdown"])
                    .output();

                let _ = std::process::Command::new("atsutil")
                    .args(["server", "-ping"])
                    .output();
            }
            FontScope::System => {
                // System cache clearing requires admin privileges
                if !self.has_admin_privileges() {
                    return Err(FontError::PermissionDenied(
                        "System cache clearing requires administrator privileges".to_string(),
                    ));
                }

                // Clear system font cache using atsutil
                let output = std::process::Command::new("atsutil")
                    .args(["databases", "-remove"])
                    .output()
                    .map_err(FontError::IoError)?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(FontError::RegistrationFailed(format!(
                        "Failed to clear system font cache: {}",
                        stderr
                    )));
                }

                // Restart ATS server for system
                let _ = std::process::Command::new("atsutil")
                    .args(["server", "-shutdown"])
                    .output();

                let _ = std::process::Command::new("atsutil")
                    .args(["server", "-ping"])
                    .output();
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_mac_font_manager_creation() {
        let manager = MacFontManager::new();
        assert_eq!(manager._private, ());
    }

    #[test]
    fn test_system_font_detection() {
        let manager = MacFontManager::new();

        // Test system font paths
        let system_path = PathBuf::from("/System/Library/Fonts/Arial.ttf");
        assert!(manager.is_system_font_path(&system_path));

        let library_path = PathBuf::from("/Library/Fonts/Helvetica.ttc");
        assert!(manager.is_system_font_path(&library_path));

        // Test user font paths
        let user_path = PathBuf::from("/Users/test/Library/Fonts/Custom.otf");
        assert!(!manager.is_system_font_path(&user_path));

        let temp_path = PathBuf::from("/tmp/test.ttf");
        assert!(!manager.is_system_font_path(&temp_path));
    }

    #[test]
    fn test_admin_detection() {
        let manager = MacFontManager::new();
        // This test will typically fail unless run as root
        let is_admin = manager.has_admin_privileges();
        // We can't assert a specific value as it depends on execution context
        println!("Running as admin: {}", is_admin);
    }

    #[test]
    fn test_font_validation() {
        let _manager = MacFontManager::new();

        // Test valid font file paths (these may not exist, just testing validation logic)
        let valid_path = PathBuf::from("/tmp/test.ttf");
        if validation::is_valid_font_extension(&valid_path) {
            // Test would pass if file existed
        }

        let invalid_path = PathBuf::from("/tmp/test.txt");
        assert!(!validation::is_valid_font_extension(&invalid_path));
    }
}
