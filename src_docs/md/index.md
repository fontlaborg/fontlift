# fontlift

Install, uninstall, list, and clean up fonts on macOS and Windows — from the
command line, from Rust, or from Python.

Made by FontLab <https://www.fontlab.com/>

---

A font is not just a file. To make a font usable by applications, the OS must be
told about it: on macOS that means a Core Text registration, on Windows a
registry entry plus a GDI call. fontlift handles that paperwork so you don't
have to.

## What it does

| Operation | Effect |
|---|---|
| `install` | Copy the font into the OS font directory and register it. Apps see it immediately. |
| `uninstall` | Remove the OS registration. The file stays on disk. |
| `remove` | Unregister **and** delete the file. |
| `list` | Enumerate every face the OS currently knows about. |
| `cleanup` | Prune stale registrations and clear font caches. |
| `doctor` | Find interrupted operations and resume them. |

## Where to go next

- **[API reference](api-reference.md)** — the `FontManager` trait and the
  `FontError` type, the contract platform backends implement and callers handle.
- **[Environment variables](environment-variables.md)** — the knobs that change
  fontlift's behaviour, and which ones are wired versus planned.
- **[What fontlift does NOT do](limitations.md)** — SIP-protected paths, WOFF,
  and other deliberate non-goals.
- **[Linux roadmap](linux.md)** — the planned `fontconfig` + `fc-cache` design.
- **[Documentation style guide](style-guide.md)** — conventions for these docs.

## Crate layout

```
fontlift/
├── core/            fontlift-core            types, traits, validation, journal
├── platform-mac/    fontlift-platform-mac    Core Text implementation
├── platform-win/    fontlift-platform-win    Registry + GDI implementation
├── cli/             fontlift-cli             clap-based CLI
├── python/          fontlift-python          PyO3 bindings
└── validator/       fontlift-validator       out-of-process font parser helper
```

`fontlift-core` defines `FontManager`, `FontError`, `FontScope`, and the shared
data types. Platform crates implement `FontManager` with real OS calls. The CLI
and Python bindings delegate to whichever platform crate is compiled in.
