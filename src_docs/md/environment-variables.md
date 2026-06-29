# Environment variables

fontlift reads a handful of environment variables. They fall into three groups:
variables that affect the runtime today, variables reserved for tests, and
variables the configuration module reads but that are **not yet wired** into the
running CLI.

## Active at runtime

| Variable | Effect | Default |
|---|---|---|
| `FONTLIFT_JOURNAL_PATH` | Override the crash-recovery journal location used by `doctor`. | Platform data dir (see below). |
| `RUST_LOG` | Standard `env_logger` filter, e.g. `RUST_LOG=debug` or `RUST_LOG=fontlift_core=trace`. | (unset) |
| `HOME` (macOS) | Resolves `~/Library/Fonts` and the per-user cache locations. | Set by the OS. |

The default journal path:

| Platform | Path |
|---|---|
| macOS | `~/Library/Application Support/FontLift/journal.json` |
| Windows | `%LOCALAPPDATA%\FontLift\journal.json` |
| Linux / other | `~/.local/share/fontlift/journal.json` |

## Test-only

These exist so the test suite never touches real system font directories or
caches. They are wired and effective, but are not part of the supported public
interface.

| Variable | Effect |
|---|---|
| `FONTLIFT_FAKE_REGISTRY_ROOT` | Redirect all install/list/uninstall to a local file tree under this root instead of calling Core Text. The journal also relocates beneath it. |
| `FONTLIFT_TEST_CACHE_ROOT` | Sandbox `clear_font_caches` so it deletes only Adobe/Office cache files beneath this root and skips `atsutil`. |

## Planned (read by the config module, not yet wired)

`core/src/config.rs` defines `FontliftConfig::from_env`, which reads the
variables below. That module is **not currently connected to the CLI runtime**,
so setting these has no effect today. They are documented here so the intended
interface is on record; treat them as a roadmap, not a contract.

| Variable | Intended effect | Planned default |
|---|---|---|
| `FONTLIFT_OVERRIDE_USER_LIBRARY` | Per-user font directory | Platform default |
| `FONTLIFT_OVERRIDE_SYSTEM_LIBRARY` | System-wide font directory | Platform default |
| `FONTLIFT_ADDITIONAL_FONTS` | Extra dirs to scan (`:`-separated) | (none) |
| `FONTLIFT_TEMP_DIR` | Scratch space for in-progress ops | OS temp dir |
| `FONTLIFT_ALLOW_SYSTEM` | Permit writes to system font dirs | `false` |
| `FONTLIFT_REQUIRE_CONFIRMATION` | Prompt before system modifications | `true` |
| `FONTLIFT_DRY_RUN` | Simulate everything, change nothing | `false` |
| `FONTLIFT_MAX_BATCH_SIZE` | Cap on fonts processed in one pass | `1000` |
| `FONTLIFT_LOG_LEVEL` | `trace`/`debug`/`info`/`warn`/`error` | `info` |
| `FONTLIFT_VERBOSE` | Extra human-readable output | `false` |
| `FONTLIFT_JSON` | Machine-readable JSON output | `false` |
| `FONTLIFT_LOG_FILE` | Write logs to a file | (stderr only) |
| `FONTLIFT_ENABLE_CACHE` | Enable the metadata cache | `true` |
| `FONTLIFT_MAX_CACHE_SIZE_MB` | Cache size cap | (built-in) |
| `FONTLIFT_CACHE_TIMEOUT_SECS` | Cache entry lifetime | (built-in) |
| `FONTLIFT_PARALLEL` | Process fonts in parallel | (built-in) |
| `FONTLIFT_MAX_THREADS` | Worker thread cap | (built-in) |

Until the config module is wired in, use the equivalent CLI flags
(`--dry-run`, `--quiet`, `--verbose`, `--json`, `--admin`) instead.
