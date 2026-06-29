# fontlift

Install, uninstall, list, and clean up fonts on macOS and Windows — from the
command line, from Rust, or from Python.

Made by FontLab https://www.fontlab.com/

---

## What fontlift does

A font is not just a file. To make a font usable by applications, the OS must
be told about it: on macOS that means a Core Text registration, on Windows a
registry entry plus a GDI call. fontlift handles that mechanics so you don't
have to.

| Operation | What it does |
|---|---|
| `install` | Copy the font file to the OS font directory, register it. Apps see it immediately. |
| `uninstall` | Remove the OS registration. File stays on disk. |
| `remove` | Unregister **and** delete the file. |
| `list` | Enumerate every face the OS currently knows about. |
| `cleanup` | Prune stale registrations + clear font caches. |
| `doctor` | Find interrupted operations and resume them. |

---

## Platform support

| Platform | Status | Install location (user) | Install location (system) | Admin needed for system? |
|---|---|---|---|---|
| macOS 12+ | Full | `~/Library/Fonts/` | `/Library/Fonts/` | sudo |
| Windows 10 1809+ | Full | `%LOCALAPPDATA%\Microsoft\Windows\Fonts\` | `C:\Windows\Fonts\` | Administrator |
| Windows 7–10 pre-1809 | System scope only | — | `C:\Windows\Fonts\` | Administrator |
| Linux | Planned | `~/.local/share/fonts/` | `/usr/share/fonts/` | — |

**macOS detail:** Core Text picks up the font immediately after installation —
no reboot, no log-out. `/System/Library/Fonts/` is managed by macOS and
protected by SIP; fontlift never touches it.

**Windows detail:** Installation writes both a registry entry under
`HKCU\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts` (user scope) or
`HKLM\...` (system scope) and calls `AddFontResourceW` + `WM_FONTCHANGE` so
running apps can use the font right away. Without the registry entry the font
disappears after the next reboot.

---

## Font formats

| Extension | Format | Notes |
|---|---|---|
| `.ttf` | TrueType | Single face. Most common format. |
| `.otf` | OpenType | Single face. PostScript or TrueType outlines. |
| `.ttc` / `.otc` | Collection | Multiple faces in one file (e.g. CJK families). |
| `.woff` / `.woff2` | Web Open Font | Compressed for the web; system support varies. |
| `.dfont` | Mac data-fork suitcase | Legacy macOS format. |

---

## CLI quick start

```sh
# Install a font for the current user (no admin needed)
fontlift install MyFont.otf

# Install an entire directory of fonts
fontlift install ~/Downloads/InterFamily/

# Install system-wide for all users (requires sudo / admin)
fontlift install --admin MyFont.otf

# List all installed fonts (one path per line, sorted, deduped)
fontlift list
fontlift list --name          # PostScript names instead of paths
fontlift list --path --name   # path::PostScriptName pairs
fontlift list --json          # machine-readable JSON

# Uninstall (keeps the file on disk)
fontlift uninstall ~/Library/Fonts/MyFont.otf
fontlift uninstall --name HelveticaNeue-Bold

# Remove (uninstall + delete the file)
fontlift remove ~/Library/Fonts/OldFont.otf
fontlift remove --name OldFont-Regular

# Prune stale registrations and clear caches
fontlift cleanup
fontlift cleanup --prune-only   # registrations only
fontlift cleanup --cache-only   # caches only
fontlift cleanup --admin        # include system scope

# Preview any operation without changing anything
fontlift --dry-run install MyFont.otf

# Check for and recover interrupted operations
fontlift doctor
fontlift doctor --preview

# Shell completions
fontlift completions bash >> ~/.bashrc
fontlift completions zsh  > ~/.zsh/completions/_fontlift
fontlift completions fish > ~/.config/fish/completions/fontlift.fish
```

Global flags work with every subcommand:

| Flag | Effect |
|---|---|
| `--dry-run` | Print what would happen, change nothing |
| `--quiet` / `-q` | Suppress all non-error output |
| `--verbose` / `-v` | Show resolved paths, scope choices, extra detail |
| `--json` / `-j` | Machine-readable JSON output |

---

## Validation

Before installing, fontlift can run an out-of-process validator to catch
malformed font files. The validator runs in a separate process so a corrupt
font cannot crash fontlift itself.

```sh
fontlift install MyFont.ttf                                 # normal (64 MB, 5 s)
fontlift install --validation-strictness lenient Big.ttf    # 128 MB, 10 s
fontlift install --validation-strictness paranoid Untrusted.ttf  # 32 MB, 2 s
fontlift install --no-validate QuickTest.ttf                # skip entirely
```

---

## Recovering interrupted operations

Install and remove are multi-step (copy, then register; unregister, then
delete). If `fontlift` is killed midway — a crash, a `Ctrl-C`, a lost SSH
session — a crash-recovery journal records what was planned and how far it got.
`doctor` reads that journal and resumes or tidies up the unfinished work.

```sh
# See what was left unfinished, without changing anything
fontlift doctor --preview

# Resume / roll forward the interrupted operations
fontlift doctor
```

Example output after an install was interrupted between copy and registration:

```text
$ fontlift doctor
Checking for interrupted operations...
Found 1 interrupted operation(s)

Operation 7f3c… (started …):
  Description: Install /Users/me/Downloads/Inter-Bold.otf
  Progress: step 1 of 2
  [2] Register /Users/me/Library/Fonts/Inter-Bold.otf (User)

Attempting recovery...
✅ Successfully recovered 1 action(s)
```

---

## What fontlift does NOT do

These boundaries are intentional, not missing features:

- **It never touches SIP-protected system fonts.** Anything under
  `/System/Library/Fonts/` (macOS) or `C:\Windows\Fonts\` (Windows) is off
  limits; such operations return `SystemFontProtection`. Deleting `SFNS.ttf` or
  `segoeui.ttf` would break the system UI.
- **It does not convert or unpack WOFF/WOFF2.** Those are web-only compression
  wrappers. fontlift recognises the extensions and hands them to the OS, but
  Windows GDI rejects them as system fonts and macOS support is not guaranteed.
  Convert WOFF to `.ttf`/`.otf` with a dedicated tool first.
- **It does not shape, render, or subset fonts.** fontlift installs files; it
  does not lay out text or rasterise glyphs.
- **No Linux support yet.** The CLI is macOS/Windows only; see the
  [Linux roadmap](src_docs/md/linux.md).

See [`src_docs/md/limitations.md`](src_docs/md/limitations.md) for the full list.

---

## Python

```python
import fontlift

# One-shot helpers
fontlift.install("MyFont.ttf")                    # user scope, no admin
fontlift.install("MyFont.ttf", admin=True)        # system scope
fontlift.uninstall("MyFont.ttf")
fontlift.uninstall(name="HelveticaNeue-Bold")     # by PostScript name
fontlift.remove("OldFont.ttf")
fontlift.cleanup(prune=True, cache=True)
fontlift.cleanup(admin=True)                      # system scope

# List all installed fonts
for font in fontlift.list_fonts():
    # font is a dict; key fields:
    # postscript_name, full_name, family_name, style, path, scope
    print(f"{font['family_name']} {font['style']}  →  {font['path']}")

# Reusable manager (one platform connection, multiple operations)
mgr = fontlift.FontliftManager()
mgr.install_font("/tmp/MyFont.ttf")
faces = mgr.list_fonts()
mgr.cleanup(prune=True, cache=True)
```

The Python package is a thin wrapper around the `fontlift._native` PyO3
extension. Build it with `maturin develop` (development) or
`maturin build --release` (wheel for distribution).

---

## Rust library

```rust
use fontlift_core::{FontManager, FontScope, FontliftFontSource};
use fontlift_platform_mac::MacFontManager; // or WinFontManager on Windows

let manager = MacFontManager::new();
let source = FontliftFontSource::new("MyFont.ttf".into())
    .with_scope(Some(FontScope::User));

manager.install_font(&source)?;

for face in manager.list_installed_fonts()? {
    println!("{}: {} {}", face.source.path.display(), face.family_name, face.style);
}
```

---

## Crate layout

```
fontlift/
├── core/            fontlift-core       types, traits, validation, journal
├── platform-mac/    fontlift-platform-mac   Core Text implementation
├── platform-win/    fontlift-platform-win   Registry + GDI implementation
├── cli/             fontlift-cli        clap-based CLI
├── python/          fontlift-python     PyO3 bindings
└── validator/       fontlift-validator  out-of-process font parser helper
```

`fontlift-core` defines `FontManager`, `FontError`, `FontScope`, and the shared
data types. Platform crates implement `FontManager` with real OS calls. The CLI
and Python bindings delegate to whichever platform crate is compiled in.

---

## Building

```sh
# Prerequisites: Rust 1.75+, platform SDK (Xcode CLT on macOS, VS Build Tools on Windows)

# Full workspace build + tests
./build.sh --test

# Rust only
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings

# Python wheel (requires maturin)
maturin develop -m python/Cargo.toml          # editable install for dev
maturin build  -m python/Cargo.toml --release # distributable wheel → dist/
```

---

## Environment variables

| Variable | Effect | Default |
|---|---|---|
| `FONTLIFT_OVERRIDE_USER_LIBRARY` | Override per-user font directory | Platform default |
| `FONTLIFT_OVERRIDE_SYSTEM_LIBRARY` | Override system font directory | Platform default |
| `FONTLIFT_DRY_RUN` | Simulate all operations | `false` |
| `FONTLIFT_ALLOW_SYSTEM` | Permit system-scope writes | `false` |
| `FONTLIFT_LOG_LEVEL` | `trace`/`debug`/`info`/`warn`/`error` | `info` |
| `FONTLIFT_JOURNAL_PATH` | Override crash-recovery journal location | Platform default |
| `RUST_LOG` | Standard `env_logger` filter | — |

---

## Error types

| Error | Meaning |
|---|---|
| `FontNotFound` | File does not exist at the given path |
| `InvalidFormat` | File exists but is not a supported font format |
| `RegistrationFailed` | OS refused to register the font |
| `SystemFontProtection` | Path is in a system-managed font directory |
| `PermissionDenied` | Process lacks the required privileges |
| `AlreadyInstalled` | A font with that path is already registered |
| `UnsupportedOperation` | Feature not available on this platform |

---

## Roadmap

- [ ] Linux support (fontconfig + `fc-cache`)
- [ ] Variable font metadata extraction
- [ ] GUI via testypf integration

---

## Documentation

Full documentation source lives in [`src_docs/md/`](src_docs/md/) and builds to a
[MkDocs Material](https://squidfunk.github.io/mkdocs-material/) site under
`docs/`:

```sh
mkdocs build -f src_docs/mkdocs.yaml   # output → docs/
mkdocs serve -f src_docs/mkdocs.yaml   # live preview
```

Key pages: [API reference](src_docs/md/api-reference.md),
[environment variables](src_docs/md/environment-variables.md),
[what fontlift does NOT do](src_docs/md/limitations.md),
[Linux roadmap](src_docs/md/linux.md).

---

## License

Apache License 2.0 — see [LICENSE](LICENSE).

See [CHANGELOG.md](CHANGELOG.md) for version history.
