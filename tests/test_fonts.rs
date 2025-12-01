//! Test font utilities and dataset management
//!
//! This module provides utilities for creating and managing test font files
//! for validation and integration testing.

use std::path::PathBuf;
use std::fs;
use tempfile::TempDir;

/// Different font formats for testing
#[derive(Debug, Clone)]
pub enum TestFontFormat {
    TrueType,
    OpenType,
    WOFF,
    WOFF2,
    TTC,
}

impl TestFontFormat {
    /// Get file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            TestFontFormat::TrueType => "ttf",
            TestFontFormat::OpenType => "otf",
            TestFontFormat::WOFF => "woff",
            TestFontFormat::WOFF2 => "woff2",
            TestFontFormat::TTC => "ttc",
        }
    }
}

/// A test font with metadata
#[derive(Debug, Clone)]
pub struct TestFont {
    pub name: String,
    pub family: String,
    pub style: String,
    pub format: TestFontFormat,
    pub path: PathBuf,
}

impl TestFont {
    /// Create a new test font
    pub fn new(name: &str, family: &str, style: &str, format: TestFontFormat) -> Self {
        Self {
            name: name.to_string(),
            family: family.to_string(),
            style: style.to_string(),
            format,
            path: PathBuf::new(),
        }
    }
}

/// Test font dataset manager
pub struct TestFontDataset {
    temp_dir: Option<TempDir>,
    fonts: Vec<TestFont>,
}

impl TestFontDataset {
    /// Create a new test font dataset
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = Some(TempDir::new()?);
        Ok(Self {
            temp_dir,
            fonts: Vec::new(),
        })
    }
    
    /// Create a comprehensive test font dataset
    pub fn create_comprehensive_dataset() -> Result<Self, Box<dyn std::error::Error>> {
        let mut dataset = Self::new()?;
        
        // Create test fonts in various formats
        dataset.create_font("TestSans-Regular", "TestSans", "Regular", TestFontFormat::TrueType)?;
        dataset.create_font("TestSans-Bold", "TestSans", "Bold", TestFontFormat::TrueType)?;
        dataset.create_font("TestSans-Italic", "TestSans", "Italic", TestFontFormat::OpenType)?;
        dataset.create_font("TestSerif-Regular", "TestSerif", "Regular", TestFontFormat::OpenType)?;
        dataset.create_font("TestMono-Regular", "TestMono", "Regular", TestFontFormat::WOFF)?;
        dataset.create_font("TestDisplay-Bold", "TestDisplay", "Bold", TestFontFormat::WOFF2)?;
        
        // Create a font collection
        dataset.create_font_collection("TestCollection")?;
        
        Ok(dataset)
    }
    
    /// Create a minimal test font file
    pub fn create_font(&mut self, name: &str, family: &str, style: &str, format: TestFontFormat) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let filename = format!("{}.{}", name, format.extension());
        let font_path = temp_dir.path().join(filename);
        
        // Create minimal font file based on format
        let font_data = self.create_minimal_font_data(&format)?;
        fs::write(&font_path, font_data)?;
        
        let mut test_font = TestFont::new(name, family, style, format);
        test_font.path = font_path;
        
        self.fonts.push(test_font);
        Ok(())
    }
    
    /// Create a font collection file
    pub fn create_font_collection(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let font_path = temp_dir.path().join(format!("{}.ttc", name));
        
        // Create minimal TTC file header
        let ttc_header = vec![
            b't', b't', b'c', b'f', // TTC signature
            0x00, 0x01, 0x00, 0x00, // Version
            0x00, 0x00, 0x00, 0x02, // Number of fonts
        ];
        
        fs::write(&font_path, ttc_header)?;
        
        let test_font = TestFont::new(name, "TestCollection", "Collection", TestFontFormat::TTC);
        self.fonts.push(test_font);
        Ok(())
    }
    
    /// Create minimal font data for testing
    fn create_minimal_font_data(&self, format: &TestFontFormat) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match format {
            TestFontFormat::TrueType | TestFontFormat::OpenType => {
                // Minimal TTF/OTF header
                Ok(vec![
                    0x00, 0x01, 0x00, 0x00, // SFNT version
                    0x00, 0x0C, // Number of tables
                    0x00, 0x20, // searchRange
                    0x00, 0x01, // entrySelector
                    0x00, 0x00, // rangeShift
                    // Table directory (minimal)
                    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x00, 0x00, 0x00, 0x04, // cmap
                    0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x04, // glyf
                    0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00, 0x04, // head
                    0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x04, // hhea
                    0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3C, 0x00, 0x00, 0x00, 0x04, // hmtx
                    0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x48, 0x00, 0x00, 0x00, 0x04, // loca
                    0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x04, // maxp
                    0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x04, // name
                    0x00, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6C, 0x00, 0x00, 0x00, 0x04, // post
                    0x00, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x00, 0x00, 0x00, 0x04, // OS/2
                    0x00, 0x0B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84, 0x00, 0x00, 0x00, 0x04, // vhea
                    0x00, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x90, 0x00, 0x00, 0x00, 0x04, // vmtx
                ])
            },
            TestFontFormat::WOFF => {
                // Minimal WOFF header
                Ok(vec![
                    b'w', b'O', b'F', b'F', // WOFF signature
                    0x00, 0x01, 0x00, 0x00, // Flavor
                    0x00, 0x00, 0x00, 0x20, // Length
                    0x00, 0x00, 0x00, 0x10, // numTables
                    0x00, 0x00, 0x00, 0x00, // reserved
                    0x00, 0x00, 0x00, 0x00, // totalSfntSize
                    0x00, 0x00, 0x00, 0x00, // majorVersion
                    0x00, 0x00, 0x00, 0x01, // minorVersion
                    0x00, 0x00, 0x00, 0x00, // metaOffset
                    0x00, 0x00, 0x00, 0x00, // metaLength
                    0x00, 0x00, 0x00, 0x00, // metaOrigLength
                    0x00, 0x00, 0x00, 0x00, // privOffset
                    0x00, 0x00, 0x00, 0x00, // privLength
                ])
            },
            TestFontFormat::WOFF2 => {
                // Minimal WOFF2 header
                Ok(vec![
                    b'w', b'O', b'F', b'2', // WOFF2 signature
                    0x00, 0x00, 0x00, 0x00, // flags
                    0x00, 0x00, 0x00, 0x20, // totalCompressedSize
                    0x00, 0x00, 0x00, 0x10, // length
                    0x00, 0x00, 0x00, 0x00, // numTables
                    0x00, 0x00, 0x00, 0x00, // reserved
                ])
            },
            TestFontFormat::TTC => {
                // Minimal TTC header is already handled in create_font_collection
                Ok(vec![])
            }
        }
    }
    
    /// Get all test fonts
    pub fn fonts(&self) -> &[TestFont] {
        &self.fonts
    }
    
    /// Get fonts by format
    pub fn fonts_by_format(&self, format: &TestFontFormat) -> Vec<&TestFont> {
        self.fonts.iter().filter(|f| matches!(&f.format, format)).collect()
    }
    
    /// Get fonts by family
    pub fn fonts_by_family(&self, family: &str) -> Vec<&TestFont> {
        self.fonts.iter().filter(|f| f.family == family).collect()
    }
    
    /// Get a specific font by name
    pub fn font_by_name(&self, name: &str) -> Option<&TestFont> {
        self.fonts.iter().find(|f| f.name == name)
    }
    
    /// Get the temporary directory path
    pub fn temp_dir_path(&self) -> &PathBuf {
        &self.temp_dir.as_ref().unwrap().path()
    }
    
    /// Create an invalid font file for testing error handling
    pub fn create_invalid_font(&mut self, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let font_path = temp_dir.path().join(format!("{}.txt", name));
        
        // Create invalid font file
        fs::write(&font_path, b"This is not a font file")?;
        
        Ok(font_path)
    }
    
    /// Create a corrupted font file for testing error handling
    pub fn create_corrupted_font(&mut self, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let font_path = temp_dir.path().join(format!("{}.ttf", name));
        
        // Create corrupted TTF file (invalid header)
        let corrupted_data = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00];
        fs::write(&font_path, corrupted_data)?;
        
        Ok(font_path)
    }
    
    /// Remove a font file (for testing removal operations)
    pub fn remove_font(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pos) = self.fonts.iter().position(|f| f.name == name) {
            let font = &self.fonts[pos];
            if font.path.exists() {
                fs::remove_file(&font.path)?;
            }
            self.fonts.remove(pos);
        }
        Ok(())
    }
}

impl Drop for TestFontDataset {
    fn drop(&mut self) {
        // TempDir will automatically clean up when dropped
    }
}

/// Create a comprehensive test dataset for integration testing
pub fn create_test_dataset() -> Result<TestFontDataset, Box<dyn std::error::Error>> {
    TestFontDataset::create_comprehensive_dataset()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_test_font_dataset_creation() {
        let dataset = TestFontDataset::new().unwrap();
        assert!(dataset.fonts().is_empty());
    }
    
    #[test]
    fn test_create_single_font() {
        let mut dataset = TestFontDataset::new().unwrap();
        dataset.create_font("TestFont", "TestFamily", "Regular", TestFontFormat::TrueType).unwrap();
        
        assert_eq!(dataset.fonts().len(), 1);
        let font = &dataset.fonts()[0];
        assert_eq!(font.name, "TestFont");
        assert_eq!(font.family, "TestFamily");
        assert!(font.path.exists());
    }
    
    #[test]
    fn test_create_comprehensive_dataset() {
        let dataset = TestFontDataset::create_comprehensive_dataset().unwrap();
        
        assert!(!dataset.fonts().is_empty());
        assert!(dataset.fonts().len() >= 7); // At least the fonts we create
        
        // Check we have different formats
        let ttf_fonts = dataset.fonts_by_format(&TestFontFormat::TrueType);
        let otf_fonts = dataset.fonts_by_format(&TestFontFormat::OpenType);
        let woff_fonts = dataset.fonts_by_format(&TestFontFormat::WOFF);
        
        assert!(!ttf_fonts.is_empty());
        assert!(!otf_fonts.is_empty());
        assert!(!woff_fonts.is_empty());
    }
    
    #[test]
    fn test_font_filtering() {
        let dataset = TestFontDataset::create_comprehensive_dataset().unwrap();
        
        let test_sans_fonts = dataset.fonts_by_family("TestSans");
        assert!(!test_sans_fonts.is_empty());
        
        let test_mono_fonts = dataset.fonts_by_family("TestMono");
        assert!(!test_mono_fonts.is_empty());
        
        let specific_font = dataset.font_by_name("TestSans-Bold");
        assert!(specific_font.is_some());
        assert_eq!(specific_font.unwrap().style, "Bold");
    }
    
    #[test]
    fn test_invalid_font_creation() {
        let mut dataset = TestFontDataset::new().unwrap();
        let invalid_path = dataset.create_invalid_font("Invalid").unwrap();
        
        assert!(invalid_path.exists());
        assert!(invalid_path.extension().unwrap() == "txt");
    }
    
    #[test]
    fn test_corrupted_font_creation() {
        let mut dataset = TestFontDataset::new().unwrap();
        let corrupted_path = dataset.create_corrupted_font("Corrupted").unwrap();
        
        assert!(corrupted_path.exists());
        assert!(corrupted_path.extension().unwrap() == "ttf");
    }
}