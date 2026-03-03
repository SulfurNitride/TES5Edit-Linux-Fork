#pragma once

#include <QDialog>

class QTreeView;
class QPushButton;
class QStandardItemModel;

/// Dialog showing all elements (subrecords) of a record in a detailed tree.
/// Double-clicking an element opens ElementDetailDialog for that subrecord.
class ViewElementsDialog : public QDialog {
    Q_OBJECT
public:
    /// Construct for a specific record.
    explicit ViewElementsDialog(int pluginIdx, int groupIdx,
                                int recordIdx,
                                QWidget* parent = nullptr);

private slots:
    void onDoubleClicked(const QModelIndex& index);

private:
    void buildUi();
    void loadElements();

    int m_pluginIdx;
    int m_groupIdx;
    int m_recordIdx;

    QTreeView*           m_treeView = nullptr;
    QStandardItemModel*  m_model    = nullptr;
    QPushButton*         m_btnClose = nullptr;
};
