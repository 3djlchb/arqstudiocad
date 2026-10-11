"""Command-line entry point: `arqstudiocad` or `python -m arqstudiocad`."""

import argparse
import sys
from collections.abc import Sequence

from arqstudiocad import __version__


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="arqstudiocad", description="ArqStudioCAD, open-source CAD."
    )
    parser.add_argument("--version", action="version", version=f"ArqStudioCAD {__version__}")
    parser.add_argument("--check", action="store_true", help="verify the installation and exit")
    parser.add_argument(
        "--lang", metavar="CODE", help="interface language for this session, e.g. en or es"
    )
    args = parser.parse_args(argv)

    if args.check:
        from arqstudiocad.check import main as check_main

        return check_main()

    from arqstudiocad.application import run

    return run(language=args.lang)


if __name__ == "__main__":
    sys.exit(main())
