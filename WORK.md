## 2025-12-01
- Added `fontlift completions <shell>` subcommand powered by `clap_complete`, emitting scripts to stdout for bash/zsh/fish/powershell/elvish.
- New test `completions_include_core_commands` to ensure generated scripts mention core commands.
- Ran `cargo test --workspace` (pass).
- Added CLI aliases (`l/i/u/rm/c`) and mapped clap parse errors to legacy exit codes (0 for help/version, 1 otherwise).
- Added tests for alias coverage and exit-code mapping; `cargo test --workspace` passes.
