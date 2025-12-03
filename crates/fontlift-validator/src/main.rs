//! fontlift-validator - Out-of-process font validation helper
//!
//! Parses font files using `read-fonts` to validate structure and extract
//! metadata before the OS font stack sees them. Designed to run as a short-lived
//! helper process with resource limits.

use fontlift_core::{FontliftFontFaceInfo, FontliftFontSource};
use read_fonts::{FileRef, FontRef, TableProvider};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Default maximum file size (64 MB)
const DEFAULT_MAX_SIZE: u64 = 64 * 1024 * 1024;

/// Default timeout per font (5 seconds)
const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Configuration for validation
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

/// Input to the validator (paths + config)
#[derive(Debug, Deserialize)]
pub struct ValidatorInput {
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub config: ValidatorConfig,
}

/// Result for a single font validation
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub path: PathBuf,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<FontliftFontFaceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ValidationResult {
    fn success(path: PathBuf, info: FontliftFontFaceInfo) -> Self {
        Self {
            path,
            ok: true,
            info: Some(info),
            error: None,
        }
    }

    fn failure(path: PathBuf, error: &str) -> Self {
        Self {
            path,
            ok: false,
            info: None,
            error: Some(sanitize_error(error)),
        }
    }
}

/// Sanitize error messages to avoid leaking internal paths or stack traces
fn sanitize_error(error: &str) -> String {
    // Remove absolute paths, keep just the error essence
    let error = error.replace('\\', "/");
    // Truncate excessively long errors
    if error.len() > 200 {
        format!("{}...", &error[..200])
    } else {
        error.to_string()
    }
}

/// Validate a single font file and extract metadata
fn validate_font(path: &PathBuf, config: &ValidatorConfig) -> ValidationResult {
    let start = Instant::now();
    let timeout = Duration::from_millis(config.timeout_ms);

    // Check file exists
    if !path.exists() {
        return ValidationResult::failure(path.clone(), "File not found");
    }

    if !path.is_file() {
        return ValidationResult::failure(path.clone(), "Path is not a file");
    }

    // Check extension
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !matches!(
        ext.as_str(),
        "ttf" | "otf" | "ttc" | "otc" | "woff" | "woff2" | "dfont"
    ) {
        return ValidationResult::failure(path.clone(), "Invalid font extension");
    }

    // Check file size
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return ValidationResult::failure(path.clone(), "Cannot read file metadata"),
    };

    if metadata.len() > config.max_file_size_bytes {
        return ValidationResult::failure(
            path.clone(),
            &format!(
                "File exceeds maximum size ({} bytes > {} bytes)",
                metadata.len(),
                config.max_file_size_bytes
            ),
        );
    }

    // Read file data
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return ValidationResult::failure(path.clone(), "Cannot read file"),
    };

    // Check timeout
    if start.elapsed() > timeout {
        return ValidationResult::failure(path.clone(), "Validation timeout");
    }

    // Parse with read-fonts
    let file_ref = match FileRef::new(&data) {
        Ok(f) => f,
        Err(e) => {
            return ValidationResult::failure(path.clone(), &format!("Invalid font structure: {e}"))
        }
    };

    let is_collection = matches!(file_ref, FileRef::Collection(_));

    if is_collection && !config.allow_collections {
        return ValidationResult::failure(path.clone(), "Font collections not allowed");
    }

    // Get first font (or single font)
    let font = match file_ref {
        FileRef::Font(f) => f,
        FileRef::Collection(c) => match c.get(0) {
            Ok(f) => f,
            Err(e) => {
                return ValidationResult::failure(
                    path.clone(),
                    &format!("Cannot read collection: {e}"),
                )
            }
        },
    };

    // Check timeout
    if start.elapsed() > timeout {
        return ValidationResult::failure(path.clone(), "Validation timeout");
    }

    // Extract metadata from name table
    let (postscript_name, full_name, family_name, style_name) = extract_names(&font);

    // Extract weight and italic from OS/2 table
    let (weight, italic) = extract_os2_info(&font);

    let format = match ext.as_str() {
        "ttf" => "TrueType",
        "otf" => "OpenType",
        "ttc" | "otc" => "Collection",
        "woff" => "WOFF",
        "woff2" => "WOFF2",
        "dfont" => "dfont",
        _ => "Unknown",
    };

    let source = FontliftFontSource::new(path.clone())
        .with_format(Some(format.to_string()))
        .with_face_index(Some(0))
        .with_collection_flag(Some(is_collection));

    let info = FontliftFontFaceInfo {
        source,
        postscript_name,
        full_name,
        family_name,
        style: style_name,
        weight: Some(weight),
        italic: Some(italic),
    };

    ValidationResult::success(path.clone(), info)
}

/// Extract names from font's name table
fn extract_names(font: &FontRef) -> (String, String, String, String) {
    let name_table = match font.name() {
        Ok(t) => t,
        Err(_) => {
            return (
                "Unknown".to_string(),
                "Unknown".to_string(),
                "Unknown".to_string(),
                "Regular".to_string(),
            )
        }
    };

    // Helper to find name by ID
    let find_name = |id: u16| -> Option<String> {
        name_table
            .name_record()
            .iter()
            .find(|r| r.name_id() == read_fonts::tables::name::NameId::new(id))
            .and_then(|r| r.string(name_table.string_data()).ok())
            .map(|s| s.to_string())
    };

    // Name IDs: 1=family, 2=subfamily, 4=full name, 6=PostScript name
    let family = find_name(1).unwrap_or_else(|| "Unknown".to_string());
    let style = find_name(2).unwrap_or_else(|| "Regular".to_string());
    let full_name = find_name(4).unwrap_or_else(|| format!("{} {}", family, style));
    let postscript = find_name(6).unwrap_or_else(|| family.replace(' ', ""));

    (postscript, full_name, family, style)
}

/// Extract weight and italic from OS/2 table
fn extract_os2_info(font: &FontRef) -> (u16, bool) {
    let os2 = font.os2();

    let weight = os2.as_ref().map(|t| t.us_weight_class()).unwrap_or(400);

    let italic = os2
        .as_ref()
        .map(|t| {
            let selection = t.fs_selection();
            // Bit 0 = italic
            selection.bits() & 1 != 0
        })
        .unwrap_or(false);

    (weight, italic)
}

fn main() {
    // Read input from stdin (JSON blob with paths and config)
    let stdin = io::stdin();
    let mut input_str = String::new();

    // Try to read JSON from stdin
    for line in stdin.lock().lines() {
        match line {
            Ok(l) => input_str.push_str(&l),
            Err(_) => break,
        }
    }

    let input: ValidatorInput = if input_str.is_empty() {
        // Fall back to CLI args
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            eprintln!("Usage: fontlift-validator <path1> [path2 ...] or pipe JSON to stdin");
            std::process::exit(1);
        }
        ValidatorInput {
            paths: args.into_iter().map(PathBuf::from).collect(),
            config: ValidatorConfig::default(),
        }
    } else {
        match serde_json::from_str(&input_str) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Invalid JSON input: {e}");
                std::process::exit(1);
            }
        }
    };

    // Validate each font
    let results: Vec<ValidationResult> = input
        .paths
        .iter()
        .map(|p| validate_font(p, &input.config))
        .collect();

    // Output JSON
    match serde_json::to_string(&results) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("Failed to serialize results: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn rejects_nonexistent_file() {
        let result = validate_font(
            &PathBuf::from("/nonexistent/font.ttf"),
            &ValidatorConfig::default(),
        );
        assert!(!result.ok);
        assert!(result.error.as_ref().unwrap().contains("not found"));
    }

    #[test]
    fn rejects_invalid_extension() {
        let mut tmp = NamedTempFile::with_suffix(".txt").unwrap();
        tmp.write_all(b"not a font").unwrap();
        let result = validate_font(&tmp.path().to_path_buf(), &ValidatorConfig::default());
        assert!(!result.ok);
        assert!(result.error.as_ref().unwrap().contains("extension"));
    }

    #[test]
    fn rejects_oversized_file() {
        let mut tmp = NamedTempFile::with_suffix(".ttf").unwrap();
        tmp.write_all(b"fake font data").unwrap();
        let config = ValidatorConfig {
            max_file_size_bytes: 5, // tiny limit
            ..Default::default()
        };
        let result = validate_font(&tmp.path().to_path_buf(), &config);
        assert!(!result.ok);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("exceeds maximum size"));
    }

    #[test]
    fn rejects_malformed_font() {
        let mut tmp = NamedTempFile::with_suffix(".ttf").unwrap();
        tmp.write_all(b"this is not a valid font file").unwrap();
        let result = validate_font(&tmp.path().to_path_buf(), &ValidatorConfig::default());
        assert!(!result.ok);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("Invalid font structure"));
    }

    #[test]
    fn sanitizes_long_errors() {
        let long_error = "x".repeat(300);
        let sanitized = sanitize_error(&long_error);
        assert!(sanitized.len() <= 203); // 200 + "..."
        assert!(sanitized.ends_with("..."));
    }
}
