from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Tuple


@dataclass
class ListRenderOptions:
    show_path: bool = False
    show_name: bool = False
    sorted_output: bool = False
    json_output: bool = False


class ListRender:
    def __init__(self, kind: str, payload):
        self.kind = kind
        self.payload = payload

    @property
    def is_json(self) -> bool:
        return self.kind == "json"

    @property
    def lines(self) -> List[str]:
        return self.payload  # type: ignore[return-value]

    @property
    def json(self) -> str:
        return self.payload  # type: ignore[return-value]


def _normalize_path(font: Dict[str, str]) -> str:
    path = font.get("path") or font.get("source", {}).get("path")
    return str(Path(path)) if path is not None else ""


def _dedupe_fonts(fonts: List[Dict[str, str]]) -> List[Dict[str, str]]:
    seen = set()
    unique: List[Dict[str, str]] = []
    for font in fonts:
        key = _normalize_path(font).lower()
        if not key:
            continue
        if key in seen:
            continue
        seen.add(key)
        unique.append(font)
    return unique


def render_list_output(fonts: List[Dict[str, str]], opts: ListRenderOptions) -> ListRender:
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
