import sys
from pathlib import Path

import pytest

from arqstudiocad import paths
from arqstudiocad.check import check_settings


@pytest.fixture
def frozen_in(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Pretend to be a frozen executable living in tmp_path."""
    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(tmp_path / "ArqStudioCAD.exe"))
    return tmp_path


def test_not_portable_from_source() -> None:
    assert paths.portable_dir() is None


def test_frozen_without_marker_is_not_portable(frozen_in: Path) -> None:
    assert paths.portable_dir() is None
    assert check_settings().detail == "system location"


def test_portable_settings_are_written_next_to_the_executable(frozen_in: Path) -> None:
    from arqstudiocad.application import SETTINGS_FILE, open_settings

    (frozen_in / paths.PORTABLE_MARKER).write_text("", encoding="utf-8")
    settings = open_settings()
    settings.setValue("language", "es")
    settings.sync()

    ini = frozen_in / SETTINGS_FILE
    assert Path(settings.fileName()).resolve() == ini.resolve()
    assert "language=es" in ini.read_text(encoding="utf-8")


def test_marker_makes_it_portable(frozen_in: Path) -> None:
    (frozen_in / paths.PORTABLE_MARKER).write_text("", encoding="utf-8")
    assert paths.portable_dir() == frozen_in.resolve()
    result = check_settings()
    assert result.ok
    assert "portable" in result.detail
    assert not (frozen_in / ".write-test").exists()
