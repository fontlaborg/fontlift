# Changelog

## Unreleased
- build.sh now runs maturin against the `crates/fontlift-python` manifest so Python binding builds no longer fail with a workspace `missing field package` error.
- Core now exposes conflict detection (path/PostScript/family+style) and Windows installs auto-unregister/remove duplicate registry/file entries before copying, while refusing to overwrite protected system font paths.
- Windows cleanup now also purges Adobe font cache manifests (`AdobeFnt*.lst`) under common Program Files Adobe TypeSupport/TypeSpt/PDFL roots to mirror legacy cache clearing.
- Windows cleanup now prunes missing registry font entries and clears the FontCache service (stop → delete cache files/FNTCACHE.DAT → start), improving parity with the legacy C++ CLI; user-scope cleanup now explicitly requires `--admin` on Windows.
- macOS installs now copy fonts into the target scope directory and auto-resolve Core Text "already registered"/duplicate-name errors by unregistering and retrying the registration path; reinstalling in fake/user scopes replaces the installed file to keep upgrades deterministic.
- `is_font_installed` now checks both the scope target path and Core Text registered URLs, and fake-registry tests are serialized to avoid environment races.
- CLI list output is now always sorted; path-only output is deduplicated by default and `--sorted` now focuses on deduping combined name/path views.
- Uninstall by name now checks both user and system scopes and removes the font from whichever scope succeeds (subject to permissions).
- Fix Python wheel build by depending on the published `hatchling-pyo3-plugin` instead of the non-existent `hatchling-pyo3`; update `publish.sh` accordingly.
- macOS listing now uses Core Text font descriptors to return PostScript/family/style/format metadata with scope tagging; `FontliftFontFaceInfo` carries optional scope for Rust/Python consumers.
- macOS cleanup now prunes stale Core Text registrations and clears Adobe/Microsoft caches; CLI gains `--prune-only` and `--cache-only` toggles for targeted cleanup.
- Core API renamed to `FontliftFontSource`/`FontliftFontFaceInfo`; CLI and Python bindings now emit structured sources (path/format/face_index/scope) alongside face metadata.
- Python bindings add name-based uninstall/remove with dry-run support and expose cleanup flag controls through the Fire CLI wrappers.
