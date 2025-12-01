//! Basic Usage Example for FontLift
//!
//! This example demonstrates how to use the fontlift library to:
//! - Create a font manager for the current platform
//! - Install and uninstall fonts
//! - List installed fonts
//! - Clear font caches

use anyhow::Result;
use fontlift_core::{FontManager, FontScope};
use std::path::PathBuf;

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Create the appropriate font manager for the current platform
    let manager = create_font_manager();

    println!("🚀 FontLift Basic Usage Example");
    println!("================================");

    // Demo font path (you'll need to provide an actual font file)
    let demo_font_path = get_demo_font_path();

    // Example 1: List installed fonts
    println!("\n📋 Listing installed fonts:");
    list_fonts(&manager)?;

    // Example 2: Install a font (if demo font exists)
    if demo_font_path.exists() {
        println!("\n➕ Installing font: {}", demo_font_path.display());
        install_font(&manager, &demo_font_path)?;
        
        // List fonts again to see the newly installed font
        println!("\n📋 Fonts after installation:");
        list_fonts(&manager)?;

        // Example 3: Uninstall the font
        println!("\n➖ Uninstalling font: {}", demo_font_path.display());
        uninstall_font(&manager, &demo_font_path)?;
        
        // List fonts to confirm uninstallation
        println!("\n📋 Fonts after uninstallation:");
        list_fonts(&manager)?;
    } else {
        println!("\n⚠️  Demo font not found at: {}", demo_font_path.display());
        println!("    Please provide a valid font file path to test installation.");
    }

    // Example 4: Clear font caches (user-level only)
    println!("\n🧹 Clearing user font caches:");
    clear_caches(&manager)?;

    println!("\n✅ Example completed successfully!");
    
    Ok(())
}

/// Create the appropriate font manager for the current platform
fn create_font_manager() -> std::sync::Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        println!("🍎 Using macOS font manager");
        std::sync::Arc::new(fontlift_platform_mac::MacFontManager::new())
    }
    
    #[cfg(target_os = "windows")]
    {
        println!("🪟 Using Windows font manager");
        std::sync::Arc::new(fontlift_platform_win::WinFontManager::new())
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        println!("🐧 Using dummy font manager (Linux not yet implemented)");
        std::sync::Arc::new(fontlift_core::DummyFontManager)
    }
}

/// List all installed fonts
fn list_fonts(manager: &std::sync::Arc<dyn FontManager>) -> Result<()> {
    match manager.list_installed_fonts() {
        Ok(fonts) => {
            if fonts.is_empty() {
                println!("   No fonts found");
            } else {
                println!("   Found {} font(s):", fonts.len());
                for (i, font) in fonts.iter().enumerate().take(10) {
                    println!("   {}. {} ({})", 
                        i + 1, 
                        font.family_name, 
                        font.postscript_name
                    );
                }
                if fonts.len() > 10 {
                    println!("   ... and {} more fonts", fonts.len() - 10);
                }
            }
        }
        Err(e) => {
            println!("   ❌ Failed to list fonts: {}", e);
        }
    }
    
    Ok(())
}

/// Install a font
fn install_font(manager: &std::sync::Arc<dyn FontManager>, font_path: &PathBuf) -> Result<()> {
    match manager.install_font(font_path, FontScope::User) {
        Ok(()) => {
            println!("   ✅ Font installed successfully");
        }
        Err(e) => {
            println!("   ❌ Failed to install font: {}", e);
        }
    }
    
    Ok(())
}

/// Uninstall a font
fn uninstall_font(manager: &std::sync::Arc<dyn FontManager>, font_path: &PathBuf) -> Result<()> {
    match manager.uninstall_font(font_path, FontScope::User) {
        Ok(()) => {
            println!("   ✅ Font uninstalled successfully");
        }
        Err(e) => {
            println!("   ❌ Failed to uninstall font: {}", e);
        }
    }
    
    Ok(())
}

/// Clear font caches
fn clear_caches(manager: &std::sync::Arc<dyn FontManager>) -> Result<()> {
    match manager.clear_font_caches(FontScope::User) {
        Ok(()) => {
            println!("   ✅ Font caches cleared successfully");
        }
        Err(e) => {
            println!("   ❌ Failed to clear font caches: {}", e);
        }
    }
    
    Ok(())
}

/// Get a demo font path for testing
fn get_demo_font_path() -> PathBuf {
    // Try to find a common system font for demonstration
    #[cfg(target_os = "macos")]
    {
        let common_paths = [
            "/System/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
        ];
        
        for path in &common_paths {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return path_buf;
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        let common_paths = [
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\calibri.ttf",
            r"C:\Windows\Fonts\tahoma.ttf",
        ];
        
        for path in &common_paths {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return path_buf;
            }
        }
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Common Linux font paths
        let common_paths = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/arial.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        ];
        
        for path in &common_paths {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return path_buf;
            }
        }
    }
    
    // Fallback to a custom path the user can modify
    PathBuf::from("./demo-font.ttf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager_creation() {
        let _manager = create_font_manager();
    }

    #[test]
    fn test_demo_font_path() {
        let path = get_demo_font_path();
        // Just ensure we get a valid PathBuf
        assert!(!path.as_os_str().is_empty());
    }
}