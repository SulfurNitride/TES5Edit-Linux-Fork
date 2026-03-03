#include "ConflictColorDelegate.h"
#include "../util/ConflictColors.h"

#include <QLineEdit>
#include <QPainter>
#include <QStyleOptionViewItem>

void ConflictColorDelegate::paint(QPainter* painter,
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

// ---------------------------------------------------------------------------
// Inline editing support
// ---------------------------------------------------------------------------

QWidget* ConflictColorDelegate::createEditor(QWidget* parent,
                                             const QStyleOptionViewItem& option,
                                             const QModelIndex& index) const
{
    // Only allow editing if the model flags indicate the cell is editable
    if (!(index.flags() & Qt::ItemIsEditable))
        return nullptr;

    auto* editor = new QLineEdit(parent);
    editor->setFrame(false);
    return editor;
}

void ConflictColorDelegate::setEditorData(QWidget* editor,
                                          const QModelIndex& index) const
{
    auto* lineEdit = qobject_cast<QLineEdit*>(editor);
    if (!lineEdit)
        return;

    lineEdit->setText(index.data(Qt::DisplayRole).toString());
}

void ConflictColorDelegate::setModelData(QWidget* editor,
                                         QAbstractItemModel* model,
                                         const QModelIndex& index) const
{
    auto* lineEdit = qobject_cast<QLineEdit*>(editor);
    if (!lineEdit)
        return;

    model->setData(index, lineEdit->text(), Qt::EditRole);
}
