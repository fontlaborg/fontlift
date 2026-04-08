//! Basic fontlift usage — install, list, uninstall, clear caches.
//!
//! Run with: `cargo run --example basic_usage`
//!
//! This example walks through the four core operations:
//!
//! 1. **List** every font the OS knows about
//! 2. **Install** a font file so applications can use it
//! 3. **Uninstall** the font (deregister it, but keep the file)
//! 4. **Clear caches** so apps pick up the changes immediately
//!
//! It picks a demo font from well-known system paths. If none exist
//! (unlikely unless you're on a very stripped-down install), it falls
//! back to `./demo-font.ttf` in the current directory.

use anyhow::Result;
use fontlift_core::{FontManager, FontScope};
use std::path::PathBuf;

fn main() -> Result<()> {
    // Enable env_logger so you can see debug output with RUST_LOG=debug
    env_logger::init();

    let manager = create_font_manager();

    println!("fontlift example: install → list → uninstall → cache clear");
    println!("===========================================================");

    let demo_font_path = get_demo_font_path();

    // Step 1: List what's already installed
    println!("\n— Installed fonts:");
    list_fonts(&manager)?;

    // Step 2: Install a font (if we found one to demo with)
    if demo_font_path.exists() {
        println!("\n— Installing: {}", demo_font_path.display());
        install_font(&manager, &demo_font_path)?;

        println!("\n— Fonts after install:");
        list_fonts(&manager)?;

        // Step 3: Uninstall it again
        println!("\n— Uninstalling: {}", demo_font_path.display());
        uninstall_font(&manager, &demo_font_path)?;

        println!("\n— Fonts after uninstall:");
        list_fonts(&manager)?;
    } else {
        println!("\n— No demo font found at: {}", demo_font_path.display());
        println!("  Drop a .ttf or .otf file there to see install/uninstall in action.");
    }

    // Step 4: Clear font caches
    println!("\n— Clearing font caches:");
    clear_caches(&manager)?;

    println!("\nDone.");

    Ok(())
}

/// Create the right font manager for whatever OS we're running on.
///
/// macOS gets a Core Text–backed manager. Windows gets Registry/GDI.
/// Anything else gets a dummy that returns "unsupported" for every call.
fn create_font_manager() -> std::sync::Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        println!("  (macOS — using Core Text)");
        std::sync::Arc::new(fontlift_platform_mac::MacFontManager::new())
    }

    #[cfg(target_os = "windows")]
    {
        println!("  (Windows — using Registry + GDI)");
        std::sync::Arc::new(fontlift_platform_win::WinFontManager::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        println!("  (Unsupported platform — using dummy manager)");
        std::sync::Arc::new(fontlift_core::DummyFontManager)
    }
}

/// Print up to 10 installed fonts, showing family name and PostScript name.
fn list_fonts(manager: &std::sync::Arc<dyn FontManager>) -> Result<()> {
    match manager.list_installed_fonts() {
        Ok(fonts) => {
            if fonts.is_empty() {
                println!("  (none found)");
            } else {
                println!("  {} font(s) installed:", fonts.len());
                for (i, font) in fonts.iter().enumerate().take(10) {
                    // PostScript name is the unique internal ID apps use;
                    // family_name is the human-readable group name.
                    println!("  {}. {} (PostScript: {})",
                        i + 1,
                        font.family_name,
                        font.postscript_name
                    );
                }
                if fonts.len() > 10 {
                    println!("  ... and {} more", fonts.len() - 10);
                }
            }
        }
        Err(e) => {
            println!("  Error listing fonts: {}", e);
        }
    }

    Ok(())
}

/// Install a font at user scope (no admin needed).
fn install_font(manager: &std::sync::Arc<dyn FontManager>, font_path: &PathBuf) -> Result<()> {
    match manager.install_font(font_path, FontScope::User) {
        Ok(()) => println!("  Installed."),
        Err(e) => println!("  Install failed: {}", e),
    }

    Ok(())
}

/// Uninstall a font — deregisters it but doesn't delete the file.
fn uninstall_font(manager: &std::sync::Arc<dyn FontManager>, font_path: &PathBuf) -> Result<()> {
    match manager.uninstall_font(font_path, FontScope::User) {
        Ok(()) => println!("  Uninstalled."),
        Err(e) => println!("  Uninstall failed: {}", e),
    }

    Ok(())
}

/// Flush font caches so apps re-read the fonts directory.
fn clear_caches(manager: &std::sync::Arc<dyn FontManager>) -> Result<()> {
    match manager.clear_font_caches(FontScope::User) {
        Ok(()) => println!("  Caches cleared."),
        Err(e) => println!("  Cache clear failed: {}", e),
    }

    Ok(())
}

/// Find a font file to use for the demo.
///
/// Searches well-known system font paths on each platform.
/// Falls back to `./demo-font.ttf` if nothing is found.
fn get_demo_font_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        // macOS system fonts — these are always present
        let candidates = [
            "/System/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
        ];

        for path in &candidates {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows ships these fonts with every install
        let candidates = [
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\calibri.ttf",
            r"C:\Windows\Fonts\tahoma.ttf",
        ];

        for path in &candidates {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // Common Linux font paths
        let candidates = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/arial.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        ];

        for path in &candidates {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }
    }

    // Fallback: let the caller handle the "file not found" case
    PathBuf::from("./demo-font.ttf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_manager_creates_without_panic() {
        let _manager = create_font_manager();
    }

    #[test]
    fn demo_font_path_is_non_empty() {
        let path = get_demo_font_path();
        assert!(!path.as_os_str().is_empty());
    }
}
