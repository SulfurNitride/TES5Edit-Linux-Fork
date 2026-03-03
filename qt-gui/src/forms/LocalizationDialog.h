#pragma once

#include <QDialog>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSortFilterProxyModel>
#include <QSplitter>
#include <QStandardItemModel>
#include <QTableView>

class LocalizationDialog : public QDialog {
    Q_OBJECT
public:
    explicit LocalizationDialog(QWidget* parent = nullptr);

    // Navigate to a specific string by file name and ID
    void editValue(const QString& fileName, quint32 id);

    // Populate the table from localization data
    // Each entry: { id, type (STRINGS/DLSTRINGS/ILSTRINGS), value }
    struct StringEntry {
        quint32     id;
        QString     type;
        QString     value;
    };
    void setEntries(const QVector<StringEntry>& entries);

private slots:
    void onSelectionChanged();
    void onTextChanged();
    void onSave();
    void onExport();
    void onImport();
    void onFilterChanged(const QString& text);

private:
    void setupUi();

    // Left panel: table
    QTableView*             m_tableView;
    QStandardItemModel*     m_model;
    QSortFilterProxyModel*  m_proxyModel;
    QLineEdit*              m_filterEdit;

    // Right panel: text editor + save button
    QPlainTextEdit*         m_textEdit;
    QPushButton*            m_btnSave;

    // Toolbar buttons
    QPushButton*            m_btnExport;
    QPushButton*            m_btnImport;

    QSplitter*              m_splitter;
};
