import pathlib
import sys

REPO_PYTHON_PATH = pathlib.Path(__file__).resolve().parents[2] / "python"
if str(REPO_PYTHON_PATH) not in sys.path:
    sys.path.insert(0, str(REPO_PYTHON_PATH))

from fontlift.render import ListRenderOptions, render_list_output


def sample_fonts():
    return [
        {
            "path": "/Library/Fonts/Alpha.ttf",
            "postscript_name": "Alpha-Regular",
            "family_name": "Alpha",
            "style": "Regular",
        },
        {
            "path": "/Users/user/Fonts/Alpha.ttf",
            "postscript_name": "Alpha-Regular",
            "family_name": "Alpha",
            "style": "Regular",
        },
        {
            "path": "/Library/Fonts/Beta.ttf",
            "postscript_name": "Beta-Bold",
            "family_name": "Beta",
            "style": "Bold",
        },
    ]


def test_render_list_defaults_show_paths_sorted_and_dedupes():
    opts = ListRenderOptions()
    rendered = render_list_output(sample_fonts(), opts)

    assert rendered.lines == [
        "/Library/Fonts/Alpha.ttf",
        "/Library/Fonts/Beta.ttf",
        "/Users/user/Fonts/Alpha.ttf",
    ]


def test_render_list_with_names_only_outputs_names_sorted():
    opts = ListRenderOptions(show_name=True)
    rendered = render_list_output(sample_fonts(), opts)

    assert rendered.lines == ["Alpha-Regular", "Alpha-Regular", "Beta-Bold"]


def test_render_list_json_is_deterministic_and_deduped():
    opts = ListRenderOptions(json_output=True, sorted_output=True)
    rendered = render_list_output(sample_fonts(), opts)

    assert '"path": "/Library/Fonts/Alpha.ttf"' in rendered.json
    assert rendered.json.count("Alpha-Regular") == 2
    assert rendered.json.index("/Library/Fonts/Alpha.ttf") < rendered.json.index(
        "/Users/user/Fonts/Alpha.ttf"
    )
