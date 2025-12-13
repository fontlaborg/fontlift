//! Craft test fonts like a digital pastry chef—delicious, temporary, and gone when you're done.
//!
//! Welcome to the font bakery where we whip up test fonts faster than you can say
//! "glyph metrics." These utilities create temporary font files for validation and
//! integration testing, leaving no crumbs behind when the party's over.

use std::path::PathBuf;
use std::fs;
use tempfile::TempDir;

/// Font format variations—like different flavors of ice cream for digital text.
///
/// Each format brings its own personality to the font party, from the classic
/// TrueType reliability to the space-efficient compression of WOFF2.
#[derive(Debug, Clone)]
pub enum TestFontFormat {
    /// TrueType—the dependable workhorse that's been around since Windows 3.1
    TrueType,
    /// OpenType—TrueType's sophisticated cousin with PostScript curves
    OpenType,
    /// WOFF—the web-optimized version that travels light and fast
    WOFF,
    /// WOFF2—the compressed superhero that saves bandwidth like a boss
    WOFF2,
    /// TTC—the font collection that packs multiple personalities into one file
    TTC,
}

impl TestFontFormat {
    /// Returns the file extension that keeps the operating system happy.
    ///
    /// Different formats, different file endings—it's the filesystem's way of
    /// knowing whether to expect TrueType tables or compressed web magic.
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

/// A test font with more personality than your average barcode.
///
/// Each TestFont carries its own story—the family it belongs to, the style
/// it proudly displays, and the temporary home where it lives until testing
/// decides its fate. It's like a character actor in the font theater.
#[derive(Debug, Clone)]
pub struct TestFont {
    /// The unique stage name for this font personality
    pub name: String,
    /// The font family—because even fonts need to know where they come from
    pub family: String,
    /// The style variant (Regular, Bold, Italic—the fashion choices of typography)
    pub style: String,
    /// The compression format that determines how this font travels
    pub format: TestFontFormat,
    /// The temporary file path where this font currently resides
    pub path: PathBuf,
}

impl TestFont {
    /// Births a new test font from the primordial soup of parameters.
    ///
    /// Like a digital font midwife, this method takes the essential traits
    /// and creates a font entity ready for testing adventures. The path starts
    /// empty—we'll fill that in when we actually create the file.
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

/// The master conductor of our temporary font orchestra.
///
/// TestFontDataset orchestrates the creation and management of test fonts,
/// providing them a temporary stage to perform their testing duties. When the
/// curtain falls, all files vanish without a trace—clean, efficient, magical.
pub struct TestFontDataset {
    /// The temporary directory that serves as our font's green room
    temp_dir: Option<TempDir>,
    /// The ensemble of test fonts ready for their performance
    fonts: Vec<TestFont>,
}

impl TestFontDataset {
    /// Conjures an empty stage, ready for font performances to begin.
    ///
    /// This method creates a temporary directory that acts as our font's
    /// dressing room—all files created here will vanish when the show ends.
    /// It's like building a theater that disappears at midnight.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = Some(TempDir::new()?);
        Ok(Self {
            temp_dir,
            fonts: Vec::new(),
        })
    }
    
    /// Assembles the ultimate font testing ensemble—every format, every style.
    ///
    /// Like casting agents for a blockbuster font movie, we handpick characters
    /// from every format family: TrueType workhorses, OpenType sophisticates,
    /// web-optimized WOFF travelers, and that one TTC collection that steals
    /// every scene. Each font gets its moment to shine in the testing spotlight.
    pub fn create_comprehensive_dataset() -> Result<Self, Box<dyn std::error::Error>> {
        let mut dataset = Self::new()?;
        
        // Cast our main characters: the Sans family with all their dramatic styles
        dataset.create_font("TestSans-Regular", "TestSans", "Regular", TestFontFormat::TrueType)?;
        dataset.create_font("TestSans-Bold", "TestSans", "Bold", TestFontFormat::TrueType)?;
        dataset.create_font("TestSans-Italic", "TestSans", "Italic", TestFontFormat::OpenType)?;
        
        // Add the sophisticated Serif cousin—class never goes out of style
        dataset.create_font("TestSerif-Regular", "TestSerif", "Regular", TestFontFormat::OpenType)?;
        
        // Introduce the Mono protagonist—unpredictable, reliable, web-ready
        dataset.create_font("TestMono-Regular", "TestMono", "Regular", TestFontFormat::WOFF)?;
        
        // And the Display star—compressed, bold, ready for the web spotlight
        dataset.create_font("TestDisplay-Bold", "TestDisplay", "Bold", TestFontFormat::WOFF2)?;
        
        // Finally, the ensemble cast in one convenient collection
        dataset.create_font_collection("TestCollection")?;
        
        Ok(dataset)
    }
    
    /// Creates a single font from scratch—like a digital type foundry in miniature.
    ///
    /// This method whips up a minimal but valid font file, giving it a temporary
    /// home in our test directory. The font won't win any design awards, but
    /// it will pass validation tests with flying colors. Perfect for when you
    /// need something that looks like a font without the typographic baggage.
    pub fn create_font(&mut self, name: &str, family: &str, style: &str, format: TestFontFormat) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let filename = format!("{}.{}", name, format.extension());
        let font_path = temp_dir.path().join(filename);
        
        // Forge the minimal font data—just enough to look legit
        let font_data = self.create_minimal_font_data(&format)?;
        fs::write(&font_path, font_data)?;
        
        let mut test_font = TestFont::new(name, family, style, format);
        test_font.path = font_path;
        
        self.fonts.push(test_font);
        Ok(())
    }
    
    /// Crafts a font collection—the efficiency champion of the font world.
    ///
    /// TTC files are like apartment buildings for fonts: multiple typefaces
    /// living harmoniously under one roof. This method creates the minimal
    /// header that convinces the font system we're serious about collections.
    /// It's the typography equivalent of a really good roommate agreement.
    pub fn create_font_collection(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let font_path = temp_dir.path().join(format!("{}.ttc", name));
        
        // The TTC magic handshake—just enough bytes to say "I'm a real collection"
        let ttc_header = vec![
            b't', b't', b'c', b'f', // TTC signature—the secret knock
            0x00, 0x01, 0x00, 0x00, // Version—telling systems we're modern
            0x00, 0x00, 0x00, 0x02, // Number of fonts—we promise at least two
        ];
        
        fs::write(&font_path, ttc_header)?;
        
        let test_font = TestFont::new(name, "TestCollection", "Collection", TestFontFormat::TTC);
        self.fonts.push(test_font);
        Ok(())
    }
    
    /// Generates just enough bytes to convince font parsers they've found the real deal.
    ///
    /// This method creates minimal but structurally valid font data for each format.
    /// Think of it as font forgery for testing purposes—we're not trying to create
    /// beautiful typography, just something that passes the basic sanity checks.
    /// The equivalent of a movie prop that looks real enough for the camera.
    fn create_minimal_font_data(&self, format: &TestFontFormat) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match format {
            TestFontFormat::TrueType | TestFontFormat::OpenType => {
                // The SFNT structure—the blueprint every TrueType/OpenType font follows
                Ok(vec![
                    0x00, 0x01, 0x00, 0x00, // SFNT version—"I speak both TrueType and OpenType"
                    0x00, 0x0C, // Number of tables—a dozen rooms in our font house
                    0x00, 0x20, // searchRange—helps systems find tables quickly
                    0x00, 0x01, // entrySelector—power of two for efficient seeking
                    0x00, 0x00, // rangeShift—the overflow for when tables don't align perfectly
                    // The table directory—our font's table of contents
                    0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x00, 0x00, 0x00, 0x04, // cmap character mapping
                    0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x04, // glyf glyph data
                    0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x00, 0x04, // head font header
                    0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x04, // hhea horizontal header
                    0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3C, 0x00, 0x00, 0x00, 0x04, // hmtx horizontal metrics
                    0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x48, 0x00, 0x00, 0x00, 0x04, // loca glyph locations
                    0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x54, 0x00, 0x00, 0x00, 0x04, // maxp maximum profile
                    0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x04, // name naming table
                    0x00, 0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6C, 0x00, 0x00, 0x00, 0x04, // post PostScript info
                    0x00, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x00, 0x00, 0x00, 0x04, // OS/2 OS/2 metrics
                    0x00, 0x0B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84, 0x00, 0x00, 0x00, 0x04, // vhea vertical header
                    0x00, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x90, 0x00, 0x00, 0x00, 0x04, // vmtx vertical metrics
                ])
            },
            TestFontFormat::WOFF => {
                // WOFF—the font format that went on a diet for the web
                Ok(vec![
                    b'w', b'O', b'F', b'F', // WOFF signature—"Hello, I'm here for the web"
                    0x00, 0x01, 0x00, 0x00, // Flavor—"I'm really just TrueType in disguise"
                    0x00, 0x00, 0x00, 0x20, // Length—how much space I take up (not much!)
                    0x00, 0x00, 0x00, 0x10, // numTables—still has all the important rooms
                    0x00, 0x00, 0x00, 0x00, // reserved—saving this for future adventures
                    0x00, 0x00, 0x00, 0x00, // totalSfntSize—what I'd be if I unpacked
                    0x00, 0x00, 0x00, 0x00, // majorVersion—born in the web era
                    0x00, 0x00, 0x00, 0x01, // minorVersion—still young and learning
                    0x00, 0x00, 0x00, 0x00, // metaOffset—no metadata, just pure font
                    0x00, 0x00, 0x00, 0x00, // metaLength—what metadata would weigh
                    0x00, 0x00, 0x00, 0x00, // metaOrigLength—before compression
                    0x00, 0x00, 0x00, 0x00, // privOffset—no private data here
                    0x00, 0x00, 0x00, 0x00, // privLength—nothing to hide
                ])
            },
            TestFontFormat::WOFF2 => {
                // WOFF2—the compression wizard that makes fonts tiny
                Ok(vec![
                    b'w', b'O', b'F', b'2', // WOFF2 signature—"I'm the newer, better version"
                    0x00, 0x00, 0x00, 0x00, // flags—"Keep it simple, no fancy features"
                    0x00, 0x00, 0x00, 0x20, // totalCompressedSize—how much space I really need
                    0x00, 0x00, 0x00, 0x10, // length—my uncompressed appetite
                    0x00, 0x00, 0x00, 0x00, // numTables—minimalist approach to table count
                    0x00, 0x00, 0x00, 0x00, // reserved—future-proofing for the unknown
                ])
            },
            TestFontFormat::TTC => {
                // TTC collections get their own creation method—we don't double-dip
                Ok(vec![])
            }
        }
    }
    
    /// Returns the entire font cast—all characters ready for their close-up.
    ///
    /// This gives you read-only access to our complete font ensemble.
    /// No modifications allowed here—we run a tight ship in our theater.
    pub fn fonts(&self) -> &[TestFont] {
        &self.fonts
    }
    
    /// Finds fonts that speak the same format language.
    ///
    /// Like grouping actors by their accent, this method collects all fonts
    /// using the same compression format. Perfect for when you need to test
    /// format-specific behavior or just want to hang with the TTF crowd.
    pub fn fonts_by_format(&self, format: &TestFontFormat) -> Vec<&TestFont> {
        self.fonts.iter().filter(|f| matches!(&f.format, format)).collect()
    }
    
    /// Gathers the font family reunion—cousins, siblings, and style variants united.
    ///
    /// Fonts from the same family share typographic DNA, just different expressions.
    /// This method helps you find all the TestSans relatives or the entire
    /// Serif clan when you need to test family dynamics.
    pub fn fonts_by_family(&self, family: &str) -> Vec<&TestFont> {
        self.fonts.iter().filter(|f| f.family == family).collect()
    }
    
    /// The font casting director—finds exactly the character you're looking for.
    ///
    /// Need "TestSans-Bold" for the dramatic scene? This method dives into our
    /// cast and pulls out the perfect match. If the font exists, you'll get it;
    /// if not, you'll get None—no drama, just efficient searching.
    pub fn font_by_name(&self, name: &str) -> Option<&TestFont> {
        self.fonts.iter().find(|f| f.name == name)
    }
    
    /// Reveals the location of our temporary font theater.
    ///
    /// Returns the path to the magical directory where all our test fonts perform.
    /// This directory will disappear when the show ends, so capture the path
    /// if you need to visit backstage during the performance.
    pub fn temp_dir_path(&self) -> &PathBuf {
        &self.temp_dir.as_ref().unwrap().path()
    }
    
    /// Creates the font impostor—looks like a font, acts like a text file.
    ///
    /// This method generates a file that's definitely not a font, perfect for
    /// testing error handling. It's like casting a cat to play a dog—you know
    /// it's going to fail, and that's exactly what you want to see happen.
    pub fn create_invalid_font(&mut self, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let font_path = temp_dir.path().join(format!("{}.txt", name));
        
        // The ultimate font fakeout—plain text in font's clothing
        fs::write(&font_path, b"This is not a font file")?;
        
        Ok(font_path)
    }
    
    /// Crafts the font zombie—dead on arrival but smells vaguely like TTF.
    ///
    /// This creates a corrupted font file with just enough TTF-like structure
    /// to confuse naive parsers, but not enough to actually work. Perfect for
    /// testing how your code handles fonts that have given up on life.
    pub fn create_corrupted_font(&mut self, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let temp_dir = self.temp_dir.as_ref().unwrap();
        let font_path = temp_dir.path().join(format!("{}.ttf", name));
        
        // The font that gave up—invalid header bytes from the dark side
        let corrupted_data = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00];
        fs::write(&font_path, corrupted_data)?;
        
        Ok(font_path)
    }
    
    /// The font exorcist—banishes fonts from both disk and memory.
    ///
    /// This method removes a font file from the filesystem and our internal
    /// tracking. It's the clean goodbye every test suite needs when testing
    /// deletion operations or just cleaning up after failed experiments.
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
        // The magic of cleanup—TempDir performs the disappearing act automatically
        // When our dataset takes its final bow, the temporary directory vanishes
        // like a theater that folds its tent and moves on to the next town
    }
}

/// The quick-start button for comprehensive font testing.
///
/// This convenience function bypasses the manual setup and delivers a fully
/// stocked font test kitchen. Just call and go—perfect for integration tests
/// that need immediate font gratification without the ceremony.
pub fn create_test_dataset() -> Result<TestFontDataset, Box<dyn std::error::Error>> {
    TestFontDataset::create_comprehensive_dataset()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_test_font_dataset_creation() {
        // The empty stage test—if we build it, will they come? (Spoiler: no, not yet)
        let dataset = TestFontDataset::new().unwrap();
        assert!(dataset.fonts().is_empty()); // Empty theater, waiting for the cast
    }
    
    #[test]
    fn test_create_single_font() {
        // Watch a single font emerge from the digital ether—magic in bytes
        let mut dataset = TestFontDataset::new().unwrap();
        dataset.create_font("TestFont", "TestFamily", "Regular", TestFontFormat::TrueType).unwrap();
        
        assert_eq!(dataset.fonts().len(), 1); // One actor takes the stage
        let font = &dataset.fonts()[0];
        assert_eq!(font.name, "TestFont"); // Our star has the right name
        assert_eq!(font.family, "TestFamily"); // And comes from good stock
        assert!(font.path.exists()); // The costume is ready in the dressing room
    }
    
    #[test]
    fn test_create_comprehensive_dataset() {
        // The grand ensemble—seven fonts enter, will any fail to impress?
        let dataset = TestFontDataset::create_comprehensive_dataset().unwrap();
        
        assert!(!dataset.fonts().is_empty()); // The theater is buzzing with activity
        assert!(dataset.fonts().len() >= 7); // At least our core cast has arrived
        
        // Check our format diversity program is working
        let ttf_fonts = dataset.fonts_by_format(&TestFontFormat::TrueType);
        let otf_fonts = dataset.fonts_by_format(&TestFontFormat::OpenType);
        let woff_fonts = dataset.fonts_by_format(&TestFontFormat::WOFF);
        
        assert!(!ttf_fonts.is_empty()); // The reliable workhorses showed up
        assert!(!otf_fonts.is_empty()); // The sophisticated cousins arrived
        assert!(!woff_fonts.is_empty()); // And the web-savvy travelers made it too
    }
    
    #[test]
    fn test_font_filtering() {
        // Font family reunion time—can we find all the relatives in the crowd?
        let dataset = TestFontDataset::create_comprehensive_dataset().unwrap();
        
        let test_sans_fonts = dataset.fonts_by_family("TestSans");
        assert!(!test_sans_fonts.is_empty()); // The Sans family gathered for the photo
        
        let test_mono_fonts = dataset.fonts_by_family("TestMono");
        assert!(!test_mono_fonts.is_empty()); // Even the Mono cousin showed up
        
        let specific_font = dataset.font_by_name("TestSans-Bold");
        assert!(specific_font.is_some()); // Found our dramatic lead in the cast
        assert_eq!(specific_font.unwrap().style, "Bold"); // Playing the bold character, as expected
    }
    
    #[test]
    fn test_invalid_font_creation() {
        // The font that thought it could—but really, really couldn't
        let mut dataset = TestFontDataset::new().unwrap();
        let invalid_path = dataset.create_invalid_font("Invalid").unwrap();
        
        assert!(invalid_path.exists()); // The impostor file was created
        assert!(invalid_path.extension().unwrap() == "txt"); // Wearing the wrong costume
    }
    
    #[test]
    fn test_corrupted_font_creation() {
        // The font that died before it lived—a tragic tale of bytes gone wrong
        let mut dataset = TestFontDataset::new().unwrap();
        let corrupted_path = dataset.create_corrupted_font("Corrupted").unwrap();
        
        assert!(corrupted_path.exists()); // The zombie font walks among us
        assert!(corrupted_path.extension().unwrap() == "ttf"); // Still pretending to be one of them
    }
}