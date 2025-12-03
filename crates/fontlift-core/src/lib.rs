//! fontlift-core - Core font management library for fontlift
//!
//! This library provides the core abstractions and types for cross-platform
//! font management, including the FontManager trait and common data structures.

use std::path::PathBuf;
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

/// Reference to a font container (file, TTC index, optional scope hint)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FontliftFontSource {
    /// Path to the font file
    pub path: PathBuf,
    /// Font format (TrueType, OpenType, etc.)
    pub format: Option<String>,
    /// Optional face index for collections
    pub face_index: Option<u32>,
    /// Whether the source is a collection
    pub is_collection: Option<bool>,
    /// Optional installation scope hint
    pub scope: Option<FontScope>,
}

impl FontliftFontSource {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            format: None,
            face_index: None,
            is_collection: None,
            scope: None,
        }
    }

    pub fn with_format(mut self, format: Option<String>) -> Self {
        self.format = format;
        self
    }

    pub fn with_face_index(mut self, index: Option<u32>) -> Self {
        self.face_index = index;
        self
    }

    pub fn with_collection_flag(mut self, is_collection: Option<bool>) -> Self {
        self.is_collection = is_collection;
        self
    }

    pub fn with_scope(mut self, scope: Option<FontScope>) -> Self {
        self.scope = scope;
        self
    }

    pub fn scope_or(self, default: FontScope) -> FontScope {
        self.scope.unwrap_or(default)
    }
}

/// Font face metadata paired with its source
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FontliftFontFaceInfo {
    /// Source (path + format + optional scope/index)
    pub source: FontliftFontSource,

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
}

impl FontliftFontFaceInfo {
    /// Create a new font face info with basic information
    pub fn new(
        source: FontliftFontSource,
        postscript_name: String,
        full_name: String,
        family_name: String,
        style: String,
    ) -> Self {
        Self {
            source,
            postscript_name,
            full_name,
            family_name,
            style,
            weight: None,
            italic: None,
        }
    }

    /// Get filename without extension
    pub fn filename_stem(&self) -> Option<&str> {
        self.source.path.file_stem()?.to_str()
    }

    /// Attach an installation scope hint to the underlying source
    pub fn with_scope(mut self, scope: Option<FontScope>) -> Self {
        self.source.scope = scope;
        self
    }
}

/// Font manager trait that must be implemented by each platform
pub trait FontManager: Send + Sync {
    /// Install a font file at the specified scope
    fn install_font(&self, source: &FontliftFontSource) -> FontResult<()>;

    /// Uninstall a font (remove from system but keep file)
    fn uninstall_font(&self, source: &FontliftFontSource) -> FontResult<()>;

    /// Remove a font (uninstall and delete file)
    fn remove_font(&self, source: &FontliftFontSource) -> FontResult<()>;

    /// Check if a font is installed
    fn is_font_installed(&self, source: &FontliftFontSource) -> FontResult<bool>;

    /// List all installed fonts
    fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>>;

    /// Clear font caches
    fn clear_font_caches(&self, scope: FontScope) -> FontResult<()>;

    /// Prune registrations that point to missing or invalid font files.
    /// Default implementation performs no pruning.
    fn prune_missing_fonts(&self, _scope: FontScope) -> FontResult<usize> {
        Ok(0)
    }
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
    pub fn extract_basic_info_from_path(path: &Path) -> FontliftFontFaceInfo {
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

        let source = FontliftFontSource::new(path.to_path_buf()).with_format(format);

        FontliftFontFaceInfo::new(source, filename_stem.clone(), filename_stem, family, style)
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

/// Helpers for system font protection and duplicate handling
pub mod protection {
    use super::FontliftFontFaceInfo;
    use std::path::Path;

    /// Normalize a path for comparison across platforms (lowercase, forward slashes)
    fn normalize(path: &Path) -> String {
        let mut normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();

        // Collapse duplicate separators that can result from Windows-style paths on POSIX hosts
        while normalized.contains("//") {
            normalized = normalized.replace("//", "/");
        }

        normalized
    }

    /// Detect whether the path points to a protected system font location.
    /// Covers common macOS and Windows system font directories.
    pub fn is_protected_system_font_path(path: &Path) -> bool {
        let normalized = normalize(path);

        normalized.starts_with("/system/library/fonts/")
            || normalized.starts_with("/library/fonts/")
            || normalized.starts_with("c:/windows/fonts/")
    }

    /// Deduplicate fonts deterministically by PostScript name (case-insensitive)
    /// and path (case-insensitive), returning a sorted list.
    pub fn dedupe_fonts(mut fonts: Vec<FontliftFontFaceInfo>) -> Vec<FontliftFontFaceInfo> {
        fonts.sort_by(|a, b| {
            let name_a = a.postscript_name.to_lowercase();
            let name_b = b.postscript_name.to_lowercase();
            let path_a = normalize(&a.source.path);
            let path_b = normalize(&b.source.path);
            (name_a, path_a).cmp(&(name_b, path_b))
        });

        fonts.dedup_by(|a, b| {
            a.postscript_name.eq_ignore_ascii_case(&b.postscript_name)
                && normalize(&a.source.path) == normalize(&b.source.path)
        });

        fonts
    }

    // Re-export for shared conflict detection helpers without exposing normalization publicly
    pub(crate) fn normalize_for_tests(path: &Path) -> String {
        normalize(path)
    }
}

/// Conflict detection helpers shared across platforms.
pub mod conflicts {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn normalize(path: &Path) -> String {
        protection::normalize_for_tests(path)
    }

    /// Detect conflicting fonts by path, PostScript name, or family+style (case-insensitive).
    /// Returns unique references to installed fonts that should be removed or updated
    /// before installing `candidate`.
    pub fn detect_conflicts<'a>(
        installed: &'a [FontliftFontFaceInfo],
        candidate: &FontliftFontFaceInfo,
    ) -> Vec<&'a FontliftFontFaceInfo> {
        let candidate_path = normalize(&candidate.source.path);
        let candidate_post = candidate.postscript_name.to_lowercase();
        let candidate_family = candidate.family_name.to_lowercase();
        let candidate_style = candidate.style.to_lowercase();

        let mut seen_paths = BTreeSet::new();

        installed
            .iter()
            .filter(|font| {
                let path = normalize(&font.source.path);
                let same_path = path == candidate_path;
                let same_post = font.postscript_name.eq_ignore_ascii_case(&candidate_post);
                let same_family_style = font
                    .family_name
                    .eq_ignore_ascii_case(&candidate_family)
                    && font.style.eq_ignore_ascii_case(&candidate_style);

                same_path || same_post || same_family_style
            })
            .filter(|font| {
                // guarantee unique paths in output for predictable handling
                seen_paths.insert(normalize(&font.source.path))
            })
            .collect()
    }
}

/// Default font manager implementation that returns "not implemented" errors
#[derive(Debug)]
pub struct DummyFontManager;

impl FontManager for DummyFontManager {
    fn install_font(&self, _source: &FontliftFontSource) -> FontResult<()> {
        Err(FontError::UnsupportedOperation(
            "Font installation not implemented for this platform".to_string(),
        ))
    }

    fn uninstall_font(&self, _source: &FontliftFontSource) -> FontResult<()> {
        Err(FontError::UnsupportedOperation(
            "Font uninstallation not implemented for this platform".to_string(),
        ))
    }

    fn remove_font(&self, _source: &FontliftFontSource) -> FontResult<()> {
        Err(FontError::UnsupportedOperation(
            "Font removal not implemented for this platform".to_string(),
        ))
    }

    fn is_font_installed(&self, _source: &FontliftFontSource) -> FontResult<bool> {
        Err(FontError::UnsupportedOperation(
            "Font installation check not implemented for this platform".to_string(),
        ))
    }

    fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>> {
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
    fn detects_protected_system_font_paths() {
        let mac_system = PathBuf::from("/System/Library/Fonts/SFNS.ttf");
        let mac_library = PathBuf::from("/Library/Fonts/Helvetica.ttc");
        let mac_user = PathBuf::from("/Users/example/Library/Fonts/Custom.otf");

        assert!(protection::is_protected_system_font_path(&mac_system));
        assert!(protection::is_protected_system_font_path(&mac_library));
        assert!(!protection::is_protected_system_font_path(&mac_user));

        let win_system = PathBuf::from(r"C:\\Windows\\Fonts\\Arial.ttf");
        let win_subdir = PathBuf::from(r"C:\\Windows\\Fonts\\TrueType\\ComicSans.ttf");
        let win_user = PathBuf::from(r"D:\\Users\\me\\Fonts\\MyFont.ttf");

        assert!(protection::is_protected_system_font_path(&win_system));
        assert!(protection::is_protected_system_font_path(&win_subdir));
        assert!(!protection::is_protected_system_font_path(&win_user));
    }

    #[test]
    fn deduplication_is_deterministic_by_name_and_path() {
        let fonts = vec![
            FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/fonts/Beta.ttf")),
                "Beta".into(),
                "Beta".into(),
                "BetaFamily".into(),
                "Regular".into(),
            ),
            FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/fonts/alpha.ttf")),
                "Alpha".into(),
                "Alpha".into(),
                "AlphaFamily".into(),
                "Regular".into(),
            ),
            // duplicate same name/path differing only in case
            FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/fonts/alpha.ttf")),
                "alpha".into(),
                "alpha".into(),
                "AlphaFamily".into(),
                "Regular".into(),
            ),
            // same name different path should keep both but order deterministic
            FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/fonts/alpha-bold.ttf")),
                "Alpha".into(),
                "Alpha".into(),
                "AlphaFamily".into(),
                "Bold".into(),
            ),
        ];

        let deduped = protection::dedupe_fonts(fonts);

        let names_and_paths: Vec<(String, String)> = deduped
            .into_iter()
            .map(|f| (f.postscript_name, f.source.path.display().to_string()))
            .collect();

        assert_eq!(
            names_and_paths,
            vec![
                ("Alpha".into(), "/fonts/alpha-bold.ttf".into()),
                ("Alpha".into(), "/fonts/alpha.ttf".into()),
                ("Beta".into(), "/fonts/Beta.ttf".into()),
            ],
            "duplicates removed and order is deterministic by name then path"
        );
    }

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

        assert_eq!(info.source.path, path);
        assert_eq!(info.postscript_name, "Arial-Bold");
        assert_eq!(info.full_name, "Arial-Bold");
        assert_eq!(info.family_name, "Arial");
        assert_eq!(info.style, "Bold");
        assert_eq!(info.source.format, Some("TTF".to_string()));
    }

    #[test]
    fn detects_conflicts_by_path_postscript_and_family_style() {
        let installed = vec![
            FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/fonts/alpha-regular.ttf")),
                "AlphaPS".into(),
                "Alpha Regular".into(),
                "Alpha".into(),
                "Regular".into(),
            ),
            FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/fonts/alpha-bold.ttf")),
                "AlphaBoldPS".into(),
                "Alpha Bold".into(),
                "Alpha".into(),
                "Bold".into(),
            ),
            // different path but same family/style should count as conflict
            FontliftFontFaceInfo::new(
                FontliftFontSource::new(PathBuf::from("/other/alpha-regular.ttf")),
                "DifferentPS".into(),
                "Alpha Regular".into(),
                "Alpha".into(),
                "Regular".into(),
            ),
        ];

        let candidate = FontliftFontFaceInfo::new(
            FontliftFontSource::new(PathBuf::from("/Fonts/ALPHA-Regular.ttf")),
            "AlphaPS".into(),
            "Alpha Regular".into(),
            "Alpha".into(),
            "Regular".into(),
        );

        let conflicts = conflicts::detect_conflicts(&installed, &candidate);

        let paths: Vec<String> = conflicts
            .into_iter()
            .map(|f| f.source.path.to_string_lossy().to_lowercase())
            .collect();

        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"/fonts/alpha-regular.ttf".to_string()));
        assert!(paths.contains(&"/other/alpha-regular.ttf".to_string()));
        assert!(paths.iter().all(|p| p.contains("alpha")));
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
