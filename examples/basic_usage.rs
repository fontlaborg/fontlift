//! FontLift: Your Font Manager's Friendly Neighbor
//!
//! Watch as fonts dance between installation and removal, all while 
//! maintaining that delightful system harmony. This example shows how
//! FontLift handles the heavy lifting so you don't have to:
//! - Create a platform-aware font manager that just works
//! - Install fonts like placing books on a cozy shelf
//! - Uninstall fonts as gently as removing sticky notes
//! - List your font collection with style
//! - Clear caches when things get a bit cluttered

use anyhow::Result;
use fontlift_core::{FontManager, FontScope};
use std::path::PathBuf;

fn main() -> Result<()> {
    // Start the gentle logging dance - traces pirouette to stdout
    env_logger::init();

    // Summon the font manager that knows your platform's secrets
    let manager = create_font_manager();

    println!("🌟 FontLift's Gentle Font Adventure Begins");
    println!("==========================================");

    // Find a font file willing to participate in our demonstration
    let demo_font_path = get_demo_font_path();

    // First act: Greet all the fonts that call this system home
    println!("\n📚 Saying hello to all resident fonts:");
    list_fonts(&manager)?;

    // Second act: Welcome a new font to the neighborhood (if they show up)
    if demo_font_path.exists() {
        println!("\n🏡 Welcoming new font friend: {}", demo_font_path.display());
        install_font(&manager, &demo_font_path)?;
        
        // See how the family has grown with our newest member
        println!("\n📚 The font family after our new arrival:");
        list_fonts(&manager)?;

        // Third act: Bid fond farewell as our font guest departs
        println!("\n👋 Waving goodbye to font friend: {}", demo_font_path.display());
        uninstall_font(&manager, &demo_font_path)?;
        
        // Confirm everyone's in their proper place after departure
        println!("\n📚 Font family back to its cozy original state:");
        list_fonts(&manager)?;
    } else {
        println!("\n🔍 No font at the rendezvous point: {}", demo_font_path.display());
        println!("    Place your own font file at the path above to join the fun.");
    }

    // Final act: A gentle spring cleaning for font caches
    println!("\n🧹 Sweeping away dusty font cobwebs:");
    clear_caches(&manager)?;

    println!("\n🎉 Our font adventure concludes with happy endings!");
    
    Ok(())
}

/// Summon the font manager that speaks your platform's language
fn create_font_manager() -> std::sync::Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        println!("🍎 macOS font manager awakens with CoreText magic");
        std::sync::Arc::new(fontlift_platform_mac::MacFontManager::new())
    }
    
    #[cfg(target_os = "windows")]
    {
        println!("🪟 Windows font manager rises through registry mists");
        std::sync::Arc::new(fontlift_platform_win::WinFontManager::new())
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        println!("🐧 Linux dreams of font support while using our friendly placeholder");
        std::sync::Arc::new(fontlift_core::DummyFontManager)
    }
}

/// Gently introduce every font currently living on the system
fn list_fonts(manager: &std::sync::Arc<dyn FontManager>) -> Result<()> {
    match manager.list_installed_fonts() {
        Ok(fonts) => {
            if fonts.is_empty() {
                println!("   The font house seems quiet - no fonts found");
            } else {
                println!("   {} font(s) call this system home:", fonts.len());
                for (i, font) in fonts.iter().enumerate().take(10) {
                    println!("   {}. {} (goes by {})", 
                        i + 1, 
                        font.family_name, 
                        font.postscript_name
                    );
                }
                if fonts.len() > 10 {
                    println!("   ... and {} more font friends hiding in the wings", fonts.len() - 10);
                }
            }
        }
        Err(e) => {
            println!("   🤷‍♀️ The fonts are feeling shy: {}", e);
        }
    }
    
    Ok(())
}

/// Welcome a new font into the system with gentle hospitality
fn install_font(manager: &std::sync::Arc<dyn FontManager>, font_path: &PathBuf) -> Result<()> {
    match manager.install_font(font_path, FontScope::User) {
        Ok(()) => {
            println!("   🎊 Font has found its happy new home");
        }
        Err(e) => {
            println!("   😅 The font trip encountered turbulence: {}", e);
        }
    }
    
    Ok(())
}

/// Guide a font gracefully to its departure from the system
fn uninstall_font(manager: &std::sync::Arc<dyn FontManager>, font_path: &PathBuf) -> Result<()> {
    match manager.uninstall_font(font_path, FontScope::User) {
        Ok(()) => {
            println!("   😊 Font departed with fond memories and clean goodbyes");
        }
        Err(e) => {
            println!("   😿 The font just can't bear to leave: {}", e);
        }
    }
    
    Ok(())
}

/// Gently dust away the digital cobwebs from font caches
fn clear_caches(manager: &std::sync::Arc<dyn FontManager>) -> Result<()> {
    match manager.clear_font_caches(FontScope::User) {
        Ok(()) => {
            println!("   🌤️ Font caches sparkle like fresh morning dew");
        }
        Err(e) => {
            println!("   🧹 The dust bunnies fought back bravely: {}", e);
        }
    }
    
    Ok(())
}

/// Go on a font treasure hunt to find a willing demonstration participant
fn get_demo_font_path() -> PathBuf {
    // Quest for fonts on Apple's sunny operating system
    #[cfg(target_os = "macos")]
    {
        let safari_spots = [
            "/System/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
        ];
        
        for path in &safari_spots {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return path_buf;
            }
        }
    }
    
    // Windows font expedition through the system directory
    #[cfg(target_os = "windows")]
    {
        let windows_hiding_places = [
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\calibri.ttf",
            r"C:\Windows\Fonts\tahoma.ttf",
        ];
        
        for path in &windows_hiding_places {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return path_buf;
            }
        }
    }
    
    // Linux font exploration in the wild
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let linux_font Meadows = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/arial.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        ];
        
        for path in &linux_font_Meadows {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return path_buf;
            }
        }
    }
    
    // When no font volunteers, we have a placeholder waiting in the wings
    PathBuf::from("./demo-font.ttf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager_creation() {
        // Summon our font manager and make sure it shows up for duty
        let _manager = create_font_manager();
    }

    #[test]
    fn test_demo_font_path() {
        // Our font hunter should always return with something, even if empty-handed
        let path = get_demo_font_path();
        // Ensure our treasure map leads somewhere definite
        assert!(!path.as_os_str().is_empty());
    }
}