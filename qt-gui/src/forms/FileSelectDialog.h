#pragma once
#include <QDialog>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QStringList>

class FileSelectDialog : public QDialog {
    Q_OBJECT
public:
    explicit FileSelectDialog(const QStringList& files, QWidget* parent = nullptr);

    QStringList selectedFiles() const;

private slots:
    void onFilterChanged(const QString& text);
    void selectAll();
    void selectNone();
    void invertSelection();

private:
    QLineEdit*   m_filterEdit;
    QListWidget* m_listWidget;
    QStringList  m_allFiles;
    QMenu*       m_contextMenu;

    void rebuildList(const QString& filter);
};
