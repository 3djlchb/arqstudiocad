"""How the interface writes and reads numbers that are not money (ADR 0039).

As in AutoCAD, ProgeCAD and every CAD program: a point for decimals, no
thousands separator and ASCII digits, whatever language the system is in.
On the command line the comma separates a point's coordinates (``10,5``),
so a number shown here can be typed there as it is. Money keeps the
drawing's own format (``arqstudiocad.currencies``, ADR 0028), and a
drawing's printed annotations will have theirs (dimensions, ``DIMDSEP``).
"""

from PySide6.QtCore import QLocale
from PySide6.QtWidgets import QAbstractSpinBox


def number_locale() -> QLocale:
    """The locale numbers are shown and read with: Qt's C locale, which
    neither writes nor accepts a thousands separator."""
    locale = QLocale.c()
    locale.setNumberOptions(
        QLocale.NumberOption.OmitGroupSeparator | QLocale.NumberOption.RejectGroupSeparator
    )
    return locale


def format_number(value: float) -> str:
    """``value`` with up to 12 significant digits: ``2.82842712475``."""
    return number_locale().toString(float(value), "g", 12)


def parse_number(text: str) -> float | None:
    """``text`` read as a number written as :func:`format_number` writes
    it, or None: a decimal comma or a thousands separator is refused."""
    value, valid = number_locale().toDouble(text)
    return value if valid else None


def with_cad_numbers[T: QAbstractSpinBox](box: T) -> T:
    """``box`` showing and reading its value by this module's rule."""
    box.setLocale(number_locale())
    return box
