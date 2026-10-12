#!/bin/sh
# Flatpak launcher: run the application from its installed source tree, with
# PySide6 provided by io.qt.PySide.BaseApp.
export PYTHONPATH="/app/share/arqstudiocad/python${PYTHONPATH:+:$PYTHONPATH}"
exec python3 -m arqstudiocad "$@"
