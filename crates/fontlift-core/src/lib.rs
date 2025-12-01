//! fontlift-core - Core font management library for fontlift
//!
//! This library provides the core abstractions and types for cross-platform
//! font management, including the FontManager trait and common data structures.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Core errors for font management operations
#[derive(Error, Debug)]
pub enum FontError {
    #[error("Font file not found: {0}\n→ Suggestion: Check the file path and ensure the font file exists")]
    FontNotFound(PathBuf),

    #[error("Invalid font format: {0}\n→ Suggestion: Ensure the file is a valid font (.ttf, .otf, .woff, etc.)")]
    InvalidFormat(String),

    #[error("Font registration failed: {0}\n→ Suggestion: Try restarting your system or using administrator/sudo privileges")]
    RegistrationFailed(String),

    #[error("System font protection: cannot modify system font {0}\n→ Suggestion: System fonts are protected for stability. Use user-level installation instead.")]
    SystemFontProtection(PathBuf),

    #[error("IO error: {0}\n→ Suggestion: Check file permissions and disk space")]
    IoError(#[from] std::io::Error),

    #[error("Permission denied: {0}\n→ Suggestion: Run with administrator privileges on Windows or use sudo on macOS")]
    PermissionDenied(String),

    #[error("Font already installed: {0}\n→ Suggestion: Use 'fontlift uninstall' first if you want to reinstall the font")]
    AlreadyInstalled(PathBuf),

    #[error("Unsupported operation: {0}\n→ Suggestion: This feature may not be available on your platform or in this version")]
    UnsupportedOperation(String),
}

/// Result type for font operations
pub type FontResult<T> = Result<T, FontError>;

/// Font installation scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FontScope {
    /// User-level installation (affects only current user)
    User,
    /// System-level installation (affects all users, requires admin)
    System,
}

impl FontScope {
    /// Get a human-readable description
    pub fn description(self) -> &'static str {
        match self {
            FontScope::User => "user-level",
            FontScope::System => "system-level",
        }
    }
}

/// Font information data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FontInfo {
    /// Path to the font file
    pub path: PathBuf,

    /// PostScript name of the font
    pub postscript_name: String,

    /// Full display name
    pub full_name: String,

    /// Font family name
    pub family_name: String,

    /// Font style/subfamily name
    pub style: String,

    /// Font weight (100-900)
    pub weight: Option<u16>,

    /// Whether font is italic
    pub italic: Option<bool>,

    /// Font format (TrueType, OpenType, etc.)
    pub format: Option<String>,
}

impl FontInfo {
    /// Create a new font info with basic information
    pub fn new(
        path: PathBuf,
        postscript_name: String,
        full_name: String,
        family_name: String,
        style: String,
    ) -> Self {
        Self {
            path,
            postscript_name,
            full_name,
            family_name,
            style,
            weight: None,
            italic: None,
            format: None,
        }
    }

    /// Get filename without extension
    pub fn filename_stem(&self) -> Option<&str> {
        self.path.file_stem()?.to_str()
    }
}

/// Font manager trait that must be implemented by each platform
pub trait FontManager: Send + Sync {
    /// Install a font file at the specified scope
    fn install_font(&self, path: &Path, scope: FontScope) -> FontResult<()>;

    /// Uninstall a font (remove from system but keep file)
    fn uninstall_font(&self, path: &Path, scope: FontScope) -> FontResult<()>;

    /// Remove a font (uninstall and delete file)
    fn remove_font(&self, path: &Path, scope: FontScope) -> FontResult<()>;

    /// Check if a font is installed
    fn is_font_installed(&self, path: &Path) -> FontResult<bool>;

    /// List all installed fonts
    fn list_installed_fonts(&self) -> FontResult<Vec<FontInfo>>;

    /// Clear font caches
    fn clear_font_caches(&self, scope: FontScope) -> FontResult<()>;
}

/// Font validation utilities
pub mod validation {
    use super::*;
    use std::path::Path;

    /// Check if file has a valid font extension
    pub fn is_valid_font_extension(path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                matches!(
                    ext_str.to_lowercase().as_str(),
                    "ttf" | "otf" | "ttc" | "otc" | "woff" | "woff2" | "dfont"
                )
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Validate font file exists and is readable
    pub fn validate_font_file(path: &Path) -> FontResult<()> {
        if !path.exists() {
            return Err(FontError::FontNotFound(path.to_path_buf()));
        }

        if !path.is_file() {
            return Err(FontError::InvalidFormat("Path is not a file".to_string()));
        }

        if !is_valid_font_extension(path) {
            return Err(FontError::InvalidFormat(
                "Invalid font extension".to_string(),
            ));
        }

        // Check if file is readable
        std::fs::metadata(path).map_err(FontError::IoError)?;

        Ok(())
    }

    /// Extract basic font information from filename (fallback method)
    pub fn extract_basic_info_from_path(path: &Path) -> FontInfo {
        let filename_stem = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        // Simple heuristics for family/style separation
        let (family, style) = if let Some(hyphen_pos) = filename_stem.rfind('-') {
            let family = filename_stem[..hyphen_pos].trim().to_string();
            let style = filename_stem[hyphen_pos + 1..].trim().to_string();
            (family, style)
        } else if let Some(space_pos) = filename_stem.rfind(' ') {
            let family = filename_stem[..space_pos].trim().to_string();
            let style = filename_stem[space_pos + 1..].trim().to_string();
            (family, style)
        } else {
            (filename_stem.clone(), "Regular".to_string())
        };

        let format = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_uppercase());

        FontInfo::new(
            path.to_path_buf(),
            filename_stem.clone(),
            filename_stem,
            family,
            style,
        )
        .with_format(format)
    }

    trait FontInfoExt {
        fn with_format(self, format: Option<String>) -> Self;
    }

    impl FontInfoExt for FontInfo {
        fn with_format(mut self, format: Option<String>) -> Self {
            self.format = format;
            self
        }
    }
}

/// Font cache management utilities
pub mod cache {

    /// Cache clearing strategy
    #[derive(Debug, Clone)]
    pub enum CacheClearStrategy {
        /// Clear only user caches
        UserOnly,
        /// Clear only system caches (requires admin)
        SystemOnly,
        /// Clear both user and system caches
        Both,
    }

    /// Result of cache clearing operation
    #[derive(Debug, Clone)]
    pub struct CacheClearResult {
        /// Number of cache entries cleared
        pub entries_cleared: usize,
        /// Whether system restart is required
        pub restart_required: bool,
        /// Any warnings that occurred
        pub warnings: Vec<String>,
    }

    impl CacheClearResult {
        pub fn success(entries_cleared: usize, restart_required: bool) -> Self {
            Self {
                entries_cleared,
                restart_required,
                warnings: Vec::new(),
            }
        }

        pub fn with_warning(mut self, warning: String) -> Self {
            self.warnings.push(warning);
            self
        }
    }
}

/// Default font manager implementation that returns "not implemented" errors
#[derive(Debug)]
pub struct DummyFontManager;

impl FontManager for DummyFontManager {
    fn install_font(&self, _path: &Path, _scope: FontScope) -> FontResult<()> {
        Err(FontError::UnsupportedOperation(
            "Font installation not implemented for this platform".to_string(),
        ))
    }

    fn uninstall_font(&self, _path: &Path, _scope: FontScope) -> FontResult<()> {
        Err(FontError::UnsupportedOperation(
            "Font uninstallation not implemented for this platform".to_string(),
        ))
    }

    fn remove_font(&self, _path: &Path, _scope: FontScope) -> FontResult<()> {
        Err(FontError::UnsupportedOperation(
            "Font removal not implemented for this platform".to_string(),
        ))
    }

    fn is_font_installed(&self, _path: &Path) -> FontResult<bool> {
        Err(FontError::UnsupportedOperation(
            "Font installation check not implemented for this platform".to_string(),
        ))
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontInfo>> {
        Err(FontError::UnsupportedOperation(
            "Font listing not implemented for this platform".to_string(),
        ))
    }

    fn clear_font_caches(&self, _scope: FontScope) -> FontResult<()> {
        Err(FontError::UnsupportedOperation(
            "Cache clearing not implemented for this platform".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_font_validation() {
        // Test valid font extensions
        assert!(validation::is_valid_font_extension(&PathBuf::from(
            "test.ttf"
        )));
        assert!(validation::is_valid_font_extension(&PathBuf::from(
            "test.otf"
        )));
        assert!(validation::is_valid_font_extension(&PathBuf::from(
            "test.OTF"
        )));
        assert!(validation::is_valid_font_extension(&PathBuf::from(
            "test.woff2"
        )));

        // Test invalid extensions
        assert!(!validation::is_valid_font_extension(&PathBuf::from(
            "test.txt"
        )));
        assert!(!validation::is_valid_font_extension(&PathBuf::from("test")));
        assert!(!validation::is_valid_font_extension(&PathBuf::from(
            "test.pdf"
        )));
    }

    #[test]
    fn test_basic_info_extraction() {
        let path = PathBuf::from("/fonts/Arial-Bold.ttf");
        let info = validation::extract_basic_info_from_path(&path);

        assert_eq!(info.path, path);
        assert_eq!(info.postscript_name, "Arial-Bold");
        assert_eq!(info.full_name, "Arial-Bold");
        assert_eq!(info.family_name, "Arial");
        assert_eq!(info.style, "Bold");
        assert_eq!(info.format, Some("TTF".to_string()));
    }

    #[test]
    fn test_scope_description() {
        assert_eq!(FontScope::User.description(), "user-level");
        assert_eq!(FontScope::System.description(), "system-level");
    }

    #[test]
    fn test_cache_clear_result() {
        let result = cache::CacheClearResult::success(5, true)
            .with_warning("Some fonts may require restart".to_string());

        assert_eq!(result.entries_cleared, 5);
        assert!(result.restart_required);
        assert_eq!(result.warnings.len(), 1);
    }
}
