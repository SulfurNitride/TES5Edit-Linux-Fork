#pragma once
#include <QStyledItemDelegate>

// Custom data roles for conflict information
enum ConflictRole {
    ConflictAllRole = Qt::UserRole + 100,
    ConflictThisRole = Qt::UserRole + 101
};

class ConflictColorDelegate : public QStyledItemDelegate {
    Q_OBJECT
public:
    using QStyledItemDelegate::QStyledItemDelegate;

    void paint(QPainter* painter, const QStyleOptionViewItem& option,
               const QModelIndex& index) const override;

    // Inline editing support for editable value columns
    QWidget* createEditor(QWidget* parent, const QStyleOptionViewItem& option,
                          const QModelIndex& index) const override;
    void setEditorData(QWidget* editor, const QModelIndex& index) const override;
    void setModelData(QWidget* editor, QAbstractItemModel* model,
                      const QModelIndex& index) const override;
};
