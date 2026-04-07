//! Extended font validation via out-of-process validator
//!
//! This module provides functions to validate fonts using the external
//! `fontlift-validator` helper process, which parses fonts in isolation
//! for safety.

use crate::{FontError, FontResult, FontliftFontFaceInfo};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Default maximum file size (64 MB)
pub const DEFAULT_MAX_SIZE: u64 = 64 * 1024 * 1024;

/// Default timeout per font (5 seconds)
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Configuration for font validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    /// Maximum file size in bytes (default: 64MB)
    #[serde(default = "default_max_size")]
    pub max_file_size_bytes: u64,

    /// Timeout per font in milliseconds (default: 5000)
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Whether to allow font collections (TTC/OTC)
    #[serde(default = "default_allow_collections")]
    pub allow_collections: bool,
}

fn default_max_size() -> u64 {
    DEFAULT_MAX_SIZE
}
fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}
fn default_allow_collections() -> bool {
    true
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            max_file_size_bytes: DEFAULT_MAX_SIZE,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            allow_collections: true,
        }
    }
}

/// Validation strictness presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStrictness {
    /// Lenient: larger size limits, longer timeouts, allow collections
    Lenient,
    /// Normal: default settings
    Normal,
    /// Paranoid: strict limits, shorter timeouts
    Paranoid,
}

impl ValidatorConfig {
    /// Create config from strictness preset
    pub fn from_strictness(strictness: ValidationStrictness) -> Self {
        match strictness {
            ValidationStrictness::Lenient => Self {
                max_file_size_bytes: 128 * 1024 * 1024, // 128 MB
                timeout_ms: 10000,                      // 10 seconds
                allow_collections: true,
            },
            ValidationStrictness::Normal => Self::default(),
            ValidationStrictness::Paranoid => Self {
                max_file_size_bytes: 32 * 1024 * 1024, // 32 MB
                timeout_ms: 2000,                      // 2 seconds
                allow_collections: true,
            },
        }
    }
}

/// Input to the validator process
#[derive(Debug, Serialize)]
struct ValidatorInput {
    paths: Vec<PathBuf>,
    config: ValidatorConfig,
}

/// Result from validator for a single font
#[derive(Debug, Deserialize)]
struct ValidationResult {
    #[allow(dead_code)] // kept for debugging/future use
    path: PathBuf,
    ok: bool,
    info: Option<FontliftFontFaceInfo>,
    error: Option<String>,
}

/// Validate fonts using the out-of-process validator and extract metadata
///
/// This spawns the `fontlift-validator` helper process, which parses fonts
/// in isolation using `read-fonts`. If validation succeeds, returns the
/// extracted `FontliftFontFaceInfo`; otherwise returns a `FontError`.
///
/// # Arguments
/// * `paths` - Font file paths to validate
/// * `config` - Validation configuration (size limits, timeouts, etc.)
///
/// # Returns
/// A vector of results, one per input path, in the same order
pub fn validate_and_introspect(
    paths: &[PathBuf],
    config: &ValidatorConfig,
) -> FontResult<Vec<Result<FontliftFontFaceInfo, FontError>>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    // Find the validator binary
    let validator_path = find_validator_binary()?;

    // Prepare input
    let input = ValidatorInput {
        paths: paths.to_vec(),
        config: config.clone(),
    };
    let input_json = serde_json::to_string(&input)
        .map_err(|e| FontError::InvalidFormat(format!("Failed to serialize input: {e}")))?;

    // Spawn validator process
    let mut child = Command::new(&validator_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| FontError::UnsupportedOperation(format!("Failed to spawn validator: {e}")))?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_json.as_bytes()).map_err(|e| {
            FontError::IoError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Failed to write to validator stdin: {e}"),
            ))
        })?;
    }

    // Wait for output
    let output = child
        .wait_with_output()
        .map_err(|e| FontError::UnsupportedOperation(format!("Validator process failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FontError::InvalidFormat(format!(
            "Validator failed: {}",
            stderr.trim()
        )));
    }

    // Parse output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<ValidationResult> = serde_json::from_str(&stdout)
        .map_err(|e| FontError::InvalidFormat(format!("Failed to parse validator output: {e}")))?;

    // Convert to FontResult per font
    Ok(results
        .into_iter()
        .map(|r| {
            if r.ok {
                r.info
                    .ok_or_else(|| FontError::InvalidFormat("Missing font info".to_string()))
            } else {
                Err(FontError::InvalidFormat(
                    r.error
                        .unwrap_or_else(|| "Unknown validation error".to_string()),
                ))
            }
        })
        .collect())
}

/// Find the fontlift-validator binary
fn find_validator_binary() -> FontResult<PathBuf> {
    // Try common locations:
    // 1. Same directory as current executable
    // 2. Parent directory (for tests running from deps/)
    // 3. PATH
    // 4. Development build directory

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let validator = dir.join("fontlift-validator");
            if validator.exists() {
                return Ok(validator);
            }
            // Windows
            let validator_exe = dir.join("fontlift-validator.exe");
            if validator_exe.exists() {
                return Ok(validator_exe);
            }

            // Try parent directory (tests run from target/debug/deps/)
            if let Some(parent) = dir.parent() {
                let validator = parent.join("fontlift-validator");
                if validator.exists() {
                    return Ok(validator);
                }
                let validator_exe = parent.join("fontlift-validator.exe");
                if validator_exe.exists() {
                    return Ok(validator_exe);
                }
            }
        }
    }

    // Try PATH
    if let Ok(output) = Command::new("which").arg("fontlift-validator").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            let path = PathBuf::from(path.trim());
            if path.exists() {
                return Ok(path);
            }
        }
    }

    // Windows: try where command
    #[cfg(windows)]
    if let Ok(output) = Command::new("where").arg("fontlift-validator").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = path.lines().next() {
                let path = PathBuf::from(line.trim());
                if path.exists() {
                    return Ok(path);
                }
            }
        }
    }

    Err(FontError::UnsupportedOperation(
        "fontlift-validator binary not found. Install it or ensure it's in PATH.".to_string(),
    ))
}

/// Validate a single font file (convenience wrapper)
pub fn validate_single(path: &Path, config: &ValidatorConfig) -> FontResult<FontliftFontFaceInfo> {
    let results = validate_and_introspect(&[path.to_path_buf()], config)?;
    results
        .into_iter()
        .next()
        .ok_or_else(|| FontError::InvalidFormat("No validation result".to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictness_presets_differ() {
        let lenient = ValidatorConfig::from_strictness(ValidationStrictness::Lenient);
        let normal = ValidatorConfig::from_strictness(ValidationStrictness::Normal);
        let paranoid = ValidatorConfig::from_strictness(ValidationStrictness::Paranoid);

        assert!(lenient.max_file_size_bytes > normal.max_file_size_bytes);
        assert!(normal.max_file_size_bytes > paranoid.max_file_size_bytes);
        assert!(lenient.timeout_ms > normal.timeout_ms);
        assert!(normal.timeout_ms > paranoid.timeout_ms);
    }

    #[test]
    fn default_config_is_normal() {
        let default = ValidatorConfig::default();
        let normal = ValidatorConfig::from_strictness(ValidationStrictness::Normal);

        assert_eq!(default.max_file_size_bytes, normal.max_file_size_bytes);
        assert_eq!(default.timeout_ms, normal.timeout_ms);
        assert_eq!(default.allow_collections, normal.allow_collections);
    }

    #[test]
    fn empty_paths_returns_empty() {
        let result = validate_and_introspect(&[], &ValidatorConfig::default());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
