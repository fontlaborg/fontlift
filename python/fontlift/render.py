"""Font list rendering and formatting utilities.

This module transforms font metadata into user-friendly output formats.
It handles both human-readable text output and machine-readable JSON,
with configurable display options for different use cases.

The renderer focuses on presenting font lists cleanly, handling duplicates,
sorting, and providing flexible formatting choices.

Typical usage:
    fonts = get_installed_fonts()
    options = ListRenderOptions(show_path=True, sorted_output=True)
    result = render_list_output(fonts, options)
    print("\\n".join(result.lines))
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List


@dataclass
class ListRenderOptions:
    """Configuration for font list output formatting.

    These options control how font lists are displayed, allowing
    customization for different use cases from debugging to user interfaces.

    Attributes:
        show_path: Include file paths in output (default: name only)
        show_name: Include font names alongside paths when both are shown
        sorted_output: Sort results alphabetically and remove duplicates
        json_output: Generate machine-readable JSON instead of text lines
    
    Note:
        - When show_path is False, only font names are shown
        - When show_path is True and show_name is False, only paths are shown
        - When both are True, format is "path::name"
        - json_output implies sorting and deduplication
    """
    show_path: bool = False
    show_name: bool = False
    sorted_output: bool = False
    json_output: bool = False


class ListRender:
    """Container for rendered font list output.

    Provides a unified interface for both text and JSON output formats,
    with convenient properties for accessing the data in the appropriate type.

    The payload type depends on the kind:
    - "lines": List[str] - one font per line
    - "json": str - JSON formatted representation
    """
    def __init__(self, kind: str, payload):
        """Initialize with output kind and payload.

        Args:
            kind: Output format type ("lines" or "json")
            payload: Formatted output data
        """
        self.kind = kind
        self.payload = payload

    @property
    def is_json(self) -> bool:
        """Check if this is JSON output."""
        return self.kind == "json"

    @property
    def lines(self) -> List[str]:
        """Get output as list of text lines.

        Raises:
            AssertionError: If this render result is not "lines" type
        """
        assert self.kind == "lines", "Attempted to get lines from JSON output"
        return self.payload  # type: ignore[return-value]

    @property
    def json(self) -> str:
        """Get output as JSON string.

        Raises:
            AssertionError: If this render result is not "json" type
        """
        assert self.kind == "json", "Attempted to get JSON from lines output"
        return self.payload  # type: ignore[return-value]


def _normalize_path(font: Dict[str, str]) -> str:
    """Extract and normalize font file path from font metadata.

    Font data structures can store paths in different locations:
    - Direct "path" field for simple structures
    - Nested "source.path" for more complex data
    
    Normalizes to a canonical path string for comparison.

    Args:
        font: Font metadata dictionary containing path information
        
    Returns:
        Normalized absolute path string, empty string if no path found
    """
    path = font.get("path") or font.get("source", {}).get("path")
    return str(Path(path)) if path is not None else ""


def _dedupe_fonts(fonts: List[Dict[str, str]]) -> List[Dict[str, str]]:
    """Remove duplicate fonts based on file paths (case-insensitive).

    Deduplication is crucial for clean output because:
    - Font systems may register the same font multiple times
    - Collection files (.ttc/.otc) create multiple entries
    - Different fonts can have the same name but different paths

    Uses lowercased normalized paths to handle case differences
    across file systems and operating systems.

    Args:
        fonts: List of font metadata dictionaries to deduplicate
        
    Returns:
        List of unique font entries, preserving first occurrence order
    """
    seen = set()
    unique: List[Dict[str, str]] = []
    for font in fonts:
        key = _normalize_path(font).lower()
        if not key:
            continue  # Skip entries without path information
        if key in seen:
            continue  # Skip duplicate paths
        seen.add(key)
        unique.append(font)
    return unique


def render_list_output(fonts: List[Dict[str, str]], opts: ListRenderOptions) -> ListRender:
    """Render a list of fonts according to specified output options.

    This is the main entry point for font list formatting. It handles:
    - Sorting and deduplication based on options
    - Format selection (lines vs JSON)
    - Flexible display formats (path-only, name-only, or combined)
    - Post-processing to ensure clean, non-redundant output

    The rendering logic follows these priorities:
    1. JSON format when requested (includes automatic sorting/deduping)
    2. Text lines with path/name separation based on show_* flags
    3. Final deduplication for path-only output when sorted

    Args:
        fonts: List of font metadata dictionaries to render
        opts: Configuration options for output formatting
        
    Returns:
        ListRender object containing formatted output in requested format
        
    Example:
        >>> fonts = [{"path": "/System/Library/Arial.ttf", "postscript_name": "ArialMT"}]
        >>> opts = ListRenderOptions(show_path=True, show_name=True)
        >>> result = render_list_output(fonts, opts)
        >>> result.is_json
        False
        >>> result.lines[0]
        '/System/Library/Arial.ttf::ArialMT'
    """
    fonts = list(fonts)
    must_dedupe = opts.sorted_output or opts.json_output
    if must_dedupe:
        fonts = _dedupe_fonts(fonts)

    sort_key = lambda f: (_normalize_path(f).lower(), f.get("postscript_name", "").lower())
    fonts.sort(key=sort_key)

    show_path = opts.show_path or not opts.show_name
    show_name = opts.show_name

    if opts.json_output:
        return ListRender("json", json.dumps(fonts, indent=2))

    lines: List[str] = []
    for font in fonts:
        path = _normalize_path(font)
        ps_name = font.get("postscript_name") or font.get("full_name") or ""

        if show_path and show_name:
            lines.append(f"{path}::{ps_name}")
        elif show_path:
            lines.append(path)
        else:
            lines.append(ps_name)

    lines.sort()
    if (show_path and not show_name) or opts.sorted_output:
        deduped: List[str] = []
        last: str | None = None
        for line in lines:
            if line == last:
                continue
            deduped.append(line)
            last = line
        lines = deduped

    return ListRender("lines", lines)
