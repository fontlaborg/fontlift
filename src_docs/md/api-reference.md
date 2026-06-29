# API reference

This page documents the public Rust API in `fontlift-core`: the
[`FontManager`](#the-fontmanager-trait) trait that platform backends implement,
and the [`FontError`](#the-fonterror-type) type every caller must handle. The
authoritative source is the rustdoc in `core/src/lib.rs`; this page is the
narrative version.

## The `FontManager` trait

Every platform backend (`MacFontManager`, `WinFontManager`) implements this
trait. It is `Send + Sync`, so a single manager can be shared across threads
behind an `Arc`.

```rust
pub trait FontManager: Send + Sync {
    fn install_font(&self, source: &FontliftFontSource) -> FontResult<()>;
    fn uninstall_font(&self, source: &FontliftFontSource) -> FontResult<()>;
    fn remove_font(&self, source: &FontliftFontSource) -> FontResult<()>;
    fn is_font_installed(&self, source: &FontliftFontSource) -> FontResult<bool>;
    fn list_installed_fonts(&self) -> FontResult<Vec<FontliftFontFaceInfo>>;
    fn clear_font_caches(&self, scope: FontScope) -> FontResult<()>;
    fn prune_missing_fonts(&self, scope: FontScope) -> FontResult<usize> { Ok(0) }
}
```

| Method | Contract |
|---|---|
| `install_font` | Register the font so applications can use it. The caller may copy the file into an OS font directory first, or register it in place. See the re-installation contract below. |
| `uninstall_font` | Remove the OS registration. The file stays on disk. |
| `remove_font` | Unregister, then delete the file. If unregistration fails, the file is still deleted. |
| `is_font_installed` | Report whether the OS currently knows about this font. |
| `list_installed_fonts` | Enumerate every face the OS knows about, across all scopes. A collection (`.ttc`/`.otc`) yields one entry per face. |
| `clear_font_caches` | Flush the OS font cache for `scope`, plus common app caches (Adobe, Microsoft Office) where practical. |
| `prune_missing_fonts` | Remove registrations whose backing files no longer exist; return the count. Defaults to a no-op. |

### Re-installation contract (`AlreadyInstalled`)

This is the part that surprises people, so it is worth stating plainly:

- **User scope** — re-installing a font that is already present **overwrites**
  the existing file and re-registers it. Calling `install_font` twice succeeds
  both times and leaves the newest bytes in place. It does *not* error.
- **System scope** — if a file with the same name already exists at the target
  directory, `install_font` returns
  [`FontError::AlreadyInstalled`](#the-fonterror-type) rather than clobbering a
  shared, all-users font. Uninstall first, or re-run with `--inplace`.
- **OS-level conflicts** — "already registered" and "duplicate PostScript name"
  responses from the OS font manager are resolved internally (unregister, then
  retry) and never surface as `AlreadyInstalled`.

## The `FontError` type

`FontError` is a `thiserror` enum. Its `Display` output includes a short, arrow-
prefixed suggestion (`→ …`) because callers frequently surface it straight to
users. `FontResult<T>` is the alias `Result<T, FontError>`.

| Variant | Meaning | When you see it |
|---|---|---|
| `FontNotFound(PathBuf)` | The path does not exist. | Wrong path, or the file was moved. |
| `InvalidFormat(String)` | Exists but is not a supported font, or failed structural parsing. | Unsupported extension, corrupt file. |
| `RegistrationFailed(String)` | The OS refused to register the font. | Core Text / GDI rejected the file. |
| `SystemFontProtection(PathBuf)` | You tried to modify an OS-owned location. | A path under `/System/Library/Fonts`, `/Library/Fonts`, or `C:\Windows\Fonts`. |
| `IoError(std::io::Error)` | A filesystem operation failed. | Permissions, disk full, broken pipe. |
| `PermissionDenied(String)` | The process lacks required privileges. | System-scope op without sudo/Administrator. |
| `AlreadyInstalled(PathBuf)` | A same-named file already exists at the destination. | System-scope re-install (see the contract above). |
| `UnsupportedOperation(String)` | Not available on this platform or build. | Linux, or a feature not compiled in. |

## Supporting types

- **`FontScope`** — `User` (current account; `~/Library/Fonts` or `HKCU`) or
  `System` (all users; `/Library/Fonts` or `HKLM`, needs elevation).
- **`FontliftFontSource`** — a pointer to a font file: `path`, optional
  `format`, `face_index` (for collections), `is_collection`, and `scope`. Built
  fluently: `FontliftFontSource::new(path).with_scope(Some(FontScope::User))`.
- **`FontliftFontFaceInfo`** — metadata for one face: `postscript_name` (stable
  identifier), `full_name` (menu display), `family_name`, `style`, and optional
  `weight`/`italic`.

## Minimal usage

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
# Ok::<(), fontlift_core::FontError>(())
```
