"""Options > User preferences > Zoom (ADR 0029 step 5): the wheel reversed
(AutoCAD's ZOOMWHEEL) and the zoom of one notch in percent, 5 to 100,
default 25 (the step it had before)."""

from dataclasses import replace
from pathlib import Path

import pytest
from PySide6.QtCore import QPoint, QPointF, QSettings, Qt
from PySide6.QtGui import QWheelEvent
from PySide6.QtWidgets import QApplication
from pytestqt.qtbot import QtBot
from ui_fakes import QuietUi

from arqstudiocad.events import OptionsChanged
from arqstudiocad.i18n import set_language
from arqstudiocad.ui.main_window import MainWindow
from arqstudiocad.ui.options_dialog import (
    DEFAULTS,
    STORED,
    Options,
    OptionsDialog,
    save_options,
    saved_options,
)
from arqstudiocad.ui.viewport import WHEEL_PERCENT, WHEEL_PERCENTS, WHEEL_STEP, Viewport


def notch(viewport: Viewport, notches: int = 1) -> None:
    """The wheel turned forward ``notches`` notches over the center."""
    at = QPointF(viewport.width() / 2, viewport.height() / 2)
    event = QWheelEvent(
        at,
        viewport.mapToGlobal(at),
        QPoint(),
        QPoint(0, 120 * notches),
        Qt.MouseButton.NoButton,
        Qt.KeyboardModifier.NoModifier,
        Qt.ScrollPhase.NoScrollPhase,
        False,
    )
    QApplication.sendEvent(viewport, event)


def test_the_defaults_are_todays_wheel() -> None:
    assert (WHEEL_PERCENT, WHEEL_PERCENTS) == (25, (5, 100))
    assert WHEEL_STEP == 1.25
    assert DEFAULTS.zoom_factor == 25 and DEFAULTS.zoom_wheel_reversed is False


@pytest.fixture
def window(qtbot: QtBot, tmp_path: Path) -> MainWindow:
    settings = QSettings(str(tmp_path / "w.ini"), QSettings.Format.IniFormat)
    w = MainWindow(settings, ui=QuietUi())
    qtbot.addWidget(w)
    w.resize(900, 600)
    w.show()
    qtbot.waitExposed(w)
    return w


def test_each_notch_zooms_by_the_factor(window: MainWindow) -> None:
    viewport = window.viewports[window.session.new_document().id]
    assert viewport.wheel == (WHEEL_STEP, False)
    scale = viewport.view.scale
    notch(viewport)
    assert viewport.view.scale == pytest.approx(scale * 1.25), "forward zooms in"
    window.apply_options(replace(window.options_values(), zoom_factor=60))
    assert viewport.wheel == (1.6, False)
    scale = viewport.view.scale
    notch(viewport)
    assert viewport.view.scale == pytest.approx(scale * 1.6)
    scale = viewport.view.scale
    notch(viewport, -2)
    assert viewport.view.scale == pytest.approx(scale / 1.6**2), "backward zooms out"


def test_the_wheel_can_be_reversed(window: MainWindow) -> None:
    viewport = window.viewports[window.session.new_document().id]
    window.apply_options(replace(window.options_values(), zoom_wheel_reversed=True))
    assert viewport.wheel == (WHEEL_STEP, True)
    scale = viewport.view.scale
    notch(viewport)
    assert viewport.view.scale == pytest.approx(scale / 1.25), "forward zooms out"
    scale = viewport.view.scale
    notch(viewport, -1)
    assert viewport.view.scale == pytest.approx(scale * 1.25)


def test_the_window_announces_keeps_and_reapplies_it(window: MainWindow, qtbot: QtBot) -> None:
    settings = window._settings
    assert settings is not None
    announced: list[OptionsChanged] = []
    window.bus.subscribe(OptionsChanged, announced.append)
    window.apply_options(replace(window.options_values(), zoom_factor=40, zoom_wheel_reversed=True))
    assert announced == [OptionsChanged(("zoom_wheel_reversed", "zoom_factor"))]
    later = window.session.new_document()
    assert window.viewports[later.id].wheel == (1.4, True), "a viewport opened later"
    settings.sync()
    again = MainWindow(settings, ui=QuietUi())
    qtbot.addWidget(again)
    values = again.options_values()
    assert (values.zoom_factor, values.zoom_wheel_reversed) == (40, True)
    assert again.viewports[again.session.new_document().id].wheel == (1.4, True)


@pytest.mark.parametrize(
    ("name", "saved"),
    [
        ("zoom_factor", "4"),
        ("zoom_factor", "101"),
        ("zoom_factor", "fast"),
        ("zoom_wheel_reversed", "yes"),
        ("zoom_wheel_reversed", "1"),
    ],
)
def test_a_value_this_version_does_not_take_reads_as_the_default(
    tmp_path: Path, name: str, saved: str
) -> None:
    settings = QSettings(str(tmp_path / "o.ini"), QSettings.Format.IniFormat)
    settings.setValue(STORED[name], saved)
    assert getattr(saved_options(settings), name) == getattr(DEFAULTS, name)


def test_the_bounds_and_both_answers_are_taken(tmp_path: Path) -> None:
    path = str(tmp_path / "o.ini")
    for factor, reverse in ((5, True), (100, False)):
        settings = QSettings(path, QSettings.Format.IniFormat)
        values = Options(zoom_factor=factor, zoom_wheel_reversed=reverse)
        save_options(settings, values, ("zoom_factor", "zoom_wheel_reversed"))
        settings.sync()
        read = saved_options(QSettings(path, QSettings.Format.IniFormat))
        assert (read.zoom_factor, read.zoom_wheel_reversed) == (factor, reverse)


def dialog(qtbot: QtBot, values: Options = DEFAULTS) -> tuple[OptionsDialog, list[Options]]:
    applied: list[Options] = []
    d = OptionsDialog(values, applied.append)
    qtbot.addWidget(d)
    d.sidebar.setCurrentRow(2)
    return d, applied


def test_user_preferences_holds_the_zoom(qtbot: QtBot) -> None:
    d, applied = dialog(qtbot)
    assert d.section() == "user_preferences"
    assert d.zoom_group.title() == "Zoom"
    assert d.zoom_wheel_reversed.text() == "Reverse wheel zoom (ZOOMWHEEL)"
    assert not d.zoom_wheel_reversed.isChecked()
    assert d.zoom_factor_label.text() == "Zoom factor" and d.zoom_unit.text() == "% per notch"
    assert (d.zoom_spin.minimum(), d.zoom_spin.maximum()) == (5, 100)
    assert (d.zoom_slider.minimum(), d.zoom_slider.maximum()) == (5, 100)
    assert d.zoom_spin.value() == d.zoom_slider.value() == 25
    assert d.zoom_spin.accessibleName() == "Zoom factor"
    assert not d.zoom_wheel_reversed.font().bold(), "plain, under the bold title"
    # Above the right-click group, as the brief shows.
    page = d.pages.currentWidget()
    assert page is not None
    assert (
        d.zoom_group.mapTo(page, QPoint(0, 0)).y()
        < d.right_click_group.mapTo(page, QPoint(0, 0)).y()
    )
    d.zoom_slider.setValue(60)
    assert d.zoom_spin.value() == 60 and d.pending.zoom_factor == 60
    d.zoom_spin.setValue(70)
    assert d.zoom_slider.value() == 70
    d.zoom_wheel_reversed.setChecked(True)
    assert d.apply_button.isEnabled()
    d.ok_button.click()
    assert applied == [Options(zoom_factor=70, zoom_wheel_reversed=True)]


def test_the_dialog_shows_and_resets_the_zoom(qtbot: QtBot) -> None:
    d, _ = dialog(qtbot, Options(right_click="repeat", zoom_factor=40, zoom_wheel_reversed=True))
    assert d.zoom_spin.value() == 40 and d.zoom_wheel_reversed.isChecked()
    assert not d.apply_button.isEnabled()
    d.reset_button.click()
    assert d.pending == DEFAULTS, "the whole section, the right-click too"
    assert d.zoom_spin.value() == 25 and not d.zoom_wheel_reversed.isChecked()


def test_the_zoom_in_spanish(qtbot: QtBot) -> None:
    try:
        set_language("es")
        d, _ = dialog(qtbot)
        assert d.zoom_wheel_reversed.text() == (
            "Invertir el zoom de la rueda del ratón (ZOOMWHEEL)"
        )
        assert d.zoom_factor_label.text() == "Factor de zoom"
        assert d.zoom_unit.text() == "% por paso"
    finally:
        set_language("en")
