# fontlift TODO

**Scope:** Consolidate `fontlift-mac-cli` and `fontlift-win-cli` into a single Rust project that delivers a reusable library, a cross-platform CLI, and Python bindings (with Fire CLI) for installing/uninstalling fonts, listing them, and clearing caches. Requirements sourced from `/Users/adam/Developer/vcs/TODO2.md` (Nov 16, 2025).

## Phase 0 – Alignment
- [ ] Read existing docs/code in `../fontlift-mac-cli` (Swift) and `../fontlift-win-cli` (C#/PowerShell)
- [ ] Summarize platform feature parity (operations, privilege requirements, UX) in `docs/feature-matrix.md`
- [ ] Define success metrics (command coverage, idempotency, performance, safety)
- [ ] Write `PLAN.md` capturing architecture + roadmap (core crate + platform adapters + CLIs)

## Phase 1 – Architectural Foundations
- [ ] Design crate layout (`fontlift-core`, `fontlift-platform-{mac,win,linux}`, `fontlift-cli`, `fontlift-python`)
- [ ] Specify platform abstraction traits (install/uninstall/list/cache_clean, privilege escalation strategy)
- [ ] Choose dependency stack (windows-rs, core-foundation/core-text bindings, crossbeam for parallel IO, clap/typer for CLIs)
- [ ] Define config file/env var story (paths, scopes, logging)
- [ ] Document security considerations (sandboxing, user-level vs system-level installs)

## Phase 2 – macOS Implementation
- [ ] Port install/uninstall logic from Swift CLI (handles .otf/.ttf/.dfont, collections)
- [ ] Support both `~/Library/Fonts` (user) and `/Library/Fonts` (system) with privilege detection/escalation instructions
- [ ] Implement cache cleaning (ATS cache + fontd restart) safely
- [ ] Add listing command (source path, postscript name, activation status)
- [ ] Write integration tests using temporary font dirs + mocked cache commands

## Phase 3 – Windows Implementation
- [ ] Port install/uninstall logic from win CLI (registry + `C:\Windows\Fonts` + per-user fonts)
- [ ] Handle font cache clearing (font cache service restart + file deletion) with rollback
- [ ] Implement listing with metadata (family, style, location, scope)
- [ ] Ensure code runs from non-admin context by default, escalate only when required
- [ ] Add integration tests using temp dirs + mocked registry/service APIs (windows-rs + test harness)

## Phase 4 – Future Linux Support
- [ ] Research font install locations for major distros (`/usr/share/fonts`, `~/.local/share/fonts`)
- [ ] Outline permissions + cache refresh commands (fc-cache, etc.) and capture as TODO entries

## Phase 5 – CLI & Python Surface
- [ ] Build `fontlift-cli` (clap) mirroring feature set with consistent exit codes + JSON output option
- [ ] Provide `fontlift-python` PyO3 bindings plus Fire CLI mirroring Rust CLI options
- [ ] Document install instructions (maturin/uv) and add CLI smoke tests (pytest + invoke)

## Phase 6 – Testing, Docs, Release
- [ ] Create cross-platform test plan (unit, integration, system tests) and automate where possible
- [ ] Document manual verification matrix per OS/version
- [ ] Update `README.md` with workflows, risk mitigations, troubleshooting
- [ ] Maintain `CHANGELOG.md` + `WORK.md` per repo standards
- [ ] Prepare release artifacts (cargo install, PyPI wheel, binaries) + publish instructions

**Dependencies:** Shared code from typf (font discovery) may be reused where helpful; coordinate with typg/testypf for integration touch points.
