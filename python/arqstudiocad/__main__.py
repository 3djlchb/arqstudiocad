"""Command-line entry point: `arqstudiocad` or `python -m arqstudiocad`."""

import argparse
import json
import sys
from collections.abc import Sequence

from arqstudiocad import __version__
from arqstudiocad.product import APP_NAME


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="arqstudiocad", description=f"{APP_NAME}, open-source CAD."
    )
    parser.add_argument("--version", action="version", version=f"{APP_NAME} {__version__}")
    parser.add_argument("--check", action="store_true", help="verify the installation and exit")
    parser.add_argument(
        "--gpu-probe",
        choices=("dx12", "vulkan", "opengl", "metal", "software"),
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--lang", metavar="CODE", help="interface language for this session, e.g. en or es"
    )
    args = parser.parse_args(argv)

    if args.check:
        from arqstudiocad.check import main as check_main

        return check_main()

    if args.gpu_probe:
        from arqstudiocad import _core

        try:
            backend, adapter, device_type = _core.probe_gpu_backend(args.gpu_probe)
        except Exception as exc:
            print(json.dumps({"backend": args.gpu_probe, "error": str(exc)}))
            return 1
        print(json.dumps({"backend": backend, "adapter": adapter, "device_type": device_type}))
        return 0

    from arqstudiocad.application import run

    return run(language=args.lang)


if __name__ == "__main__":
    sys.exit(main())
