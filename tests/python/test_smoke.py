import tomllib
from pathlib import Path

import arqstudiocad

ROOT = Path(__file__).resolve().parents[2]


def test_native_module_version_matches_workspace() -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    assert arqstudiocad.__version__ == cargo["workspace"]["package"]["version"]
