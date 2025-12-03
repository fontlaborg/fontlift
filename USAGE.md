# FontLift Usage Guide

This guide provides comprehensive usage examples for FontLift, both as a library and as a CLI tool.

## CLI Usage

### Basic Commands

```bash
# List installed fonts (sorted; path-only output is deduped)
fontlift list

# List as JSON (deterministic order, includes path + names)
fontlift list --json

# List with detailed information (use --sorted to dedupe names/paths when combining)
fontlift list --path --name --sorted

# Install one or more fonts for current user
fontlift install /path/to/font.ttf /other/font.otf

# Install every font in a directory (non-recursive)
fontlift install /path/to/font-folder

# Install system-wide (requires admin)
fontlift install /path/to/font.ttf --admin

# Preview what would happen without changing the system
fontlift install /path/to/font.ttf --dry-run

# Quieter or more verbose status output
fontlift install /path/to/font.ttf --quiet
fontlift install /path/to/font.ttf --verbose

# Uninstall a font by name
fontlift uninstall --name "Arial"

# Uninstall by file path or directory
fontlift uninstall /path/to/font.ttf /path/to/font-folder

# Remove font (uninstall + delete)
fontlift remove /path/to/font.ttf /path/to/font-folder

# Clear font caches
fontlift cleanup

# Clear system caches (requires admin)
fontlift cleanup --admin

# Prune stale registrations without clearing caches
fontlift cleanup --prune-only

# Clear caches only (skip pruning)
fontlift cleanup --cache-only

# Generate shell completions (bash|zsh|fish|powershell|elvish)
fontlift completions bash > /usr/local/etc/bash_completion.d/fontlift

# Recover from interrupted operations (crash recovery)
fontlift doctor

# Preview what would be recovered without taking action
fontlift doctor --preview
```

### Font Validation

```bash
# Install with out-of-process validation (default)
fontlift install /path/to/font.ttf

# Skip validation (faster, less safe)
fontlift install /path/to/font.ttf --no-validate

# Use stricter validation
fontlift install /path/to/font.ttf --validation-strictness paranoid
```

## Library Usage

### Basic Font Management

```rust
use fontlift_core::{FontManager, FontScope};
use fontlift_platform_mac::MacFontManager; // or WinFontManager

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = MacFontManager::new();
    
    // Install font
    let font_path = std::path::PathBuf::from("my-font.ttf");
    manager.install_font(&font_path, FontScope::User)?;
    
    // List fonts
    let fonts = manager.list_installed_fonts()?;
    for font in fonts {
        println!("{}: {}", font.family_name, font.style);
    }
    
    Ok(())
}
```

### Font Validation

```rust
use fontlift_core::validation;

fn validate_font(path: &std::path::Path) -> Result<(), fontlift_core::FontError> {
    // Check file extension
    if !validation::is_valid_font_extension(&path.to_path_buf()) {
        return Err(fontlift_core::FontError::InvalidFormat(
            "Invalid font extension".to_string()
        ));
    }
    
    // Validate file contents
    validation::validate_font_file(&path.to_path_buf())?;
    
    // Extract basic information
    let info = validation::extract_basic_info_from_path(&path.to_path_buf());
    println!("Font: {} - {}", info.family_name, info.style);
    
    Ok(())
}
```

### Cross-Platform Manager Creation

```rust
use fontlift_core::FontManager;
use std::sync::Arc;

fn create_font_manager() -> Arc<dyn FontManager> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(fontlift_platform_mac::MacFontManager::new())
    }
    
    #[cfg(target_os = "windows")]
    {
        Arc::new(fontlift_platform_win::WinFontManager::new())
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Arc::new(fontlift_core::DummyFontManager)
    }
}
```

## Python Integration

### Basic Python Usage

```python
import fontlift

# Create manager
manager = fontlift.FontliftManager()

# List fonts
fonts = manager.list_fonts()
for font in fonts:
    print(f"{font['family_name']}: {font['style']}")

# Install font
manager.install_font("my-font.ttf")

# Functional API
fontlift.install("my-font.ttf", admin=False)
fontlift.list()
fontlift.cleanup(admin=False)

# Cleanup with toggles and dry-run support
fontlift.cleanup(prune=True, cache=True, admin=False, dry_run=True)

# Fire CLI mirror with JSON/quiet/verbose/dry-run toggles (matches Rust CLI)
# fontliftpy list --json --path --name --sorted
# fontliftpy install my-font.ttf --dry_run True --quiet True
```

Notes:
- Windows install/remove/cleanup honor `admin` to pick system scope; calls that require elevation will raise `PermissionDenied`.
- macOS supports fake-registry/dry-run paths for tests via `FONTLIFT_FAKE_REGISTRY_ROOT`.

## Error Handling

FontLift provides comprehensive error types:

```rust
use fontlift_core::FontError;

match manager.install_font(&font_path, FontScope::User) {
    Ok(()) => println!("Font installed successfully"),
    Err(FontError::FontNotFound(path)) => {
        println!("Font file not found: {}", path.display());
    },
    Err(FontError::InvalidFormat(msg)) => {
        println!("Invalid font format: {}", msg);
    },
    Err(FontError::PermissionDenied(msg)) => {
        println!("Permission denied: {}", msg);
    },
    Err(e) => println!("Other error: {}", e),
}
```

## Font Formats Supported

- TrueType (.ttf, .ttc)
- OpenType (.otf, .otc)  
- Web Open Font Format (.woff, .woff2)
- macOS dfont (.dfont)

## Security Considerations

- Font files are validated before installation
- System fonts are protected from modification
- Scope-based privilege separation (user vs system)
- Safe path handling and sandboxing

## Performance Tips

- Use batch operations when installing multiple fonts
- Cache font information when listing frequently
- Use appropriate scope (User vs System) for your use case
- Consider font validation costs in performance-critical applications

## Platform-Specific Notes

### macOS

- Uses Core Text APIs for font registration
- Supports user (`~/Library/Fonts`) and system (`/Library/Fonts`) scopes
- Cache clearing via `atsutil` commands

### Windows

- Uses Windows Registry and GDI APIs
- Supports per-user and system-wide font installation
- Registry-based font tracking

### Linux (Not Yet Supported)

- Planned support via fontconfig integration
- Will support standard font directories
