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

    def install(self, path: str, admin: bool = False, dry_run: bool = False) -> None:
        install(path, admin=admin, dry_run=dry_run)

    def uninstall(
        self,
        path: str | None = None,
        *,
        name: str | None = None,
        admin: bool = False,
        dry_run: bool = False,
    ) -> None:
        uninstall(path, name=name, admin=admin, dry_run=dry_run)

    def remove(
        self,
        path: str | None = None,
        *,
        name: str | None = None,
        admin: bool = False,
        dry_run: bool = False,
    ) -> None:
        remove(path, name=name, admin=admin, dry_run=dry_run)

    def cleanup(
        self,
        admin: bool = False,
        prune: bool = True,
        cache: bool = True,
        dry_run: bool = False,
    ) -> None:
        cleanup(admin=admin, prune=prune, cache=cache, dry_run=dry_run)


def main(argv: list[str] | None = None) -> None:
    argv = argv if argv is not None else sys.argv[1:]
    fire.Fire(FontliftCLI, argv=argv)


if __name__ == "__main__":
    main()
