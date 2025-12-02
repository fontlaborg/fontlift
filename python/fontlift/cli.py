from __future__ import annotations

import sys
import fire

from . import cleanup, install, list_fonts, remove, uninstall


class FontliftCLI:
    """Fire-powered CLI that mirrors the Rust binary surface."""

    def list(self) -> None:
        fonts = list_fonts()
        for font in fonts:
            print(f"{font['family_name']} - {font['style']} ({font['path']})")

    def install(self, path: str, admin: bool = False) -> None:
        install(path, admin)

    def uninstall(self, path: str, admin: bool = False) -> None:
        uninstall(path, admin)

    def remove(self, path: str, admin: bool = False) -> None:
        remove(path, admin)

    def cleanup(self, admin: bool = False) -> None:
        cleanup(admin)


def main(argv: list[str] | None = None) -> None:
    argv = argv if argv is not None else sys.argv[1:]
    fire.Fire(FontliftCLI, argv=argv)


if __name__ == "__main__":
    main()
