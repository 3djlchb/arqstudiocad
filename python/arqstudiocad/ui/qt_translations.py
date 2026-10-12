"""Translations of Qt's own widgets (standard buttons, dialogs) from Qt's .qm files."""

from PySide6.QtCore import QCoreApplication, QLibraryInfo, QTranslator


class QtTranslations:
    """Keeps at most one Qt base translator installed, matching our language."""

    def __init__(self) -> None:
        self._translator: QTranslator | None = None

    def apply(self, code: str) -> bool:
        """Install Qt's translation for `code`. Returns False when Qt ships none
        (English needs none: Qt's source strings are English)."""
        app = QCoreApplication.instance()
        if app is None:
            return False
        if self._translator is not None:
            app.removeTranslator(self._translator)
            self._translator = None
        translator = QTranslator()
        folder = QLibraryInfo.path(QLibraryInfo.LibraryPath.TranslationsPath)
        if not translator.load(f"qtbase_{code}", folder):
            return False
        app.installTranslator(translator)
        self._translator = translator
        return True
