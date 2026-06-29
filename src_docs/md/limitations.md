# What fontlift does NOT do

fontlift has a deliberately narrow scope: register and deregister fonts with the
operating system. The boundaries below are intentional, not missing features.

## It will not touch SIP-protected system fonts

macOS keeps its own fonts in `/System/Library/Fonts/`, guarded by System
Integrity Protection (SIP). Windows keeps `C:\Windows\Fonts\` under similar
ownership. fontlift refuses any operation whose target normalises to one of:

- `/System/Library/Fonts/` (macOS)
- `/Library/Fonts/` (macOS, all-users — writable only with `--admin`/sudo)
- `C:\Windows\Fonts\` (Windows — writable only as Administrator)

Attempting to modify a SIP-owned path returns
[`FontError::SystemFontProtection`](api-reference.md#the-fonterror-type). This is
a guard rail, not a bug: deleting `SFNS.ttf` on macOS or `segoeui.ttf` on Windows
breaks the system UI. If you genuinely need to change a system font, that is the
OS vendor's job, not fontlift's.

## It does not convert or unpack WOFF/WOFF2

`.woff` and `.woff2` are compression wrappers built for the web. fontlift
recognises the extensions and will *pass them to the OS*, but:

- On **macOS**, Core Text may accept a WOFF for the current process, but
  system-wide use is not guaranteed and is not something fontlift promises.
- On **Windows**, GDI does not support WOFF/WOFF2 as installed system fonts at
  all.

fontlift does **not** decompress WOFF to TTF/OTF, and it does not re-pack fonts.
If you need a desktop-installable font from a web font, convert it first with a
dedicated tool, then install the resulting `.ttf`/`.otf`.

## It does not shape, render, or subset fonts

fontlift installs files. It does not lay out text, rasterise glyphs, subset
character sets, or read variable-font axes for rendering. Those are separate
concerns handled by other tools in the FontLab toolchain.

## It does not (yet) support Linux

The CLI is gated to macOS and Windows; building for other targets fails at
compile time by design. See the [Linux roadmap](linux.md) for the planned
`fontconfig` + `fc-cache` approach.

## It is not a font manager UI

There is no GUI, no font preview, no activation sets, no "smart collections".
fontlift is a single-purpose CLI and library. Pair it with a real font manager
if you want those features.
