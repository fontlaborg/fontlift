"""
fontlift — cross-platform font install/uninstall/list/cleanup for Python.

The heavy lifting is done by the Rust core compiled into `fontlift._native`
(a PyO3 extension). This module re-exports the public API and provides pure-
Python fallbacks so the package imports cleanly even when the native extension
is not built yet.

Platform mechanics (invisible to callers):
- macOS: copies the file to ~/Library/Fonts (user) or /Library/Fonts (system),
  then calls CTFontManagerRegisterFontsForURL. Core Text notifies running apps
  immediately — no reboot needed.
- Windows: copies to %LOCALAPPDATA%\\Microsoft\\Windows\\Fonts (user) or
  C:\\Windows\\Fonts (system), writes a registry entry under
  HKCU/HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts,
  and broadcasts WM_FONTCHANGE via GDI so running apps see the change.

Scope terminology:
- "user" (default): font is visible to the current account only. No admin needed.
- "system": font is visible to all users. Requires sudo / Administrator rights.

Font name fields (on FontFaceInfo / dict output):
- postscript_name: stable programmatic ID used by apps (e.g. "HelveticaNeue-Bold")
- full_name:       menu-friendly name (e.g. "Helvetica Neue Bold")
- family_name:     groups all weights/styles (e.g. "Helvetica Neue")
- style:           variant within the family (e.g. "Bold", "Italic")
"""

from __future__ import annotations

from importlib import import_module
from typing import Any, Dict, List, Mapping

try:
    _native = import_module("fontlift._native")
except ModuleNotFoundError as exc:  # pragma: no cover - exercised via importorskip
    _native = None
    _native_import_error = exc
else:
    _native_import_error = None

if _native:
    FontliftManager = _native.FontliftManager  # re-export
    FontSource = _native.FontSource
    FontFaceInfo = _native.FontFaceInfo  # exposed for structured metadata
else:  # pragma: no cover - importorskip handles runtime use without native module
    FontliftManager = FontSource = FontFaceInfo = None


def _require_native() -> None:
    if _native is None:
        raise ModuleNotFoundError(
            "fontlift._native is not built; run `maturin develop -m crates/fontlift-python/Cargo.toml`",
        ) from _native_import_error


def _font_to_dict(font: Any) -> Dict[str, Any]:
    """Normalise native FontFaceInfo objects into plain dictionaries."""
    if isinstance(font, Mapping):
        return dict(font)

    dict_fn = getattr(font, "dict", None)
    if callable(dict_fn):
        return dict_fn()

    source = getattr(font, "source", None)
    path = getattr(source, "path", getattr(font, "path", None))
    return {
        "source": {
            "path": path,
            "format": getattr(source, "format", None),
            "face_index": getattr(source, "face_index", None),
            "is_collection": getattr(source, "is_collection", None),
            "scope": getattr(source, "scope", None),
        },
        "path": path,
        "postscript_name": getattr(font, "postscript_name", None),
        "full_name": getattr(font, "full_name", None),
        "family_name": getattr(font, "family_name", None),
        "style": getattr(font, "style", None),
        "weight": getattr(font, "weight", None),
        "italic": getattr(font, "italic", None),
        "format": getattr(source, "format", None),
        "scope": getattr(source, "scope", None),
    }


def list_fonts() -> List[Dict[str, Any]]:
    """Return all fonts the OS currently knows about, one dict per face.

    A collection file (.ttc / .otc) produces multiple entries — one per face
    inside the file. Results are not limited to fonts installed by fontlift.

    Each dict has these keys:
      path            – absolute path to the font file
      postscript_name – stable programmatic name (e.g. "Arial-BoldMT")
      full_name       – menu display name (e.g. "Arial Bold")
      family_name     – family group (e.g. "Arial")
      style           – variant within the family (e.g. "Bold")
      weight          – numeric weight 100–900 (None if unknown)
      italic          – True/False (None if unknown)
      format          – file format string (e.g. "TTF", "OTF") or None
      scope           – "user" or "system"
      source          – nested dict with the above source-level fields
    """
    _require_native()
    return [_font_to_dict(font) for font in _native.list()]


list = list_fonts  # alias for CLI parity


def install(font_path: str, admin: bool = False, dry_run: bool = False) -> None:
    """Install a font file so applications can use it.

    Copies the file to the OS font directory for the chosen scope and
    registers it with the OS font manager (Core Text on macOS, GDI +
    Registry on Windows). The font is available to all running applications
    immediately after this call — no reboot required.

    Args:
        font_path: Absolute or relative path to a .ttf, .otf, .ttc, .otc,
                   .woff, or .woff2 file.
        admin:     If True, install system-wide (all users). Requires sudo on
                   macOS or Administrator on Windows. Defaults to user scope.
        dry_run:   If True, return immediately without changing anything.

    Raises:
        RuntimeError: if the file does not exist, is not a valid font, the
                      process lacks the required privileges, or the OS
                      registration call fails.
    """
    if dry_run:
        return
    _require_native()
    _native.install(font_path, admin)


def uninstall(
    font_path: str | None = None,
    *,
    name: str | None = None,
    admin: bool = False,
    dry_run: bool = False,
) -> None:
    """Remove a font's OS registration while keeping the file on disk.

    Pass exactly one of ``font_path`` or ``name``. Name matching checks both
    PostScript name and full name (case-sensitive).

    fontlift tries the preferred scope first, then falls back to the other
    scope if the first attempt fails — useful when you're not certain whether
    a font was installed at user or system level.

    Args:
        font_path: Path to the font file to uninstall.
        name:      PostScript name or full name of the font to uninstall.
        admin:     Prefer system scope first. Without this flag user scope
                   is tried first.
        dry_run:   If True, resolve the target without changing anything.

    Raises:
        RuntimeError: if neither identifier is provided, both are provided,
                      the font is not found, or the OS call fails.
    """
    _require_native()
    _native.uninstall(font_path, name, admin, dry_run)


def remove(
    font_path: str | None = None,
    *,
    name: str | None = None,
    admin: bool = False,
    dry_run: bool = False,
) -> None:
    """Unregister a font and delete its file from disk.

    This is the destructive counterpart to :func:`uninstall`. If
    deregistration fails, fontlift still attempts to delete the file so the
    font is fully gone. Use ``dry_run=True`` first to confirm what will be
    deleted.

    Args:
        font_path: Path to the font file to remove.
        name:      PostScript name or full name of the font to remove.
        admin:     Prefer system scope first.
        dry_run:   If True, resolve the target without deleting anything.

    Raises:
        RuntimeError: same conditions as :func:`uninstall`, plus IO errors
                      when deleting the file.
    """
    _require_native()
    _native.remove(font_path, name, admin, dry_run)


def cleanup(
    admin: bool = False,
    *,
    prune: bool = True,
    cache: bool = True,
    dry_run: bool = False,
) -> None:
    """Prune stale font registrations and/or clear OS font caches.

    Stale registrations point to files that no longer exist — they can
    accumulate when fonts are deleted without going through fontlift. Cache
    clearing asks the OS and common applications (Adobe, Microsoft Office) to
    discard their cached font data so they see the current font set.

    At least one of ``prune`` or ``cache`` must be True.

    Args:
        admin:   Operate on system-wide registrations and caches. Requires
                 sudo / Administrator rights.
        prune:   Remove registrations whose backing files are missing.
        cache:   Clear OS font caches (Core Text on macOS, FontCache service
                 on Windows) and third-party app caches where supported.
        dry_run: If True, return immediately without changing anything.

    Raises:
        RuntimeError: if both ``prune`` and ``cache`` are False, or if an
                      OS cache operation fails.
    """
    _require_native()
    _native.cleanup(admin, prune, cache, dry_run)


__all__ = [
    "FontliftManager",
    "FontSource",
    "FontFaceInfo",
    "list_fonts",
    "list",
    "install",
    "uninstall",
    "remove",
    "cleanup",
]

# Maturin exposes __version__ from the Cargo crate metadata; keep a fallback so
# importorskip consumers degrade gracefully when the native module is missing.
version = getattr(_native, "__version__", None)
__version__ = version
