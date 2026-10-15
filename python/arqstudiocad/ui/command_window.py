"""The command window, docked at the bottom as in AutoCAD and IngeCAD
(ADR 0013, D5.4).

A read-only history of prompts, answers and messages above one input line
showing the current prompt. Enter sends the line, Space does too except
while a command asks for text, Esc cancels, Up/Down recall earlier lines,
and command names complete as they are typed.
"""

from PySide6.QtCore import QEvent, QObject, QStringListModel, Qt
from PySide6.QtGui import QFontDatabase, QKeyEvent
from PySide6.QtWidgets import (
    QApplication,
    QCompleter,
    QDockWidget,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPlainTextEdit,
    QVBoxLayout,
    QWidget,
)

from arqstudiocad.commands import CommandRunner
from arqstudiocad.events import CommandCancelled, CommandFinished, CommandMessage, CommandPrompt
from arqstudiocad.i18n import tr
from arqstudiocad.ui.event_bridge import QtEventBridge

#: Lines kept in the history.
HISTORY_LINES = 1000
#: Typed lines that Up/Down can recall.
RECALL_LINES = 100


class CommandWindow(QDockWidget):
    """History, prompt and input line of the command runner."""

    def __init__(
        self, runner: CommandRunner, events: QtEventBridge, parent: QWidget | None = None
    ) -> None:
        super().__init__(parent)
        self.setObjectName("command_window")
        self._runner = runner
        self._recall: list[str] = []
        self._recall_index = 0

        self.history = QPlainTextEdit()
        self.history.setReadOnly(True)
        self.history.setMaximumBlockCount(HISTORY_LINES)
        self.prompt = QLabel()
        self.input = QLineEdit()
        mono = QFontDatabase.systemFont(QFontDatabase.SystemFont.FixedFont)
        for widget in (self.history, self.prompt, self.input):
            widget.setFont(mono)
        self.input.installEventFilter(self)
        self.input.returnPressed.connect(self._send)

        self._completer = QCompleter(runner.registry.names(), self)
        self._completer.setCaseSensitivity(Qt.CaseSensitivity.CaseInsensitive)
        self._completer.setCompletionMode(QCompleter.CompletionMode.InlineCompletion)
        self.input.setCompleter(self._completer)

        line = QHBoxLayout()
        line.setContentsMargins(0, 0, 0, 0)
        line.addWidget(self.prompt)
        line.addWidget(self.input, 1)
        body = QWidget()
        layout = QVBoxLayout(body)
        layout.setContentsMargins(2, 2, 2, 2)
        layout.addWidget(self.history, 1)
        layout.addLayout(line)
        self.setWidget(body)

        events.command_prompt.connect(self._on_prompt)
        events.command_message.connect(self._on_message)
        events.command_finished.connect(self._on_end)
        events.command_cancelled.connect(self._on_end)
        self.retranslate()

    def retranslate(self) -> None:
        self.setWindowTitle(tr("Command line"))
        # The language may add command names of its own (ADR 0016 D9).
        model = self._completer.model()
        assert isinstance(model, QStringListModel)
        model.setStringList(self._runner.registry.names())
        if self._runner.active is None:
            self.prompt.setText(tr("Command:"))

    def escape(self) -> None:
        """Esc, wherever it is pressed: as AutoCAD, it cancels the running
        command, drops what was typed and empties the selection (ADR 0016
        D3)."""
        self.input.clear()
        self._runner.cancel()
        session = self._runner.context.session
        if session.active is not None:
            session.clear_selection(session.active)

    def type_key(self, event: QKeyEvent) -> None:
        """A key typed elsewhere (the viewport): the command line takes it,
        shown if it was closed, as AutoCAD's does."""
        self.show()
        self.input.setFocus()
        QApplication.sendEvent(self.input, event)

    def _send(self) -> None:
        text = self.input.text()
        self.input.clear()
        self.history.appendPlainText(f"{self.prompt.text()} {text}".rstrip())
        if text.strip():
            self._recall.append(text)
            del self._recall[:-RECALL_LINES]
        self._recall_index = len(self._recall)
        self._runner.execute(text)

    def _on_prompt(self, event: CommandPrompt) -> None:
        self.prompt.setText(event.text)
        # Answers are not command names. Qt takes None (no completer); the
        # PySide6 stubs do not say so.
        self.input.setCompleter(None)  # type: ignore[arg-type]

    def _on_message(self, event: CommandMessage) -> None:
        self.history.appendPlainText(event.text)

    def _on_end(self, event: CommandFinished | CommandCancelled) -> None:
        if self._runner.active is None:
            self.prompt.setText(tr("Command:"))
            self.input.setCompleter(self._completer)

    def eventFilter(self, watched: QObject, event: QEvent) -> bool:  # noqa: N802 (Qt API)
        if watched is self.input and event.type() == QEvent.Type.KeyPress:
            assert isinstance(event, QKeyEvent)
            key = event.key()
            if key == Qt.Key.Key_Escape:
                self.escape()
                return True
            if key == Qt.Key.Key_Space and not self._typing_text():
                self._send()
                return True
            if key in (Qt.Key.Key_Up, Qt.Key.Key_Down) and self._recall:
                step = -1 if key == Qt.Key.Key_Up else 1
                self._recall_index = max(0, min(len(self._recall), self._recall_index + step))
                recalled = self._recall[self._recall_index : self._recall_index + 1]
                self.input.setText(recalled[0] if recalled else "")
                return True
        return super().eventFilter(watched, event)

    def _typing_text(self) -> bool:
        """Whether the running command asks for free text, where Space is a
        character (a file name), not Enter."""
        prompt = self._runner.prompt
        return prompt is not None and prompt.kind == "text"
