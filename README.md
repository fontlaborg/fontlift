# FontLift

A cross-platform font management library and CLI tool written in Rust, consolidating the functionality of the existing Swift and C++ implementations into a unified codebase.

## Overview

FontLift provides:
- **Core Library**: Cross-platform font management abstraction
- **Platform Implementations**: Native macOS and Windows integration  
- **CLI Tool**: Command-line interface for font operations
- **Python Bindings**: PyO3 bindings for Python integration

## Architecture

The project is organized into several crates:

- `fontlift-core`: Core traits, types, and platform-agnostic logic
- `fontlift-platform-mac`: macOS-specific implementation using Core Text
- `fontlift-platform-win`: Windows-specific implementation using Registry APIs
- `fontlift-cli`: Command-line interface built with Clap
- `fontlift-python`: Python bindings using PyO3

## Features

### Font Management
- ✅ Install fonts (user-level and system-level)
- ✅ Uninstall fonts (keeping files)
- ✅ Remove fonts (uninstall and delete)
- ✅ List installed fonts with metadata
- ✅ Clear font caches
- ✅ Cross-platform support (macOS, Windows)

### Font Formats Supported
- TrueType (.ttf, .ttc)
- OpenType (.otf, .otc)  
- Web Open Font Format (.woff, .woff2)
- macOS dfont (.dfont)

### Safety Features
- Validation of font files before installation
- Protection against modifying system fonts
- Proper error handling and reporting
- Scope-based operations (user vs system)

## Quick Start

### As a Rust Library

```rust
use fontlift_core::{FontManager, FontScope};
use fontlift_platform_mac::MacFontManager; // or WinFontManager

let manager = MacFontManager::new();
let font_path = std::path::PathBuf::from("my-font.ttf");

// Install font for current user
manager.install_font(&font_path, FontScope::User)?;

// List installed fonts
let fonts = manager.list_installed_fonts()?;
for font in fonts {
    println!("{}: {}", font.family_name, font.style);
}
```

### CLI Usage

```bash
# Install a font
fontlift install my-font.ttf

# List installed fonts as deterministic JSON
fontlift list --json

# Install multiple fonts or an entire directory (non-recursive)
fontlift install my-font.ttf extras/AnotherFont.otf fonts/

# Preview changes without touching the system
fontlift install my-font.ttf --dry-run --quiet

# Install system-wide (requires admin)
fontlift install my-font.ttf --admin

# List installed fonts
fontlift list --path --name --sorted

# Uninstall a font
fontlift uninstall --name "MyFont"

# Remove a font (uninstall + delete)
fontlift remove my-font.ttf

# Clear font caches
fontlift cleanup

# Clear system caches (requires admin)
fontlift cleanup --admin

# Generate shell completions (bash|zsh|fish|powershell|elvish)
fontlift completions bash > /usr/local/etc/bash_completion.d/fontlift
```

### Python Integration

```python
import fontlift_python

# Create manager
manager = fontlift_python.FontliftManager()

# List fonts
fonts = manager.list_fonts()
for font in fonts:
    print(f"{font['family_name']}: {font['style']}")

# Install font
manager.install_font("my-font.ttf")

# Or use functional API
fontlift_python.install("my-font.ttf", admin=False)
fontlift_python.list()
fontlift_python.cleanup(False)
```

## Platform-Specific Details

### macOS
- Uses Core Text APIs for font registration
- Supports both user (`~/Library/Fonts`) and system (`/Library/Fonts`) scopes
- Cache clearing via `atsutil` commands
- Safe handling of system font protection

### Windows  
- Uses Windows Registry and GDI APIs
- Supports per-user and system-wide font installation
- Registry-based font tracking
- Safe handling of Windows Fonts directory protection

## Building

### Prerequisites
- Rust 1.75+
- Platform-specific build tools:
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio Build Tools

### Build Commands

```bash
# Build all workspace members
cargo build --workspace

# Build release
cargo build --workspace --release

# Run tests
cargo test --workspace

# Run with specific features
cargo build --workspace --features "python"
```

### Platform-Specific Builds

```bash
# Build only current platform
cargo build -p fontlift-core
cargo build -p fontlift-platform-mac  # macOS only
cargo build -p fontlift-platform-win  # Windows only

# Build CLI
cargo build -p fontlift-cli

# Build Python bindings
cargo build -p fontlift-python
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p fontlift-core
cargo test -p fontlift-cli

# Run documentation tests
cargo test --doc

# Run with logging
RUST_LOG=debug cargo test --workspace
```

## Development

### Project Structure
```
fontlift/
├── Cargo.toml              # Workspace configuration
├── README.md
├── crates/
│   ├── fontlift-core/      # Core library
│   ├── fontlift-platform-mac/  # macOS implementation
│   ├── fontlift-platform-win/  # Windows implementation
│   ├── fontlift-cli/       # Command-line interface
│   └── fontlift-python/    # Python bindings
└── docs/
    └── platform-specific.md
```

### Adding New Platforms
1. Create a new crate: `fontlift-platform-{platform}`
2. Implement the `FontManager` trait
3. Add platform-specific dependencies
4. Update workspace and CLI integration
5. Add tests and documentation

### Code Style
- Use `cargo fmt` for formatting
- Use `cargo clippy -- -D warnings` for linting
- Follow Rust API guidelines
- Document all public APIs

## Error Handling

FontLift uses a comprehensive error type system:

- `FontNotFound`: Font file doesn't exist
- `InvalidFormat`: Not a valid font file
- `RegistrationFailed`: Platform registration failed
- `SystemFontProtection`: Attempted to modify system font
- `PermissionDenied`: Insufficient privileges
- `AlreadyInstalled`: Font already exists
- `UnsupportedOperation`: Platform doesn't support operation

## Security Considerations

- Font files are validated before installation
- System fonts are protected from modification
- Scope-based privilege separation
- Safe path handling and sandboxing
- No network operations by default

## Performance

- Minimal memory allocations
- Efficient font metadata extraction
- Lazy loading of platform resources
- Async operations where applicable
- Optimized for bulk operations

## Roadmap

- [ ] Linux platform support (fontconfig integration)
- [ ] Font collection (.ttc/.otc) handling
- [ ] Variable font metadata extraction
- [ ] Font conflict detection and resolution
- [ ] Batch installation/uninstallation
- [ ] Font preview generation
- [ ] GUI application (via testypf integration)

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## License

FontLift is licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and release notes.

---

Made by FontLab https://www.fontlab.com/
