#pragma once
#include <QColor>

// Record-level conflict status (across all mods)
enum class ConflictAll {
    Unknown = 0,
    OnlyOne,
    NoConflict,
    Benign,
    Override,
    Conflict,
    Critical
};

// File-specific conflict status
enum class ConflictThis {
    Unknown = 0,
    Ignored,
    NotDefined,
    IdenticalToMaster,
    OnlyOne,
    HiddenByModGroup,
    Master,
    ConflictBenign,
    Override,
    IdenticalToMasterWinsConflict,
    ConflictWins,
    ConflictLoses
};

namespace ConflictColors {
    bool isDarkTheme();
    QColor backgroundFor(ConflictAll ca);
    QColor textColorFor(ConflictThis ct);
}
