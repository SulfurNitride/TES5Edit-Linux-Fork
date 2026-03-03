#pragma once
#include <QDialog>
#include <QLineEdit>
#include <QTreeView>
#include <QPushButton>
#include <QStandardItemModel>
#include <QSortFilterProxyModel>
#include <QStringList>

class ModuleSelectDialog : public QDialog {
    Q_OBJECT
public:
    explicit ModuleSelectDialog(const QStringList& availablePlugins, QWidget* parent = nullptr);

    QStringList selectedPlugins() const;

private slots:
    void onFilterChanged(const QString& text);
    void onSelectAll();
    void onSelectNone();
    void onInvertSelection();

private:
    void populateModel(const QStringList& plugins);

    QStandardItemModel* m_model;
    QSortFilterProxyModel* m_proxyModel;
    QTreeView* m_treeView;
    QLineEdit* m_filterEdit;
    QPushButton* m_btnOk;
    QPushButton* m_btnCancel;
};
