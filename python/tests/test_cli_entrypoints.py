from __future__ import annotations

from pathlib import Path
import tomllib

from pytest import MonkeyPatch

from fontlift import cli


ROOT = Path(__file__).resolve().parents[2]


def test_pyproject_when_installed_exposes_fontlift_command() -> None:
    pyproject = tomllib.loads((ROOT / "pyproject.toml").read_text())

    scripts = pyproject["project"]["scripts"]

    assert scripts["fontlift"] == "fontlift.cli:main"
    assert scripts["fontliftpy"] == "fontlift.cli:main"


def test_main_when_explicit_argv_passes_fire_command(
    monkeypatch: MonkeyPatch,
) -> None:
    captured: dict[str, object] = {}

    def fake_fire(component: object, **kwargs: object) -> None:
        captured["component"] = component
        captured["kwargs"] = kwargs

    monkeypatch.setattr(cli.fire, "Fire", fake_fire)

    cli.main(["list", "--json"])

    assert captured == {
        "component": cli.FontliftCLI,
        "kwargs": {"command": ["list", "--json"], "name": "fontlift"},
    }
