import pathlib
import sys

import pytest

# Ensure the in-repo Python package is imported ahead of any globally installed copy
REPO_PYTHON_PATH = pathlib.Path(__file__).resolve().parents[2] / "python"
if str(REPO_PYTHON_PATH) not in sys.path:
    sys.path.insert(0, str(REPO_PYTHON_PATH))


def pytest_collection_modifyitems(config, items):
    """
    Skip tests marked with `needs_native` when the extension module is absent.
    """
    try:
        import fontlift._native  # noqa: F401

        native_available = True
    except Exception:
        native_available = False

    if native_available:
        return

    skip = pytest.mark.skip(
        reason="fontlift._native not built; run `maturin develop -m crates/fontlift-python/Cargo.toml` first"
    )
    for item in items:
        if "needs_native" in item.keywords:
            item.add_marker(skip)
