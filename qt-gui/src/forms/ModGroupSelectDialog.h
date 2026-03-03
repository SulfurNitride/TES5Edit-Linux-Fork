#pragma once
#include <QDialog>
#include <QLineEdit>
#include <QListWidget>
#include <QPushButton>
#include <QStringList>

class ModGroupSelectDialog : public QDialog {
    Q_OBJECT
public:
    explicit ModGroupSelectDialog(const QStringList& availableModGroups,
                                  QWidget* parent = nullptr);

    QStringList selectedModGroups() const;

    void setCheckedModGroups(const QStringList& modGroups);

private slots:
    void onSelectAll();
    void onSelectNone();
    void onInvertSelection();
    void onFilterChanged(const QString& text);

private:
    void populateList(const QStringList& modGroups);

    QLineEdit* m_filterEdit;
    QListWidget* m_lstModGroups;
    QPushButton* m_btnOk;
    QPushButton* m_btnCancel;
};
