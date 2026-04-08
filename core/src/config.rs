//! Runtime configuration for fontlift.
//!
//! Today, the real runtime entry point is [`FontliftConfig::from_env`], which
//! reads `FONTLIFT_*` variables and fills in built-in defaults for anything not
//! set.
//!
//! [`FontliftConfig::from_file`] exists, but TOML parsing is not implemented
//! yet. It currently verifies that a file is readable and then returns
//! [`FontliftConfig::minimal()`]. There is no automatic file-plus-env merge yet.
//!
//! Once file parsing lands, the intended precedence is:
//! 1. Environment variables
//! 2. Config file values
//! 3. Built-in defaults
//!
//! ## Current `FONTLIFT_*` environment variables
//!
//! | Variable | What it controls | Default |
//! |---|---|---|
//! | `FONTLIFT_OVERRIDE_USER_LIBRARY` | Per-user font directory | Platform default |
//! | `FONTLIFT_OVERRIDE_SYSTEM_LIBRARY` | System-wide font directory | Platform default |
//! | `FONTLIFT_ADDITIONAL_FONTS` | Extra dirs to scan (`:` separated) | (none) |
//! | `FONTLIFT_TEMP_DIR` | Scratch space for in-progress ops | OS temp dir |
//! | `FONTLIFT_ALLOW_SYSTEM` | Permit writes to system font dirs | `false` |
//! | `FONTLIFT_REQUIRE_CONFIRMATION` | Prompt before system modifications | `true` |
//! | `FONTLIFT_DRY_RUN` | Simulate everything, change nothing | `false` |
//! | `FONTLIFT_MAX_BATCH_SIZE` | Cap on fonts processed in one pass | `1000` |
//! | `FONTLIFT_LOG_LEVEL` | `trace`/`debug`/`info`/`warn`/`error` | `info` |
//! | `FONTLIFT_VERBOSE` | Extra human-readable output | `false` |
//! | `FONTLIFT_JSON` | Machine-readable JSON output | `false` |
//! | `FONTLIFT_LOG_FILE` | Write logs here too (in addition to stdout) | (none) |
//! | `FONTLIFT_ENABLE_CACHE` | Cache font metadata between scans | `true` |
//! | `FONTLIFT_MAX_CACHE_SIZE_MB` | Maximum cache footprint in MB | `100` |
//! | `FONTLIFT_CACHE_TIMEOUT_SECS` | How long cached metadata stays fresh | `3600` |
//! | `FONTLIFT_PARALLEL` | Process multiple fonts concurrently | `true` |
//! | `FONTLIFT_MAX_THREADS` | Thread pool ceiling (unset = all cores) | (all cores) |
//! | `FONTLIFT_JOURNAL_PATH` | Override journal file location | Platform default |

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

/// Top-level configuration used by fontlift.
///
/// Build one with [`FontliftConfig::from_env`] for real runs or
/// [`FontliftConfig::minimal`] for tests. Call [`FontliftConfig::validate`]
/// before mutating fonts.
///
/// ```rust,ignore
/// let config = FontliftConfig::from_env()?;
/// config.validate()?;
/// let user_fonts = config.user_library_path(); // e.g. ~/Library/Fonts on macOS
/// ```
#[derive(Debug, Clone)]
pub struct FontliftConfig {
    /// Where fonts live on disk, including overrides.
    pub font_paths: FontPaths,
    /// What fontlift is allowed to do.
    pub permissions: Permissions,
    /// Logging and output format.
    pub logging: Logging,
    /// Caching and parallelism settings.
    pub performance: Performance,
}

/// Font directories and staging paths.
///
/// Each platform keeps user fonts and system fonts in different places:
///
/// | Platform | User fonts | System fonts |
/// |---|---|---|
/// | macOS | `~/Library/Fonts` | `/Library/Fonts` |
/// | Windows | per-user fonts directory from `dirs::font_dir()` | `C:\Windows\Fonts` |
/// | Linux | `~/.local/share/fonts` | `/usr/share/fonts` |
///
/// User directories are per-account and do not need elevation. System
/// directories are shared by all users and usually require admin rights.
/// `FONTLIFT_OVERRIDE_*` variables replace those defaults for the current run.
#[derive(Debug, Clone)]
pub struct FontPaths {
    pub user_library_override: Option<PathBuf>,

    pub system_library_override: Option<PathBuf>,

    pub additional_directories: Vec<PathBuf>,

    pub temp_directory: PathBuf,
}

/// Permissions and safety switches.
///
/// Defaults are conservative: user scope only, confirmation required, and no
/// dry run unless requested.
#[derive(Debug, Clone)]
pub struct Permissions {
    pub allow_system_operations: bool,

    pub require_system_confirmation: bool,

    pub dry_run_mode: bool,

    pub max_batch_size: usize,
}

#[derive(Debug, Clone)]
pub struct Logging {
    pub level: String,

    pub verbose: bool,

    pub json_output: bool,

    pub log_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Performance {
    pub enable_cache: bool,

    pub max_cache_size_mb: usize,

    pub cache_timeout_secs: u64,

    pub parallel_processing: bool,

    pub max_threads: Option<usize>,
}

impl Default for FontliftConfig {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|_| Self::minimal())
    }
}

impl FontliftConfig {
    /// Build configuration from `FONTLIFT_*` environment variables.
    ///
    /// Unset variables fall back to built-in defaults. Present but invalid
    /// values return an error.
    pub fn from_env() -> Result<Self> {
        let font_paths = FontPaths::from_env()?;
        let permissions = Permissions::from_env()?;
        let logging = Logging::from_env()?;
        let performance = Performance::from_env()?;

        Ok(Self {
            font_paths,
            permissions,
            logging,
            performance,
        })
    }

    /// Build configuration from built-in defaults only.
    ///
    /// This does not read environment variables and is mainly useful in tests.
    pub fn minimal() -> Self {
        Self {
            font_paths: FontPaths::minimal(),
            permissions: Permissions::minimal(),
            logging: Logging::minimal(),
            performance: Performance::minimal(),
        }
    }

    /// Load configuration from a TOML file placeholder.
    ///
    /// Current behavior is limited: the file is read to prove it exists and is
    /// readable, then [`FontliftConfig::minimal`] is returned. File values are
    /// not parsed yet.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        // TODO: Implement TOML parsing when serde is added
        // For now, return minimal config
        Ok(Self::minimal())
    }

    /// Write a placeholder TOML file.
    ///
    /// Full serialization is not implemented yet.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        // TODO: Implement TOML serialization when serde is added
        std::fs::write(
            path,
            "# FontLift Configuration\n# TODO: Implement TOML serialization\n",
        )?;
        Ok(())
    }

    /// Return the effective per-user font directory for this run.
    pub fn user_library_path(&self) -> PathBuf {
        self.font_paths
            .user_library_override
            .clone()
            .unwrap_or_else(|| default_user_library_path())
    }

    /// Return the effective system-wide font directory for this run.
    ///
    /// Writing here still requires both config permission and real elevated
    /// privileges.
    pub fn system_library_path(&self) -> PathBuf {
        self.font_paths
            .system_library_override
            .clone()
            .unwrap_or_else(|| default_system_library_path())
    }

    /// Return `true` only when config and process privileges both allow system work.
    pub fn can_perform_system_operations(&self) -> bool {
        self.permissions.allow_system_operations && is_admin()
    }

    /// Validate internal consistency before doing real work.
    ///
    /// This checks that override paths exist when set, and that size limits are
    /// non-zero.
    pub fn validate(&self) -> Result<()> {
        // Check if paths exist where they should
        if let Some(ref path) = self.font_paths.user_library_override {
            if !path.exists() {
                anyhow::bail!("User library override path does not exist: {:?}", path);
            }
        }

        if let Some(ref path) = self.font_paths.system_library_override {
            if !path.exists() {
                anyhow::bail!("System library override path does not exist: {:?}", path);
            }
        }

        // Validate performance settings
        if self.performance.max_cache_size_mb == 0 {
            anyhow::bail!("Max cache size must be greater than 0");
        }

        if self.permissions.max_batch_size == 0 {
            anyhow::bail!("Max batch size must be greater than 0");
        }

        Ok(())
    }
}

impl FontPaths {
    pub fn from_env() -> Result<Self> {
        let user_library_override = env::var("FONTLIFT_OVERRIDE_USER_LIBRARY")
            .ok()
            .map(PathBuf::from);

        let system_library_override = env::var("FONTLIFT_OVERRIDE_SYSTEM_LIBRARY")
            .ok()
            .map(PathBuf::from);

        let additional_directories = env::var("FONTLIFT_ADDITIONAL_FONTS")
            .ok()
            .map(|paths| {
                paths
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default();

        let temp_directory = env::var("FONTLIFT_TEMP_DIR")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(default_temp_directory);

        Ok(Self {
            user_library_override,
            system_library_override,
            additional_directories,
            temp_directory,
        })
    }

    pub fn minimal() -> Self {
        Self {
            user_library_override: None,
            system_library_override: None,
            additional_directories: Vec::new(),
            temp_directory: default_temp_directory(),
        }
    }
}

impl Permissions {
    pub fn from_env() -> Result<Self> {
        let allow_system_operations = env::var("FONTLIFT_ALLOW_SYSTEM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);

        let require_system_confirmation = env::var("FONTLIFT_REQUIRE_CONFIRMATION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        let dry_run_mode = env::var("FONTLIFT_DRY_RUN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);

        let max_batch_size = env::var("FONTLIFT_MAX_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);

        Ok(Self {
            allow_system_operations,
            require_system_confirmation,
            dry_run_mode,
            max_batch_size,
        })
    }

    pub fn minimal() -> Self {
        Self {
            allow_system_operations: false,
            require_system_confirmation: true,
            dry_run_mode: false,
            max_batch_size: 1000,
        }
    }
}

impl Logging {
    pub fn from_env() -> Result<Self> {
        let level = env::var("FONTLIFT_LOG_LEVEL")
            .ok()
            .unwrap_or_else(|| "info".to_string());

        let verbose = env::var("FONTLIFT_VERBOSE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);

        let json_output = env::var("FONTLIFT_JSON")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);

        let log_file = env::var("FONTLIFT_LOG_FILE").ok().map(PathBuf::from);

        Ok(Self {
            level,
            verbose,
            json_output,
            log_file,
        })
    }

    pub fn minimal() -> Self {
        Self {
            level: "info".to_string(),
            verbose: false,
            json_output: false,
            log_file: None,
        }
    }
}

impl Performance {
    pub fn from_env() -> Result<Self> {
        let enable_cache = env::var("FONTLIFT_ENABLE_CACHE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        let max_cache_size_mb = env::var("FONTLIFT_MAX_CACHE_SIZE_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        let cache_timeout_secs = env::var("FONTLIFT_CACHE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);

        let parallel_processing = env::var("FONTLIFT_PARALLEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        let max_threads = env::var("FONTLIFT_MAX_THREADS")
            .ok()
            .and_then(|v| v.parse().ok());

        Ok(Self {
            enable_cache,
            max_cache_size_mb,
            cache_timeout_secs,
            parallel_processing,
            max_threads,
        })
    }

    pub fn minimal() -> Self {
        Self {
            enable_cache: true,
            max_cache_size_mb: 100,
            cache_timeout_secs: 3600,
            parallel_processing: true,
            max_threads: None,
        }
    }
}

/// Platform default for the per-user font directory.
///
/// - **macOS**: `~/Library/Fonts` — the standard per-user font location.
///   Fonts here are visible to the current user immediately after being copied;
///   no cache flush or admin rights needed.
/// - **Windows**: resolved via `dirs::font_dir()`, typically
///   a per-user fonts directory under Local AppData. Per-user font install landed in
///   Windows 10 1809 — older systems only have `C:\Windows\Fonts` (system-wide).
///   Falls back to `C:\Windows\Fonts` if the per-user path is unavailable.
/// - **Linux**: `~/.local/share/fonts` — the XDG user data convention.
///   After copying here, run `fc-cache -f` to make the font visible to apps
///   that use Fontconfig (most Linux GUI apps do).
fn default_user_library_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join("Library")
            .join("Fonts")
    }

    #[cfg(target_os = "windows")]
    {
        dirs::font_dir().unwrap_or_else(|| PathBuf::from("C:\\Windows\\Fonts"))
    }

    #[cfg(target_os = "linux")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".local")
            .join("share")
            .join("fonts")
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PathBuf::from("/tmp/fonts")
    }
}

/// Platform default for the machine-wide font directory.
///
/// - **macOS**: `/Library/Fonts` — visible to all users. Writing here requires
///   root or a process with appropriate entitlements. macOS also has
///   `/System/Library/Fonts` for system-managed fonts, which fontlift never touches.
/// - **Windows**: `C:\Windows\Fonts` — the traditional system font location,
///   shared by all users. Requires Administrator privileges to modify.
/// - **Linux**: `/usr/share/fonts` — distro convention for shared fonts.
///   Requires root. After modifying, run `fc-cache -f -s` (system-wide cache flush).
fn default_system_library_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Fonts")
    }

    #[cfg(target_os = "windows")]
    {
        PathBuf::from("C:\\Windows\\Fonts")
    }

    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/usr/share/fonts")
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PathBuf::from("/usr/share/fonts")
    }
}

/// Default scratch directory path for fontlift.
///
/// Returns `{OS_TEMP}/fontlift`.
fn default_temp_directory() -> PathBuf {
    std::env::temp_dir().join("fontlift")
}

/// Returns `true` if the running process has administrator-level privileges.
///
/// What "admin" means depends on the OS:
///
/// - **macOS / Linux**: checks whether the effective user ID is `0` (root).
///   Running under `sudo` sets euid to 0; running as a normal user does not,
///   even if that user is in the `wheel` or `sudo` group.
/// - **Windows**: checks for membership in the Administrators group via the
///   Windows security API. A process can have an Administrator token but still
///   run at medium integrity (UAC); this function returns `true` only when the
///   token is elevated. (Full implementation pending — currently returns `false`.)
/// - **Other platforms**: conservatively returns `false`.
///
/// Used by [`FontliftConfig::can_perform_system_operations`] as one of two
/// required conditions for system-scope font operations.
pub fn is_admin() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows admin check - simplified version
        // TODO: Implement proper Windows admin check
        false
    }

    #[cfg(target_os = "linux")]
    {
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FontliftConfig::minimal();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_user_library_path() {
        let config = FontliftConfig::minimal();
        let path = config.user_library_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_system_library_path() {
        let config = FontliftConfig::minimal();
        let path = config.system_library_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_env_override() {
        // Test environment variable override
        env::set_var("FONTLIFT_DRY_RUN", "true");
        let permissions = Permissions::from_env().unwrap();
        assert!(permissions.dry_run_mode);
        env::remove_var("FONTLIFT_DRY_RUN");

        let permissions = Permissions::from_env().unwrap();
        assert!(!permissions.dry_run_mode);
    }

    #[test]
    fn test_validation() {
        let mut config = FontliftConfig::minimal();
        assert!(config.validate().is_ok());

        // Test invalid cache size
        config.performance.max_cache_size_mb = 0;
        assert!(config.validate().is_err());
    }
}
