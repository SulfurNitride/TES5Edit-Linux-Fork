#include "ConflictColors.h"

namespace ConflictColors {

QColor backgroundFor(ConflictAll ca)
{
    switch (ca) {
    case ConflictAll::NoConflict: return QColor(0x00, 0xFF, 0x00);
    case ConflictAll::Benign:     return QColor(0xAD, 0xFF, 0x2F);
    case ConflictAll::Override:   return QColor(0xFF, 0xFF, 0x00);
    case ConflictAll::Conflict:   return QColor(0xFF, 0x00, 0x00);
    case ConflictAll::Critical:   return QColor(0xFF, 0x00, 0xFF);
    case ConflictAll::Unknown:
    case ConflictAll::OnlyOne:
    default:                      return Qt::transparent;
    }
}

QColor textColorFor(ConflictThis ct)
{
    switch (ct) {
    case ConflictThis::NotDefined:                     return QColor(0xA0, 0xA0, 0xA0);
    case ConflictThis::IdenticalToMaster:              return QColor(0x80, 0x80, 0x80);
    case ConflictThis::HiddenByModGroup:               return QColor(0xC0, 0xC0, 0xC0);
    case ConflictThis::Master:                         return QColor(0x80, 0x00, 0x80);
    case ConflictThis::Override:                       return QColor(0x00, 0x80, 0x00);
    case ConflictThis::IdenticalToMasterWinsConflict:  return QColor(0x80, 0x80, 0x00);
    case ConflictThis::ConflictWins:                   return QColor(0xFF, 0xA5, 0x00);
    case ConflictThis::ConflictLoses:                  return QColor(0xFF, 0x00, 0x00);
    case ConflictThis::Unknown:
    case ConflictThis::Ignored:
    case ConflictThis::OnlyOne:
    case ConflictThis::ConflictBenign:
    default:                                           return QColor();
    }
}

} // namespace ConflictColors
