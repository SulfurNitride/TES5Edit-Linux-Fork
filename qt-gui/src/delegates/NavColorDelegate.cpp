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
    QVariant caVar = index.data(ConflictAllRole);
    if (caVar.isValid()) {
        auto ca = static_cast<ConflictAll>(caVar.toInt());
        QColor bg = ConflictColors::backgroundFor(ca);
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

    QStyledItemDelegate::paint(painter, opt, index);
}
