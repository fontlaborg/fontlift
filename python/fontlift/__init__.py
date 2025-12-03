"""
Python package exposing the FontLift PyO3 extension.
"""

from __future__ import annotations

from importlib import import_module
from typing import Any, Dict, List

_native = import_module("fontlift._native")

FontliftManager = _native.FontliftManager  # re-export
FontSource = _native.FontSource
FontFaceInfo = _native.FontFaceInfo  # exposed for structured metadata


def list_fonts() -> List[Dict[str, Any]]:
    return _native.list()

list = list_fonts  # alias for CLI parity

def install(font_path: str, admin: bool = False, dry_run: bool = False) -> None:
    if dry_run:
        return
    _native.install(font_path, admin)


def uninstall(
    font_path: str | None = None,
    *,
    name: str | None = None,
    admin: bool = False,
    dry_run: bool = False,
) -> None:
    _native.uninstall(font_path, name, admin, dry_run)


def remove(
    font_path: str | None = None,
    *,
    name: str | None = None,
    admin: bool = False,
    dry_run: bool = False,
) -> None:
    _native.remove(font_path, name, admin, dry_run)


def cleanup(
    admin: bool = False,
    *,
    prune: bool = True,
    cache: bool = True,
    dry_run: bool = False,
) -> None:
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

# Hatch-vcs will write the version from the git tag; fall back to the Rust
# crate version if the dynamic metadata isn't injected (e.g., editable builds).
version = getattr(_native, "__version__", None)
__version__ = version
