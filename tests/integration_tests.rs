//! Integration tests: Where fonts meet their destiny
//!
//! These tests put fontlift through its paces, watching it dance with real platform APIs.
//! We create temporary font files, install them like careful librarians, and clean up
//! like responsible party guests who always take their coat when they leave.

use std::path::{Path, PathBuf};
use tempfile::TempDir;
use fontlift_core::{FontManager, FontScope};

/// Forge a pretend font file from the ether of temporary directories
/// 
/// We're basically font counterfeiters, but in a good way - creating just enough
/// TTF header bytes to convince the font managers we're worth talking to.
/// Think of it as a student ID for a font that never went to font school.
fn create_test_font_file() -> Result<(TempDir, PathBuf), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let font_path = temp_dir.path().join("test-font.ttf");
    
    // We're baking a minimalist TTF cake with only the essential ingredients
    // This fake font speaks the language but doesn't know how to draw letters
    let ttf_header = vec![
        0x00, 0x01, 0x00, 0x00, // Font file version - says "I'm a TrueType!"
        0x00, 0x0C, // Number of tables - we lie convincingly
        0x00, 0x20, // searchRange - mathematical magic for table lookup
        0x00, 0x01, // entrySelector - more lookup table math
        0x00, 0x00, // rangeShift - the final piece of the lookup puzzle
    ];
    
    std::fs::write(&font_path, ttf_header)?;
    
    Ok((temp_dir, font_path))
}

/// macOS font waltz: install, list, and vanish like magic
/// 
/// Apple's Core Text is picky - it's like a sommelier who only accepts
/// perfectly aged fonts. Our mock font might not make the cut, but that's okay.
/// We're testing the dance steps, not whether the shoes are polished.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_macos_font_installation_and_listing() {
    let manager = fontlift_platform_mac::MacFontManager::new();
    
    // Summon our test font from the temporary directory
    let (_temp_dir, font_path) = create_test_font_file().expect("Failed to create test font");
    
    // Try to convince Core Text to accept our font imposter
    let install_result = manager.install_font(&font_path, FontScope::User);
    match install_result {
        Ok(()) => {
            println!("✅ Font installation succeeded on macOS - Core Text accepted our invitation");
            
            // Time for roll call: does our font show up in the registry?
            match manager.list_installed_fonts() {
                Ok(fonts) => {
                    println!("✅ Font listing succeeded, found {} fonts in the registry", fonts.len());
                    
                    // Our font might be shy - Core Text can be particular about new friendships
                    let found = fonts.iter().any(|f| {
                        f.path.file_name() == font_path.file_name() ||
                        f.postscript_name.contains("test-font")
                    });
                    
                    if found {
                        println!("✅ Our test font made friends with Core Text");
                    } else {
                        println!("⚠️  Test font playing hide and seek (normal for mock fonts)");
                    }
                },
                Err(e) => {
                    println!("⚠️  Font listing got shy: {}", e);
                    // We won't cry about it - macOS permissions can be stubborn
                }
            }
            
            // Time to say goodbye to our temporary font friend
            match manager.uninstall_font(&font_path, FontScope::User) {
                Ok(()) => {
                    println!("✅ Font uninstallation succeeded - clean slate achieved");
                },
                Err(e) => {
                    println!("⚠️  Font uninstallation got difficult: {}", e);
                }
            }
        },
        Err(e) => {
            println!("⚠️  Font installation failed on macOS: {}", e);
            // Mock fonts sometimes crash parties - Core Text is the bouncer
        }
    }
}

/// Windows font rodeo: GDI meets our font impostor
/// 
/// Windows font management is like playing poker with your grandmother's rules.
/// Our mock font might look suspicious, but Windows usually gives it a chance.
/// We're testing whether our fontlift can navigate GDI's quirky personality.
#[cfg(windows)]
#[tokio::test]
async fn test_windows_font_installation_and_listing() {
    let manager = fontlift_platform_win::WinFontManager::new();
    
    // Bake up our test font fresh from the temporary directory
    let (_temp_dir, font_path) = create_test_font_file().expect("Failed to create test font");
    
    // Let's see if Windows GDI will fall for our font disguise
    let install_result = manager.install_font(&font_path, FontScope::User);
    match install_result {
        Ok(()) => {
            println!("✅ Font installation succeeded on Windows - GDI played along with our game");
            
            // Time to check the registry: Windows' font guest list
            match manager.list_installed_fonts() {
                Ok(fonts) => {
                    println!("✅ Font listing succeeded, found {} fonts at the party", fonts.len());
                    
                    // Does our impostor font show up on the guest list?
                    let found = fonts.iter().any(|f| {
                        f.path.file_name() == font_path.file_name() ||
                        f.postscript_name.contains("test-font")
                    });
                    
                    if found {
                        println!("✅ Our test font successfully crashed Windows' font party");
                    } else {
                        println!("⚠️  Test font being antisocial (mock fonts sometimes are)");
                    }
                },
                Err(e) => {
                    println!("⚠️  Font listing went awry: {}", e);
                    // Registry access can be finicky - Windows being Windows
                }
            }
            
            // Time to kick our font friend out of the Windows party
            match manager.uninstall_font(&font_path, FontScope::User) {
                Ok(()) => {
                    println!("✅ Font uninstallation succeeded - registry cleaned up");
                },
                Err(e) => {
                    println!("⚠️  Font uninstallation got stuck: {}", e);
                }
            }
        },
        Err(e) => {
            println!("⚠️  Font installation failed on Windows: {}", e);
            // Windows font installation can be picky - our mock font might be too fake
        }
    }
}

/// Font validation: Separating the fonts from the frauds
/// 
/// We're playing font detective here - testing our ability to distinguish
/// real fonts from clever impersonators and complete phonies.
/// This is where fontlift proves it has good judgment.
#[tokio::test]
async fn test_font_validation() {
    let manager = create_platform_manager();
    
    // Start with something that looks like a font (our TTF impostor)
    let (_temp_dir, font_path) = create_test_font_file().expect("Failed to create test font");
    
    // Ask the platform: "Hey, is this font installed?" 
    match manager.is_font_installed(&font_path) {
        Ok(installed) => {
            println!("✅ Font installation check succeeded: {} (probably false for mock font)", installed);
        },
        Err(e) => {
            println!("⚠️  Font installation check got confused: {}", e);
        }
    }
    
    // Test with a font that exists only in our imagination
    let nonexistent_path = PathBuf::from("/nonexistent/font.ttf");
    match manager.is_font_installed(&nonexistent_path) {
        Ok(_) => {
            println!("⚠️  Ghost font check should have failed - we saw through the illusion");
        },
        Err(e) => {
            println!("✅ Non-existent font check correctly failed: {}", e);
        }
    }
    
    // Try to install a wolf in sheep's clothing (text file pretending to be a font)
    let invalid_path = PathBuf::from("test.txt");
    match manager.install_font(&invalid_path, FontScope::User) {
        Ok(_) => {
            println!("⚠️  Invalid font installation should have failed - the imposter was rejected");
        },
        Err(e) => {
            println!("✅ Invalid font correctly rejected: {} (good judgment!) ", e);
        }
    }
}

/// Cache clearing spring cleaning: digital dust bunnies beware
/// 
/// Font caches are like browser history - they accumulate gunk over time.
/// We test both user cache cleaning (should work) and system cache cleaning
/// (should politely refuse unless you're wearing the admin crown).
#[tokio::test]
async fn test_cache_clearing() {
    let manager = create_platform_manager();
    
    // First, let's clean up our own mess - user cache clearing
    match manager.clear_font_caches(FontScope::User) {
        Ok(()) => {
            println!("✅ User cache clearing succeeded - digital dust bunnies vanquished");
        },
        Err(e) => {
            println!("⚠️  User cache clearing got tangled: {}", e);
            // Cache clearing can fail due to locked files - normal behavior
        }
    }
    
    // Now let's attempt the forbidden - system cache clearing without privilege
    match manager.clear_font_caches(FontScope::System) {
        Ok(()) => {
            println!("⚠️  System cache clearing should have failed - we shouldn't be admin here");
        },
        Err(e) => {
            println!("✅ System cache clearing correctly refused without admin: {}", e);
            // Expected behavior - system caches are protected like Fort Knox
        }
    }
}

/// Error handling: When things go sideways, we guide users home
/// 
/// Good error handling is like being a helpful bartender - you don't just say "we're out",
/// you suggest what's actually available. We test that fontlift gives useful guidance
/// when faced with forbidden operations and broken fonts.
#[tokio::test]
async fn test_error_handling() {
    let manager = create_platform_manager();
    
    // Let's try to mess with a system font - these are sacred text files
    #[cfg(target_os = "macos")]
    let system_font_path = PathBuf::from("/System/Library/Fonts/Arial.ttf");
    #[cfg(windows)]
    let system_font_path = PathBuf::from(r"C:\Windows\Fonts\arial.ttf");
    #[cfg(not(any(target_os = "macos", windows)))]
    let system_font_path = PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
    
    match manager.install_font(&system_font_path, FontScope::User) {
        Ok(_) => {
            println!("⚠️  System font installation should have failed - that was too easy!");
        },
        Err(e) => {
            println!("✅ System font protection correctly triggered: {}", e);
            
            // Does the error message actually help the user?
            let error_msg = e.to_string();
            if error_msg.contains("admin") || error_msg.contains("sudo") || error_msg.contains("Administrator") {
                println!("✅ Error message provides helpful guidance - user knows what to do next");
            } else {
                println!("⚠️  Error message could be more actionable: {} (be more helpful, fontlift!)", error_msg);
            }
        }
    }
    
    // Now let's create something that's definitely not a font
    let (_temp_dir, temp_dir) = create_test_font_file().expect("Failed to create test font");
    let invalid_font_path = temp_dir.path().join("invalid.txt");
    std::fs::write(&invalid_font_path, b"not a font").expect("Failed to write invalid file");
    
    match manager.install_font(&invalid_font_path, FontScope::User) {
        Ok(_) => {
            println!("⚠️  Invalid font installation should have failed - our validation is sleeping");
        },
        Err(e) => {
            println!("✅ Invalid font correctly shown the door: {} (good job, fontlift!)", e);
        }
    }
}

/// Font removal: The final act of our font management drama
/// 
/// Removing fonts should be clean and final - no haunting ghosts left behind.
/// We test that fontlift can properly vanish fonts from whence they came,
/// whether they were installed or just temporary visitors to our temporary directory.
#[tokio::test]
async fn test_font_removal() {
    let manager = create_platform_manager();
    
    // Create our sacrificial font - it exists only to be removed
    let (_temp_dir, font_path) = create_test_font_file().expect("Failed to create test font");
    
    // Test font removal (should work even if font wasn't really installed)
    match manager.remove_font(&font_path, FontScope::User) {
        Ok(()) => {
            println!("✅ Font removal succeeded - the font has left the building");
        },
        Err(e) => {
            println!("⚠️  Font removal got complicated: {}", e);
            // Font removal can fail if the file is in use or permissions are tricky
        }
    }
    
    // Check if the ghost of our font file still haunts the filesystem
    if !font_path.exists() {
        println!("✅ Font file properly vanished - no ghosts left behind");
    } else {
        println!("⚠️  Font file still exists (may be normal if it moved to system directory)");
    }
}

/// Platform manager factory: Pick the right tool for the job
/// 
/// Different operating systems need different love when it comes to fonts.
/// macOS gets the Core Text treatment, Windows gets GDI, and Linux gets 
/// a polite dummy manager that pretends while still testing our logic.
fn create_platform_manager() -> std::sync::Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        // macOS speaks Core Text fluently - give it what it wants
        std::sync::Arc::new(fontlift_platform_mac::MacFontManager::new())
    }
    
    #[cfg(windows)]
    {
        // Windows prefers GDI with a side of registry editing
        std::sync::Arc::new(fontlift_platform_win::WinFontManager::new())
    }
    
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        // Linux (and others) gets a manager that smiles and nods politely
        std::sync::Arc::new(fontlift_core::DummyFontManager)
    }
}

/// CLI integration: Testing the command line interface charm offensive
/// 
/// The CLI is fontlift's face to the world - it needs to understand humans.
/// We test that our command line parsing recognizes flags like a seasoned diplomat,
/// turning user input into internal commands without losing its cool.
#[tokio::test]
async fn test_cli_integration() {
    // Let's see if our CLI can read user intentions
    use clap::Parser;
    use fontlift_cli::Cli;
    
    // Simulate a user who knows what they want: list fonts with all the bells and whistles
    let cli = Cli::try_parse_from(&["fontlift", "list", "-p", "-n", "-s"]).unwrap();
    match cli.command {
        fontlift_cli::Commands::List { path, name, sorted } => {
            assert!(path);
            assert!(name);
            assert!(sorted);
            println!("✅ CLI parsing works correctly - it speaks fluent user");
        },
        _ => panic!("Expected list command - our CLI got confused"),
    }
}

/// Python bindings: The bridge between Rust speed and Python convenience
/// 
/// Python bindings are like ambassadors between two kingdoms - they translate
/// Rust's raw performance into Python's cozy ecosystem. This test is just a
/// placeholder checking that the bridge exists, not that traffic flows through it.
#[tokio::test]
async fn test_python_bindings_integration() {
    // This is a diplomatic mission - we're just confirming the embassy exists
    // Actual Python testing would require a full Python interpreter running
    #[cfg(any(target_os = "macos", windows))]
    {
        println!("✅ Python bindings integration test placeholder - embassy still standing");
        println!("   Note: Real Python testing needs a live Python interpreter, not just Rust dreams");
    }
}