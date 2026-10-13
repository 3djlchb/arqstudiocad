"""PySide6 presentation layer: windows, viewport, panels and command line.

It shows the application (sessions, commands, events) and never touches the
core directly (contract §1). Where things are:

| Need | Module |
|---|---|
| The window: menus, tabs, docks, quitting | :mod:`~arqstudiocad.ui.main_window` |
| One action per command | :mod:`~arqstudiocad.ui.actions` |
| Events as Qt signals | :mod:`~arqstudiocad.ui.event_bridge` |
| Background open and save | :mod:`~arqstudiocad.ui.background` |
| Dialogs commands ask for | :mod:`~arqstudiocad.ui.qt_services` |
| A drawing on screen | :mod:`~arqstudiocad.ui.viewport` |
| The drawings' tab strip | :mod:`~arqstudiocad.ui.document_tabs` |
| Command line | :mod:`~arqstudiocad.ui.command_window` |
| Layers, Properties, History panels | ``layers_panel``, ``properties_panel``, ``history_panel`` |
| Property grid | :mod:`~arqstudiocad.ui.property_grid` |
| Pins and auto-hidden panels | :mod:`~arqstudiocad.ui.auto_hide` |
| Workspaces: Ribbon and Classic | ``workspace_switcher``, ``ribbon`` |
| Dark and light themes | :mod:`~arqstudiocad.ui.theme` |
| Qt's own translations | :mod:`~arqstudiocad.ui.qt_translations` |

The disciplines' tools are data in :mod:`arqstudiocad.workspaces` (ADR 0015).
"""
