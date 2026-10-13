# 0015. Workspaces: a discipline's tools in a layout

Date: 2026-10-13 · Status: Accepted · Settles open question 3 of
[0014](0014-platform-direction.md) · Builds on [0013](0013-application-shell.md) D5.1

## Context

ADR 0013 gave the shell two layouts over one set of actions: **Classic**
(menus, toolbars) and **Ribbon** (tabs of tool groups). ADR 0014 made the
project a multidiscipline platform: CAD now, then BIM architecture,
structures, MEP, topography… each with its own tools, most of them plugins.
The question is how a discipline's tools reach the window without one
workspace per discipline and layout written by hand.

## Decision

### Two independent choices

A **workspace** is a **discipline** shown in a **layout**. The user picks
each one separately, from View › Workspace and from a button in the status
bar (AutoCAD's gear). Milestone 2.4 ships one discipline, **2D drafting**
(`cad`), in both layouts.

**Only 2D drafting has the Classic layout.** Every other discipline (BIM,
structures, MEP…) is shown in the Ribbon; whether any of them gets a Classic
layout is decided later (maintainer, 2026-10-13). A discipline offers Classic
when it declares toolbars; otherwise the Classic choice is disabled while it
is shown. Each discipline remembers the layout it was last shown in.

### A discipline is data, declared in Python

`arqstudiocad.workspaces` (no Qt) describes a discipline as plain frozen
dataclasses: its ribbon tabs of groups (required) and its toolbars (only for
a discipline with a Classic layout), each listing **command names**.
Titles are functions that call `tr()`, so they translate when the language
changes. A `WorkspaceRegistry` holds the disciplines; discipline plugins will register
theirs the same way. The window builds toolbars and ribbon from that data
using the one `QAction` per command (ADR 0013 D5.1); a name without an
action is an error caught by the tests.

User customization (AutoCAD's CUI) is later work: files that add to or
override this data, not a second mechanism.

### Layouts

- **Ribbon** is the default on first run, as in current CAD and BIM tools.
  Our own widget: tabs holding groups of buttons with a title underneath,
  text until there are icons. Double-clicking a tab folds the ribbon to
  its tab bar, and again unfolds it, for small screens such as the
  reference machine's.
- **Classic** shows the discipline's toolbars.
- The **menu bar stays in both** for now (File, Edit, View, Tools, Help);
  a ribbon application menu may replace it later.
- Panels and the command window are shared. Their arrangement (dock and
  toolbar state) is saved **per workspace** and restored when the user
  comes back to it. The current discipline and layout are remembered.

## Consequences

- Adding a discipline is declaring data and its commands; no window code.
- Classic and Ribbon cannot drift apart: both show the same actions.
- A ribbon is required of every discipline; toolbars are optional, so the
  Classic layout costs nothing to disciplines that do not want it.
- The data format is internal until customization exists, so it can change
  freely; the registry API is what plugins will rely on (ADR 0012 rules for
  public API apply when it is exported).
