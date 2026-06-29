# Documentation style guide

Conventions for fontlift's prose — these docs, the README, and rustdoc comments.
The goal is one voice across the project: precise, plain, and short.

## Voice

- **Plain language.** Use "use", not "utilize"; "before", not "prior to". If a
  reader has to read a sentence twice, rewrite it once.
- **Active voice, strong verbs.** "Core Text registers the font" beats "the font
  is registered by Core Text".
- **No hype.** Skip "revolutionary", "blazingly fast", "seamless". State what it
  does and let the reader judge.
- **Lead with the outcome.** Say what the reader accomplishes before how.
- **Trust the reader.** Don't re-explain what a code sample already shows.

## Structure

- One `#` H1 per page, matching the nav title.
- Short sections. If a section runs past a screen, split it.
- Prefer tables for any "name → meaning → default" mapping. Every API surface
  (errors, env vars, flags, trait methods) is documented as a table.
- Code blocks are tagged with a language (` ```rust `, ` ```sh `, ` ```json `).
- Cross-link with relative `.md` paths so links work both on GitHub and in the
  built site.

## Domain terms (use consistently)

- **font file** — a file on disk (`.ttf`, `.otf`, `.ttc`, …).
- **face** — one styled instance inside a file. A collection holds several.
- **PostScript name** — the stable programmatic identifier (`HelveticaNeue-Bold`).
  Not the filename, not the family name.
- **scope** — `user` (current account) or `system` (all users).
- **register / unregister** — telling the OS about a font, or removing that
  record. Distinct from copying or deleting the file.

## Rust doc comments

- `//!` module headers explain *what the module is for* and *why it exists*,
  especially non-obvious design choices (e.g. why the validator runs
  out-of-process).
- `///` item docs state the contract: preconditions, what is returned, and which
  `FontError` variants can come back.
- When behaviour differs by platform or scope, say so explicitly — the
  `AlreadyInstalled` re-installation contract is the canonical example.
- Keep the arrow-suggestion convention in `FontError` `Display` strings:
  `error text\n→ what to do about it`.

## Error messages are UX

Write them like directions, not diagnostics. Every user-facing error names the
problem and the next step. Follow the existing `→` suggestion pattern rather than
inventing a new one.

## Building these docs

The source lives in `src_docs/md/`. Build the site with MkDocs Material:

```sh
mkdocs build -f src_docs/mkdocs.yaml   # output → docs/
mkdocs serve  -f src_docs/mkdocs.yaml   # live preview at http://127.0.0.1:8000
```

Edit the Markdown in `src_docs/md/`, never the generated `docs/` output.
