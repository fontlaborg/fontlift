//! Integration tests for fontlift-core

#[cfg(test)]
mod integration_tests {
    use crate::validation;
    use crate::{
        cache, DummyFontManager, FontError, FontManager, FontScope, FontliftFontFaceInfo,
        FontliftFontSource,
    };
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;
    
    #[test]
    fn test_font_validation_valid_extensions() {
        let valid_paths = vec![
            PathBuf::from("test.ttf"),
            PathBuf::from("test.otf"),
            PathBuf::from("test.TTC"),
            PathBuf::from("test.woff2"),
        ];
        
        for path in valid_paths {
            assert!(validation::is_valid_font_extension(&path));
        }
    }
    
    #[test]
    fn test_font_validation_invalid_extensions() {
        let invalid_paths = vec![
            PathBuf::from("test.txt"),
            PathBuf::from("test.pdf"),
            PathBuf::from("test.doc"),
            PathBuf::from("test"),
        ];
        
        for path in invalid_paths {
            assert!(!validation::is_valid_font_extension(&path));
        }
    }
    
    #[test]
    fn test_basic_font_info_extraction() {
        let path = PathBuf::from("/fonts/OpenSans-Bold.ttf");
        let info = validation::extract_basic_info_from_path(&path);

        assert_eq!(info.source.path, path);
        assert_eq!(info.postscript_name, "OpenSans-Bold");
        assert_eq!(info.family_name, "OpenSans");
        assert_eq!(info.style, "Bold");
        assert_eq!(info.source.format, Some("TTF".to_string()));
    }
    
    #[test]
    fn test_font_info_extraction_simple() {
        let path = PathBuf::from("/fonts/Arial.ttf");
        let info = validation::extract_basic_info_from_path(&path);

        assert_eq!(info.source.path, path);
        assert_eq!(info.postscript_name, "Arial");
        assert_eq!(info.family_name, "Arial");
        assert_eq!(info.style, "Regular");
        assert_eq!(info.source.format, Some("TTF".to_string()));
    }
    
    #[test]
    fn test_font_scope_descriptions() {
        assert_eq!(FontScope::User.description(), "user-level");
        assert_eq!(FontScope::System.description(), "system-level");
    }
    
    #[test]
    fn test_error_creation() {
        let path = PathBuf::from("/nonexistent/font.ttf");
        let error = FontError::FontNotFound(path.clone());
        assert!(error.to_string().contains("Font file not found"));
        assert!(matches!(error, FontError::FontNotFound(p) if p == path));
    }
    
    #[test]
    fn test_cache_clear_result() {
        let result = fontlift_core::cache::CacheClearResult::success(3, false)
            .with_warning("Some fonts may require restart".to_string());
        
        assert_eq!(result.entries_cleared, 3);
        assert!(!result.restart_required);
        assert_eq!(result.warnings.len(), 1);
    }
    
    #[test]
    fn test_dummy_font_manager() {
        let manager = DummyFontManager;
        let source = FontliftFontSource::new(PathBuf::from("test.ttf"));

        // All operations should return UnsupportedOperation
        assert!(matches!(
            manager.install_font(&source),
            Err(FontError::UnsupportedOperation(_))
        ));

        assert!(matches!(
            manager.is_font_installed(&source),
            Err(FontError::UnsupportedOperation(_))
        ));
    }
    
    #[test]
    fn test_font_info_creation() {
        let path = PathBuf::from("/fonts/TestFont-Regular.otf");
        let info = FontliftFontFaceInfo::new(
            FontliftFontSource::new(path.clone()),
            "TestFont-Regular".to_string(),
            "TestFont Regular".to_string(),
            "TestFont".to_string(),
            "Regular".to_string(),
        );

        assert_eq!(info.source.path, path);
        assert_eq!(info.postscript_name, "TestFont-Regular");
        assert_eq!(info.full_name, "TestFont Regular");
        assert_eq!(info.family_name, "TestFont");
        assert_eq!(info.style, "Regular");
        assert_eq!(info.weight, None);
        assert_eq!(info.italic, None);
    }
    
    #[test]
    fn test_font_info_filename_stem() {
        let path = PathBuf::from("/fonts/Complex-Font-Name.ttf");
        let info = FontliftFontFaceInfo::new(
            FontliftFontSource::new(path.clone()),
            "Complex-Font-Name".to_string(),
            "Complex Font Name".to_string(),
            "Complex Font".to_string(),
            "Name".to_string(),
        );
        
        assert_eq!(info.filename_stem(), Some("Complex-Font-Name"));
    }
    
    #[test]
    fn test_temp_dir_operations() {
        let temp_dir = TempDir::new().unwrap();
        let font_path = temp_dir.path().join("test.ttf");
        
        // Create a dummy font file
        std::fs::write(&font_path, b"dummy font content").unwrap();
        
        // Test validation
        assert!(validation::is_valid_font_extension(&font_path));
        assert!(validation::validate_font_file(&font_path).is_ok());
        
        // Test info extraction
        let info = validation::extract_basic_info_from_path(&font_path);
        assert_eq!(info.postscript_name, "test");
        assert_eq!(info.family_name, "test");
        assert_eq!(info.style, "Regular");
    }
    
    #[test]
    fn test_font_validation_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/font.ttf");
        let result = validation::validate_font_file(&path);
        
        assert!(matches!(result, Err(FontError::FontNotFound(_))));
    }
    
    #[test]
    fn test_font_validation_not_a_file() {
        let temp_dir = TempDir::new().unwrap();
        let result = validation::validate_font_file(&temp_dir.path().to_path_buf());
        
        assert!(matches!(result, Err(FontError::InvalidFormat(_))));
    }
}
