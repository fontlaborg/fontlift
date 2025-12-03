# Changelog

## Unreleased
- CLI list output is now always sorted; path-only output is deduplicated by default and `--sorted` now focuses on deduping combined name/path views.
- Uninstall by name now checks both user and system scopes and removes the font from whichever scope succeeds (subject to permissions).
- Fix Python wheel build by depending on the published `hatchling-pyo3-plugin` instead of the non-existent `hatchling-pyo3`; update `publish.sh` accordingly.
- macOS listing now uses Core Text font descriptors to return PostScript/family/style/format metadata with scope tagging; `FontliftFontFaceInfo` carries optional scope for Rust/Python consumers.
- macOS cleanup now prunes stale Core Text registrations and clears Adobe/Microsoft caches; CLI gains `--prune-only` and `--cache-only` toggles for targeted cleanup.
- Core API renamed to `FontliftFontSource`/`FontliftFontFaceInfo`; CLI and Python bindings now emit structured sources (path/format/face_index/scope) alongside face metadata.
