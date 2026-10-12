import os
from collections.abc import Iterator

import pytest

# Qt tests run without a display, on every OS.
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from arqstudiocad.i18n import SOURCE_LANGUAGE, set_language


@pytest.fixture(autouse=True)
def _english_by_default() -> Iterator[None]:
    """Each test starts and ends in English; the active language is global state."""
    set_language(SOURCE_LANGUAGE)
    yield
    set_language(SOURCE_LANGUAGE)
