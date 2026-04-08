//! Shared font management types and contracts for all platform backends.
//!
//! This crate defines the data model and public API that stay the same across
//! macOS and Windows. Platform crates such as `fontlift-platform-mac` and
//! `fontlift-platform-win` implement [`FontManager`] with the real OS calls.
//! `fontlift-core` itself does not call Core Text, GDI, or registry APIs.
//!
//! # Key abstractions
//!
//! - [`FontliftFontSource`] — a pointer to a font file on disk, plus optional
//!   metadata like format and scope.
//! - [`FontliftFontFaceInfo`] — everything we know about one *face* inside a
//!   font file: family, style, weight, PostScript name.
//! - [`FontManager`] — the trait each platform implements: install, uninstall,
//!   list, remove, clear caches.
//! - [`FontError`] — every failure fontlift can produce, with a human-readable
//!   suggestion baked into the `Display` output.
//!
//! # Font terminology
//!
//! A **font file** can contain one or more **faces**.
//! A **font collection** (`.ttc`, `.otc`) bundles several faces into one file.
//!
//! Each face has a **PostScript name** (a stable programmatic identifier such
//! as `"ArialMT"`), a **full name** for menus, a **family name**, and a
//! **style**. Weight uses the common 100 to 900 scale where 400 is Regular and
//! 700 is Bold.

use std::path::PathBuf;
use thiserror::Error;

/// Errors returned by fontlift's core API.
///
/// The `Display` text includes a short suggestion because many callers surface
/// it directly to users.
#[derive(Error, Debug)]
pub enum FontError {
    /// The target path no longer exists.
    #[error("Font file not found: {0}\n→ Check the path. Does the file exist? Was it moved?")]
    FontNotFound(PathBuf),

    /// The file exists but is not a supported font, or failed structural parsing.
    #[error("Invalid font format: {0}\n→ Accepted formats: .ttf, .otf, .ttc, .otc, .woff, .woff2, .dfont")]
    InvalidFormat(String),

    /// The OS refused to register the font.
    #[error("Font registration failed: {0}\n→ Try restarting your system, or run with admin/sudo privileges")]
    RegistrationFailed(String),

    /// You tried to modify a font in an OS-owned location.
    #[error("System font protection: cannot modify {0}\n→ System fonts are off-limits for stability. Use user-level installation instead")]
    SystemFontProtection(PathBuf),

    /// A filesystem operation failed.
    #[error("IO error: {0}\n→ Check file permissions and available disk space")]
    IoError(#[from] std::io::Error),

    /// The operation needs privileges the process does not have.
    #[error("Permission denied: {0}\n→ On macOS: use sudo. On Windows: run as Administrator")]
    PermissionDenied(String),

    /// A font file with the same target name already exists.
    #[error("Font already installed: {0}\n→ Uninstall it first with 'fontlift uninstall', or reinstall with --inplace")]
    AlreadyInstalled(PathBuf),

    /// This feature is not available on the current platform or build.
    #[error("Unsupported operation: {0}\n→ This feature may not be available on your platform or in this version")]
    UnsupportedOperation(String),
}

/// Shorthand for `Result<T, FontError>`.
pub type FontResult<T> = Result<T, FontError>;

/// Where a font is installed, and who can use it.
///
/// - **User** scope installs only for the current account.
///   - macOS: `~/Library/Fonts/`
///   - Windows: per-user font registration in `HKCU`
///
/// - **System** scope installs for all users on the machine.
///   - macOS: `/Library/Fonts/`
///   - Windows: `C:\Windows\Fonts\` + `HKLM` Registry entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FontScope {
    User,
    System,
}

impl FontScope {
    pub fn description(self) -> &'static str {
        match self {
            FontScope::User => "user-level",
            FontScope::System => "system-level",
        }
    }
}

/// Identifies a font file and, when needed, one face inside it.
///
/// `face_index` is used for collection files such as `.ttc` and `.otc`, which
/// can hold several faces in one file. For ordinary single-face files,
/// `face_index` stays `None`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FontliftFontSource {
    pub path: PathBuf,
    pub format: Option<String>,
    pub face_index: Option<u32>,
    pub is_collection: Option<bool>,
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

/// Metadata for one face inside a font file.
///
/// Name fields have different jobs:
/// - `postscript_name` is the stable programmatic identifier.
/// - `full_name` is the menu-facing display name.
/// - `family_name` groups related faces together.
/// - `style` names the specific variant inside that family.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FontliftFontFaceInfo {
    pub source: FontliftFontSource,
    pub postscript_name: String,
    pub full_name: String,
    pub family_name: String,
    pub style: String,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
}

impl FontliftFontFaceInfo {
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

    pub fn filename_stem(&self) -> Option<&str> {
        self.source.path.file_stem()?.to_str()
    }

    pub fn with_scope(mut self, scope: Option<FontScope>) -> Self {
        self.source.scope = scope;
        self
    }
}

/// Platform contract for font management.
///
/// Implementations handle the OS-specific work: register fonts, unregister
/// them, enumerate installed faces, clear caches, and prune stale records.
/// The caller decides whether install happens after copying the file or by
/// registering the original path in place.
pub trait FontManager: Send + Sync {
    /// Register the font at `source.path` so applications can use it.
    ///
    /// The caller may already have copied the file into an OS font directory,
    /// or may be doing an in-place install.
    fn install_font(&self, source: &FontliftFontSource) -> FontResult<()>;

    /// Unregister a font without deleting the file.
    fn uninstall_font(&self, source: &FontliftFontSource) -> FontResult<()>;

    /// Unregister a font and delete the file.
    fn remove_font(&self, source: &FontliftFontSource) -> FontResult<()>;

    /// Check whether the OS currently knows about this font.
    fn is_font_installed(&self, source: &FontliftFontSource) -> FontResult<bool>;

    /// Enumerate every font the OS knows about, across all scopes.
    ///
    /// Returns one [`FontliftFontFaceInfo`] per face, so a collection file may
    /// produce several entries.
    fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>>;

    /// Flush the OS font cache for the given scope.
    ///
    /// Platform implementations may also clear common application caches where
    /// that is practical.
    fn clear_font_caches(&self, scope: FontScope) -> FontResult<()>;

    /// Prune registrations whose backing files no longer exist.
    ///
    /// Returns the number of pruned entries. The default implementation is a
    /// no-op for platforms that do not need this cleanup.
    fn prune_missing_fonts(&self, _scope: FontScope) -> FontResult<usize> {
        Ok(0)
    }
}

/// Quick-and-cheap font file checks that don't require parsing the file contents.
///
/// These functions answer surface-level questions: Does the file exist?
/// Is the extension one we recognize? Can we guess the family name from
/// the filename? They run in microseconds and never open the file for
/// deep inspection — that's what [`validation_ext`] is for.
pub mod validation {
    use super::*;
    use std::path::Path;

    /// Does the file extension look like a font format we support?
    ///
    /// Recognized extensions (case-insensitive):
    /// - `.ttf` — TrueType (single face)
    /// - `.otf` — OpenType with PostScript or TrueType outlines
    /// - `.ttc` / `.otc` — TrueType / OpenType Collection (multiple faces in one file)
    /// - `.woff` / `.woff2` — Web Open Font Format (compressed for the web)
    /// - `.dfont` — macOS data-fork suitcase (legacy macOS format)
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

    /// Verify that `path` exists, is a regular file (not a directory),
    /// and has a recognized font extension. Does *not* parse the file contents.
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

    /// Guess font names from the filename when we can't (or haven't yet)
    /// parsed the file's internal name table.
    ///
    /// Splits on the last hyphen or space:
    /// - `"OpenSans-Bold.ttf"` → family `"OpenSans"`, style `"Bold"`
    /// - `"Noto Sans Light.otf"` → family `"Noto Sans"`, style `"Light"`
    /// - `"Courier.ttf"` → family `"Courier"`, style `"Regular"` (no separator found)
    ///
    /// This is a rough heuristic — real font metadata comes from the OS
    /// (Core Text / GDI) or from parsing the font's `name` table.
    pub fn extract_basic_info_from_path(path: &Path) -> FontliftFontFaceInfo {
        let filename_stem = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        // Split "FamilyName-Style" or "Family Name Style" at the last separator
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

/// Deep font validation in a separate process.
///
/// Why out-of-process? A malformed font file can crash the parser.
/// Running the parser in a child process means a crash kills the child,
/// not fontlift itself. See [`validation_ext::validate_and_introspect`].
pub mod validation_ext;

/// Crash-safe operation journal.
///
/// Font installation is multi-step: copy the file, then register with
/// the OS. If fontlift is killed between those steps, the journal
/// records what happened so `fontlift doctor` can finish or undo the
/// interrupted operation on the next run.
pub mod journal;

/// Font cache management.
///
/// Operating systems and some desktop applications maintain
/// internal caches of font data — glyph outlines, name tables, metrics —
/// to avoid re-reading font files on every launch. When you install or
/// remove a font, these caches can go stale: an app might keep showing
/// a font you deleted, or refuse to see one you just installed.
///
/// Clearing the cache forces apps to re-scan the fonts directory.
/// On macOS, this means deleting files under `~/Library/Caches/`
/// and other app-specific font cache locations.
/// On Windows, it means restarting the Windows Font Cache Service.
pub mod cache {

    /// Which caches to clear.
    #[derive(Debug, Clone)]
    pub enum CacheClearStrategy {
        /// Only the current user's caches. Safe, no admin needed.
        UserOnly,
        /// Only system-wide caches. Requires admin privileges.
        SystemOnly,
        /// Both user and system caches.
        Both,
    }

    /// What happened when we tried to clear caches.
    #[derive(Debug, Clone)]
    pub struct CacheClearResult {
        /// How many cache files or entries were deleted.
        pub entries_cleared: usize,
        /// Some cache changes only take effect after a reboot (looking
        /// at you, Windows Font Cache Service).
        pub restart_required: bool,
        /// Non-fatal issues encountered during cleanup. For example,
        /// "app cache directory not found" means there was nothing to clear for
        /// that cache location.
        pub warnings: Vec<String>,
    }

    impl CacheClearResult {
        /// Create a successful result with the given count and restart flag.
        pub fn success(entries_cleared: usize, restart_required: bool) -> Self {
            Self {
                entries_cleared,
                restart_required,
                warnings: Vec::new(),
            }
        }

        /// Append a warning message. Builder-style.
        pub fn with_warning(mut self, warning: String) -> Self {
            self.warnings.push(warning);
            self
        }
    }
}

/// Guard rails: system font protection and deduplication.
///
/// Some fonts ship with the OS and must not be modified. Deleting
/// `SFNS.ttf` on macOS breaks the entire system UI. Removing `segoeui.ttf`
/// on Windows makes dialog boxes unreadable. This module identifies those
/// protected paths and refuses to touch them.
///
/// It also handles deduplication: when listing fonts, the same face can
/// appear multiple times (e.g. registered under both user and system scope).
/// [`dedupe_fonts`] collapses those duplicates deterministically.
pub mod protection {
    use super::FontliftFontFaceInfo;
    use std::path::Path;

    /// Normalize a path for cross-platform comparison: lowercase,
    /// forward slashes, no doubled separators. This lets us compare
    /// `/Library/Fonts/Helvetica.ttc` and `/library/fonts/helvetica.ttc`
    /// as equal.
    fn normalize(path: &Path) -> String {
        let mut normalized = path.to_string_lossy().replace('\\', "/").to_lowercase();

        // Collapse duplicate separators that can result from Windows-style paths on POSIX hosts
        while normalized.contains("//") {
            normalized = normalized.replace("//", "/");
        }

        normalized
    }

    /// Is this font in a directory the OS owns?
    ///
    /// Protected paths:
    /// - macOS: `/System/Library/Fonts/`, `/Library/Fonts/`
    /// - Windows: `C:\Windows\Fonts\`
    ///
    /// Fonts in `~/Library/Fonts/` (macOS) or user-installed fonts on
    /// Windows are *not* protected — the user put them there and can
    /// remove them.
    pub fn is_protected_system_font_path(path: &Path) -> bool {
        let normalized = normalize(path);

        normalized.starts_with("/system/library/fonts/")
            || normalized.starts_with("/library/fonts/")
            || normalized.starts_with("c:/windows/fonts/")
    }

    /// Remove duplicate font entries and return them in a stable, sorted order.
    ///
    /// Two entries are considered duplicates if they share the same PostScript
    /// name *and* the same file path (both compared case-insensitively).
    /// This happens when the OS reports the same font through multiple
    /// enumeration paths.
    ///
    /// The output is sorted by (PostScript name, path), so results are
    /// deterministic regardless of the order the OS returned them.
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

    // Re-export normalization for the `conflicts` module without making it public API.
    pub(crate) fn normalize_for_tests(path: &Path) -> String {
        normalize(path)
    }
}

/// Font conflict detection.
///
/// Before installing a new font, we check whether it collides with something
/// already on the system. A "conflict" means any of:
///
/// 1. **Same file path** — the exact same file is already installed
///    (case-insensitive comparison, so `/Fonts/Arial.ttf` matches
///    `/fonts/arial.ttf`).
/// 2. **Same PostScript name** — another file is already registered under
///    the same unique identifier. Installing both would confuse applications.
/// 3. **Same family + style** — e.g. two different files both claiming to be
///    "Helvetica Bold". Applications would pick one arbitrarily.
///
/// The install flow uses this to unregister conflicting fonts before
/// registering the new one, avoiding unpredictable behavior.
pub mod conflicts {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn normalize(path: &Path) -> String {
        protection::normalize_for_tests(path)
    }

    /// Find installed fonts that would conflict with `candidate`.
    ///
    /// Returns references to entries in `installed` that share any of:
    /// path, PostScript name, or family+style (all case-insensitive).
    /// Each conflicting font appears at most once, even if it matches
    /// on multiple criteria.
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
                let same_family_style = font.family_name.eq_ignore_ascii_case(&candidate_family)
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

/// A font manager that refuses every operation.
///
/// Used on platforms where fontlift has no real implementation yet (Linux),
/// and in tests that need a `FontManager` instance without touching the OS.
/// Every method returns [`FontError::UnsupportedOperation`].
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
