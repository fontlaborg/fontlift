import importlib

import pytest


@pytest.fixture(scope="module")
def fontlift_modules():
    native = pytest.importorskip(
        "fontlift._native",
        reason="fontlift._native not built; run `maturin develop -m crates/fontlift-python/Cargo.toml` first",
    )
    fontlift = importlib.import_module("fontlift")
    return fontlift, native


def test_version_matches_native(fontlift_modules) -> None:
    fontlift, native = fontlift_modules
    assert fontlift.__version__ == native.__version__


def test_cleanup_dry_run_is_noop(fontlift_modules) -> None:
    fontlift, _native = fontlift_modules
    assert fontlift.cleanup(dry_run=True) is None
