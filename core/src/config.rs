//! Configuration management for fontlift
//!
//! This module provides configuration handling via environment variables,
//! config files, and runtime settings.

use std::path::{Path, PathBuf};
use std::env;
use anyhow::{Result, Context};

/// Global configuration for fontlift operations
#[derive(Debug, Clone)]
pub struct FontliftConfig {
    /// Font paths and directories
    pub font_paths: FontPaths,
    /// Operation scopes and permissions
    pub permissions: Permissions,
    /// Logging and output settings
    pub logging: Logging,
    /// Cache and performance settings
    pub performance: Performance,
}

/// Font path configuration
#[derive(Debug, Clone)]
pub struct FontPaths {
    /// Override user library path (FONTLIFT_OVERRIDE_USER_LIBRARY)
    pub user_library_override: Option<PathBuf>,
    /// Override system library path (FONTLIFT_OVERRIDE_SYSTEM_LIBRARY)
    pub system_library_override: Option<PathBuf>,
    /// Additional font directories to scan
    pub additional_directories: Vec<PathBuf>,
    /// Temporary directory for operations
    pub temp_directory: PathBuf,
}

/// Permission and security settings
#[derive(Debug, Clone)]
pub struct Permissions {
    /// Allow system-level operations (requires admin/root)
    pub allow_system_operations: bool,
    /// Require confirmation for system font modifications
    pub require_system_confirmation: bool,
    /// Enable dry-run mode for testing
    pub dry_run_mode: bool,
    /// Maximum number of fonts to process in batch operations
    pub max_batch_size: usize,
}

/// Logging and output configuration
#[derive(Debug, Clone)]
pub struct Logging {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Enable verbose output
    pub verbose: bool,
    /// Enable JSON output for machine processing
    pub json_output: bool,
    /// Log file path (None for stdout only)
    pub log_file: Option<PathBuf>,
}

/// Performance and caching settings
#[derive(Debug, Clone)]
pub struct Performance {
    /// Enable font result caching
    pub enable_cache: bool,
    /// Maximum cache size in MB
    pub max_cache_size_mb: usize,
    /// Cache timeout in seconds
    pub cache_timeout_secs: u64,
    /// Enable parallel processing
    pub parallel_processing: bool,
    /// Maximum number of parallel threads
    pub max_threads: Option<usize>,
}

impl Default for FontliftConfig {
    fn default() -> Self {
        Self::from_env().unwrap_or_else(|_| Self::minimal())
    }
}

impl FontliftConfig {
    /// Create configuration from environment variables and config files
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
    
    /// Create minimal default configuration
    pub fn minimal() -> Self {
        Self {
            font_paths: FontPaths::minimal(),
            permissions: Permissions::minimal(),
            logging: Logging::minimal(),
            performance: Performance::minimal(),
        }
    }
    
    /// Load configuration from a TOML file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;
        
        // TODO: Implement TOML parsing when serde is added
        // For now, return minimal config
        Ok(Self::minimal())
    }
    
    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        // TODO: Implement TOML serialization when serde is added
        std::fs::write(path, "# FontLift Configuration\n# TODO: Implement TOML serialization\n")?;
        Ok(())
    }
    
    /// Get effective user library path
    pub fn user_library_path(&self) -> PathBuf {
        self.font_paths.user_library_override
            .clone()
            .unwrap_or_else(|| default_user_library_path())
    }
    
    /// Get effective system library path
    pub fn system_library_path(&self) -> PathBuf {
        self.font_paths.system_library_override
            .clone()
            .unwrap_or_else(|| default_system_library_path())
    }
    
    /// Check if system operations are allowed
    pub fn can_perform_system_operations(&self) -> bool {
        self.permissions.allow_system_operations && is_admin()
    }
    
    /// Validate configuration settings
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
                paths.split(':')
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
            
        let log_file = env::var("FONTLIFT_LOG_FILE")
            .ok()
            .map(PathBuf::from);
        
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

/// Get default user library path based on platform
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
        dirs::font_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\Windows\\Fonts"))
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

/// Get default system library path based on platform
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

/// Get default temporary directory
fn default_temp_directory() -> PathBuf {
    std::env::temp_dir().join("fontlift")
}

/// Check if current process has administrator/root privileges
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
