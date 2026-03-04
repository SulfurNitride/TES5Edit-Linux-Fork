#include "NavColorDelegate.h"
#include "ConflictColorDelegate.h"
#include "../util/ConflictColors.h"

#include <QPainter>
#include <QStyleOptionViewItem>

void NavColorDelegate::paint(QPainter* painter,
                             const QStyleOptionViewItem& option,
                             const QModelIndex& index) const
{
    QStyleOptionViewItem opt = option;
    initStyleOption(&opt, index);

    // Apply background color from ConflictAll status
    QColor bg;
    QVariant caVar = index.data(ConflictAllRole);
    if (caVar.isValid()) {
        auto ca = static_cast<ConflictAll>(caVar.toInt());
        bg = ConflictColors::backgroundFor(ca);
        if (bg.isValid() && bg != Qt::transparent) {
            painter->fillRect(opt.rect, bg);
        }
    }

    // Apply text color from ConflictThis status
    QVariant ctVar = index.data(ConflictThisRole);
    if (ctVar.isValid()) {
        auto ct = static_cast<ConflictThis>(ctVar.toInt());
        QColor fg = ConflictColors::textColorFor(ct);
        if (fg.isValid()) {
            opt.palette.setColor(QPalette::Text, fg);
            opt.palette.setColor(QPalette::HighlightedText, fg);
        }
    }

    // Force readable text on conflict backgrounds when no ConflictThis color is set
    if (bg.isValid() && bg != Qt::transparent) {
        if (!ctVar.isValid() || !ConflictColors::textColorFor(static_cast<ConflictThis>(ctVar.toInt())).isValid()) {
            QColor textColor = ConflictColors::isDarkTheme() ? Qt::white : Qt::black;
            opt.palette.setColor(QPalette::Text, textColor);
            opt.palette.setColor(QPalette::HighlightedText, textColor);
        }
    }

    QStyledItemDelegate::paint(painter, opt, index);
}
