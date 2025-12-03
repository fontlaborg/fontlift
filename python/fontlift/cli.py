from __future__ import annotations

import sys
import fire

from . import cleanup, install, list_fonts, remove, uninstall
from .render import ListRenderOptions, render_list_output


def _log_status(message: str, quiet: bool) -> None:
    if not quiet:
        print(message)


def _log_verbose(message: str, quiet: bool, verbose: bool) -> None:
    if verbose and not quiet:
        print(message, file=sys.stderr)


class FontliftCLI:
    """Fire-powered CLI that mirrors the Rust binary surface."""

    def list(
        self,
        *,
        path: bool = False,
        name: bool = False,
        sorted: bool = False,  # noqa: A002
        json: bool = False,  # noqa: A002
    ) -> None:
        fonts = list_fonts()
        opts = ListRenderOptions(
            show_path=path,
            show_name=name,
            sorted_output=sorted,
            json_output=json,
        )
        rendered = render_list_output(fonts, opts)
        if rendered.is_json:
            print(rendered.json)
        else:
            for line in rendered.lines:
                print(line)

    def install(
        self,
        path: str,
        *,
        admin: bool = False,
        dry_run: bool = False,
        quiet: bool = False,
        verbose: bool = False,
    ) -> None:
        if dry_run:
            _log_status(
                f"DRY-RUN: would install font {path} ({'system' if admin else 'user'})",
                quiet,
            )
            return

        _log_verbose(
            f"Installing font at scope: {'system' if admin else 'user'}", quiet, verbose
        )
        install(path, admin=admin, dry_run=dry_run)
        _log_status("✅ Successfully installed font", quiet)

    def uninstall(
        self,
        path: str | None = None,
        *,
        name: str | None = None,
        admin: bool = False,
        dry_run: bool = False,
        quiet: bool = False,
        verbose: bool = False,
    ) -> None:
        target = f"path {path}" if path else f"name {name}"
        if dry_run:
            _log_status(
                f"DRY-RUN: would uninstall font by {target} "
                f"({'system' if admin else 'user'} then {'user' if admin else 'system'})",
                quiet,
            )
            return

        _log_verbose(
            f"Uninstalling font by {target} "
            f"({'system' if admin else 'user'} then {'user' if admin else 'system'})",
            quiet,
            verbose,
        )
        uninstall(path, name=name, admin=admin, dry_run=dry_run)
        _log_status("✅ Successfully uninstalled font", quiet)

    def remove(
        self,
        path: str | None = None,
        *,
        name: str | None = None,
        admin: bool = False,
        dry_run: bool = False,
        quiet: bool = False,
        verbose: bool = False,
    ) -> None:
        target = f"path {path}" if path else f"name {name}"
        if dry_run:
            _log_status(
                f"DRY-RUN: would remove font by {target} "
                f"({'system' if admin else 'user'})",
                quiet,
            )
            return

        _log_verbose(
            f"Removing font by {target} ({'system' if admin else 'user'})",
            quiet,
            verbose,
        )
        remove(path, name=name, admin=admin, dry_run=dry_run)
        _log_status("✅ Successfully removed font", quiet)

    def cleanup(
        self,
        admin: bool = False,
        prune: bool = True,
        cache: bool = True,
        dry_run: bool = False,
        quiet: bool = False,
        verbose: bool = False,
    ) -> None:
        if dry_run:
            planned = []
            if prune:
                planned.append("prune stale registrations")
            if cache:
                planned.append("clear font caches")
            _log_status(
                f"DRY-RUN: would {' and '.join(planned)} "
                f"({'system' if admin else 'user'})",
                quiet,
            )
            return

        scope = "system" if admin else "user"
        _log_verbose(f"Starting {scope} cleanup", quiet, verbose)
        cleanup(admin=admin, prune=prune, cache=cache, dry_run=dry_run)
        _log_status("✅ Cleanup finished", quiet)


def main(argv: list[str] | None = None) -> None:
    argv = argv if argv is not None else sys.argv[1:]
    fire.Fire(FontliftCLI, argv=argv)


if __name__ == "__main__":
    main()
