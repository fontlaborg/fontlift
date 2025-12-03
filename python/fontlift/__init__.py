"""
Python package exposing the FontLift PyO3 extension.
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
    _require_native()
    return [_font_to_dict(font) for font in _native.list()]


list = list_fonts  # alias for CLI parity


def install(font_path: str, admin: bool = False, dry_run: bool = False) -> None:
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
    _require_native()
    _native.uninstall(font_path, name, admin, dry_run)


def remove(
    font_path: str | None = None,
    *,
    name: str | None = None,
    admin: bool = False,
    dry_run: bool = False,
) -> None:
    _require_native()
    _native.remove(font_path, name, admin, dry_run)


def cleanup(
    admin: bool = False,
    *,
    prune: bool = True,
    cache: bool = True,
    dry_run: bool = False,
) -> None:
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
