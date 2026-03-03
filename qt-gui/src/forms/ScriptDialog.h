#pragma once
#include <QDialog>

class QAction;
class QPlainTextEdit;
class QSplitter;
class QToolBar;
class PascalHighlighter;

class ScriptDialog : public QDialog {
    Q_OBJECT
public:
    explicit ScriptDialog(QWidget* parent = nullptr);

    QString scriptText() const;
    void setScriptText(const QString& text);

    void appendOutput(const QString& text);
    void clearOutput();

signals:
    void runRequested(const QString& script);
    void stopRequested();

private slots:
    void onRun();
    void onStop();
    void onSave();
    void onLoad();

private:
    void setupUI();

    QToolBar*          m_toolBar;
    QPlainTextEdit*    m_editor;
    QPlainTextEdit*    m_output;
    QSplitter*         m_splitter;
    PascalHighlighter* m_highlighter;

    QAction* m_actRun;
    QAction* m_actStop;
    QAction* m_actSave;
    QAction* m_actLoad;
};
