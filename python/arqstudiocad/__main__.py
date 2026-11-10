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
    backends = ("dx12", "vulkan", "opengl", "metal", "software")
    parser.add_argument("--gpu-probe", choices=backends, help=argparse.SUPPRESS)
    parser.add_argument("--gpu-adapters", choices=backends, help=argparse.SUPPRESS)
    parser.add_argument("--adapter", type=int, help=argparse.SUPPRESS)
    parser.add_argument(
        "--lang", metavar="CODE", help="interface language for this session, e.g. en or es"
    )
    parser.add_argument(
        "--safe-mode",
        action="store_true",
        help="draw on the CPU for this session, whatever the graphics card",
    )
    args = parser.parse_args(argv)

    if args.check:
        from arqstudiocad.check import main as check_main

        return check_main()

    if args.gpu_adapters:
        from arqstudiocad import _core

        try:
            listed = _core.list_gpu_adapters(args.gpu_adapters)
        except Exception as exc:
            print(json.dumps({"backend": args.gpu_adapters, "error": str(exc)}))
            return 1
        adapters = [
            {"name": name, "device_type": kind, "vendor": vendor, "device": device}
            for name, kind, vendor, device in listed
        ]
        print(json.dumps({"backend": args.gpu_adapters, "adapters": adapters}))
        return 0

    if args.gpu_probe:
        from arqstudiocad import _core

        try:
            if args.adapter is None:
                backend, adapter, device_type = _core.probe_gpu_backend(args.gpu_probe)
            else:
                backend, adapter, device_type = _core.probe_gpu_adapter(
                    args.gpu_probe, args.adapter
                )
        except Exception as exc:
            print(json.dumps({"backend": args.gpu_probe, "error": str(exc)}))
            return 1
        print(json.dumps({"backend": backend, "adapter": adapter, "device_type": device_type}))
        return 0

    from arqstudiocad.application import run

    return run(language=args.lang, safe_mode=args.safe_mode)


if __name__ == "__main__":
    sys.exit(main())
