"""The test harness must never open the installed application's preferences."""

from collections.abc import Iterator
from pathlib import Path

import pytest
from PySide6.QtCore import QCoreApplication, QSettings

from arqstudiocad.application import open_settings
from arqstudiocad.product import APP_NAME, ORGANIZATION


@pytest.fixture
def settings_identity() -> Iterator[None]:
    """Use the production identity, without leaking it into later Qt tests."""
    previous = (
        QCoreApplication.organizationName(),
        QCoreApplication.applicationName(),
    )
    QCoreApplication.setOrganizationName(ORGANIZATION)
    QCoreApplication.setApplicationName(APP_NAME)
    yield
    QCoreApplication.setOrganizationName(previous[0])
    QCoreApplication.setApplicationName(previous[1])


def test_application_settings_use_a_temporary_ini(tmp_path: Path, settings_identity: None) -> None:
    settings = open_settings()
    # Check containment before writing: even a broken harness must not touch the profile.
    assert settings.format() == QSettings.Format.IniFormat
    assert Path(settings.fileName()).resolve().is_relative_to(tmp_path / "user")
    settings.setValue("isolation/probe", "temporary")
    settings.sync()
    assert settings.status() == QSettings.Status.NoError
    assert open_settings().value("isolation/probe") == "temporary"


def test_system_fallback_is_temporary(tmp_path: Path, settings_identity: None) -> None:
    system = QSettings(
        QSettings.Format.IniFormat, QSettings.Scope.SystemScope, ORGANIZATION, APP_NAME
    )
    assert Path(system.fileName()).resolve().is_relative_to(tmp_path / "system")
    system.setValue("isolation/fallback", "temporary system")
    system.sync()
    assert system.status() == QSettings.Status.NoError
    user = open_settings()
    assert user.format() == QSettings.Format.IniFormat
    assert Path(user.fileName()).resolve().is_relative_to(tmp_path / "user")
    assert user.value("isolation/fallback") == "temporary system"


@pytest.mark.parametrize("repetition", [0, 1])
def test_implicit_settings_do_not_leak_between_tests(
    tmp_path: Path, settings_identity: None, repetition: int
) -> None:
    settings = open_settings()
    assert settings.format() == QSettings.Format.IniFormat
    assert Path(settings.fileName()).resolve().is_relative_to(tmp_path.parent)
    assert settings.allKeys() == []
    settings.setValue("isolation/previous-test", repetition)
    settings.sync()
    assert settings.status() == QSettings.Status.NoError
