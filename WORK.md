## 2025-12-03

### Test Suite Status

**13 macOS integration tests + 15 unit tests pass.**
**5 Python tests pass.**
**Clippy clean, fmt clean.**

---

### Latest Changes

- Swapped the pyproject build backend to `maturin` with explicit manifest/python-source settings to silence pep517 warnings; rebuilt via `maturin develop -m crates/fontlift-python/Cargo.toml` and reran `pytest python/tests` (5 passing).
- Added `help_text_includes_all_commands` test verifying all 7 commands appear in CLI help
- Added `shell_completions_generate_for_all_shells` test for Bash/Zsh/Fish/PowerShell/Elvish
- Added `fontlift doctor` and validation docs to USAGE.md
- Added `validation_strictness_presets_parse` test for lenient/normal/paranoid CLI flags
- Added `no_validate_flag_parses` test for `--no-validate` flag
- Fixed CI workflow: changed Python build from `maturin develop` to `uv pip install -e .`
