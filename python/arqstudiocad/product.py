"""Product identity: the only place the user-visible name and IDs are written.

User-facing strings never spell the name out; they use the `{app}`
placeholder, which `tr()` fills from `APP_NAME`. Renaming the product means
changing this file (and packaging metadata), not every translation.
"""

APP_NAME = "ArqStudioCAD"
"""User-visible product name."""

APP_ID = "io.github._3djlchb.ArqStudioCAD"
"""Reverse-DNS application ID (ADR 0009). Must never change once published."""

ORGANIZATION = "ArqStudioCAD"
"""Organization name used by QSettings to locate saved preferences."""
