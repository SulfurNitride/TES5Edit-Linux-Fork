#!/bin/bash
# Launch xEditLaz with Qt6/Wayland noise suppressed
cd "$(dirname "$0")"

# Suppress Qt6/Wayland warnings that are harmless LCL compatibility issues:
#   - QComboBox::_q_returnPressed: LCL disconnects a slot that doesn't exist in Qt6
#   - "popup windows" mouse grab: Wayland doesn't allow mouse grab outside popups
#   - "Failed to create popup": Wayland requires transientParent on popups, LCL doesn't set it
#   - QThreadStorage: thread cleanup order on exit
export QT_LOGGING_RULES="qt.qpa.wayland.warning=false;qt.core.qobject.connect=false"

LD_LIBRARY_PATH=. exec ./xEditLaz 2>&1 | grep -v \
    -e "QThreadStorage:" \
    -e "This plugin supports grabbing"
