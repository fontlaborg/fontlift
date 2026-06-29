# Linux roadmap (stub)

Linux support is planned but not implemented. The CLI currently `compile_error!`s
on non-macOS, non-Windows targets rather than silently degrading. This page
records the intended design so contributors know what "done" looks like before
writing any code.

> Status: **design intent only.** Nothing here ships yet.

## The mechanism: fontconfig + fc-cache

Unlike macOS (Core Text) and Windows (registry + GDI), Linux has no single
system API to "register" a font. Instead, applications discover fonts through
**fontconfig**, which scans a known set of directories and keeps an on-disk
cache. Installing a font is therefore:

1. **Copy** the font file into a directory fontconfig watches:
   - user scope: `~/.local/share/fonts/` (XDG; preferred) or `~/.fonts/` (legacy)
   - system scope: `/usr/share/fonts/` or `/usr/local/share/fonts/` (needs root)
2. **Rebuild the cache** so applications pick it up:
   - `fc-cache -f <dir>` (force) for the affected directory
   - user scope needs no privileges; system scope needs root

Uninstall reverses this: remove the file from the fonts directory, then re-run
`fc-cache -f`.

## Mapping to the `FontManager` trait

| Trait method | Planned Linux implementation |
|---|---|
| `install_font` | Copy into the scope's fonts dir, then `fc-cache -f`. |
| `uninstall_font` | Remove the registration by deleting the file from the fonts dir (Linux has no registration separate from the file), then `fc-cache -f`. |
| `remove_font` | Same as uninstall on Linux — the file *is* the registration. |
| `is_font_installed` | Query `fc-list` (or libfontconfig) for the path / family. |
| `list_installed_fonts` | Parse `fc-list` output, or bind libfontconfig via a crate such as `fontconfig`. |
| `clear_font_caches` | `fc-cache -rf` (rebuild everything). |
| `prune_missing_fonts` | fontconfig already ignores missing files; a forced `fc-cache` rebuild suffices. |

## Open questions before implementation

- **Binding vs. shelling out.** Call the `fc-*` binaries (zero extra deps, but
  fragile parsing) or link `libfontconfig` via a crate (richer, adds a C dep)?
  The macOS/Windows backends call native APIs directly; a libfontconfig binding
  is more consistent, but shelling out to `fc-cache` is simpler for a first cut.
- **Scope semantics.** Linux distros vary in their system font dirs. The
  `FONTLIFT_OVERRIDE_*` planned variables (see
  [Environment variables](environment-variables.md)) should cover the
  differences once that config module is wired in.
- **No journal-visible "register" step.** Because the file *is* the
  registration, the crash-recovery journal collapses to copy/delete plus a
  cache rebuild. The `RegisterFont`/`UnregisterFont` journal actions become
  no-ops or cache rebuilds on Linux.
- **WOFF/WOFF2.** fontconfig support depends on the build; do not assume it.
  See [What fontlift does NOT do](limitations.md).

## Acceptance criteria

A Linux backend is complete when it implements `FontManager`, copies into the
correct XDG directories, drives `fc-cache` for user and system scopes, lists via
fontconfig, and passes the existing fake-registry-style integration tests
adapted to a temp `XDG_DATA_HOME`.
