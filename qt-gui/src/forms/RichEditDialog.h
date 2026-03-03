#pragma once
#include <QDialog>
#include <QTextEdit>
#include <QToolBar>

class RichEditDialog : public QDialog {
    Q_OBJECT
public:
    explicit RichEditDialog(QWidget* parent = nullptr);

    QString toHtml() const;
    QString toPlainText() const;
    void setHtml(const QString& html);

private slots:
    void toggleBold();
    void toggleItalic();
    void toggleUnderline();
    void changeFontSize(const QString& sizeText);
    void updateFormatActions();

private:
    QTextEdit* m_textEdit;
    QToolBar*  m_toolbar;
    QAction*   m_boldAction;
    QAction*   m_italicAction;
    QAction*   m_underlineAction;
};
