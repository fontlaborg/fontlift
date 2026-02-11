# FontLift Release Checklist (2025-12-03)

## Preconditions
- Working tree clean; all CI jobs green.
- Versions bumped in `Cargo.toml` and `pyproject.toml`; changelog updated.
- Confirm `README.md`, `USAGE.md`, and `FEATURE_MATRIX.md` match the release surface.

## Rust crates (workspace)
- Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test --workspace --exclude fontlift-python`.
- For each publishable crate, run `cargo publish --dry-run` to catch packaging issues before uploading. citeturn0search0
- If using CI-based publishing, ensure crates.io Trusted Publishing is configured after an initial manual publish so GitHub Actions can mint short-lived tokens via OIDC. citeturn0search1turn0search4
- Optional: rehearse tagging/version bumps with `cargo release --dry-run`; switch to `--execute` when ready. citeturn0search6
- Publish crates in dependency order (core → platform crates → CLI → Python helper crates if any).

## Python wheels
- Ensure the Python extension builds locally: `maturin develop -m crates/fontlift-python/Cargo.toml --features python-bindings`.
- Build and upload release wheels with `maturin publish -m crates/fontlift-python/Cargo.toml --features python-bindings` (uses the same command to build and push to PyPI).
- If using CI, prefer trusted secretless publishing; otherwise supply a short-lived PyPI token.

## GitHub release
- Tag the commit (`vX.Y.Z`) after publishes succeed; push tags.
- Draft a GitHub release that links the changelog section, attaches prebuilt binaries/wheels if produced, and notes platform caveats (Windows validation pending).

## Post-release
- Yank or deprecate superseded versions if needed.
- Update `CHANGELOG.md`, `TASKS.md`, `TODO.md`, and `WORK.md` with release status.
- Announce internally; capture any follow-up bugs in `TODO.md`.
