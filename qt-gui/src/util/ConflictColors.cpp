#include "ConflictColors.h"
#include <QApplication>
#include <QPalette>

namespace ConflictColors {

bool isDarkTheme() {
    return QApplication::palette().color(QPalette::Window).lightnessF() < 0.5;
}

static QColor lightenColor(const QColor& color, float factor = 0.5f) {
    float h, s, l, a;
    color.getHslF(&h, &s, &l, &a);
    l = qMin(l + (1.0f - l) * factor, 1.0f);
    return QColor::fromHslF(h, s, l, a);
}

static QColor darkenColor(const QColor& color, float factor = 0.5f) {
    float h, s, l, a;
    color.getHslF(&h, &s, &l, &a);
    l = qMax(l - l * factor, 0.0f);
    return QColor::fromHslF(h, s, l, a);
}

QColor backgroundFor(ConflictAll ca)
{
    QColor base;
    switch (ca) {
    case ConflictAll::NoConflict: base = QColor(0x00, 0xFF, 0x00); break;
    case ConflictAll::Benign:     base = QColor(0xAD, 0xFF, 0x2F); break;
    case ConflictAll::Override:   base = QColor(0xFF, 0xFF, 0x00); break;
    case ConflictAll::Conflict:   base = QColor(0xFF, 0x00, 0x00); break;
    case ConflictAll::Critical:   base = QColor(0xFF, 0x00, 0xFF); break;
    case ConflictAll::Unknown:
    case ConflictAll::OnlyOne:
    default:                      return Qt::transparent;
    }

    if (isDarkTheme())
        return darkenColor(base, 0.5f * 0.95f);  // Darken by ~47.5% in dark mode
    else
        return lightenColor(base, 0.5f);          // Lighten by 50% in light mode
}

QColor textColorFor(ConflictThis ct)
{
    QColor base;
    switch (ct) {
    case ConflictThis::NotDefined:                     base = QColor(0xA0, 0xA0, 0xA0); break;
    case ConflictThis::IdenticalToMaster:              base = QColor(0x80, 0x80, 0x80); break;
    case ConflictThis::HiddenByModGroup:               base = QColor(0xC0, 0xC0, 0xC0); break;
    case ConflictThis::Master:                         base = QColor(0x80, 0x00, 0x80); break;
    case ConflictThis::Override:                       base = QColor(0x00, 0x80, 0x00); break;
    case ConflictThis::IdenticalToMasterWinsConflict:  base = QColor(0x80, 0x80, 0x00); break;
    case ConflictThis::ConflictWins:                   base = QColor(0xFF, 0x80, 0x40); break;
    case ConflictThis::ConflictLoses:                  base = QColor(0xFF, 0x00, 0x00); break;
    case ConflictThis::Unknown:
    case ConflictThis::Ignored:
    case ConflictThis::OnlyOne:
    case ConflictThis::ConflictBenign:
    default:                                           return QColor();
    }

    if (isDarkTheme())
        return lightenColor(base, 0.25f);  // Lighten text in dark mode for readability
    else
        return base;                       // Unchanged in light mode
}

} // namespace ConflictColors
