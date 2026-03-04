#include "ScriptDialog.h"
#include "highlighting/PascalHighlighter.h"
#include <QAction>
#include <QFileDialog>
#include <QFont>
#include <QPlainTextEdit>
#include <QSplitter>
#include <QToolBar>
#include <QVBoxLayout>

ScriptDialog::ScriptDialog(QWidget* parent)
    : QDialog(parent)
{
    setupUI();
}

void ScriptDialog::setupUI()
{
    setWindowTitle("Script Editor");
    resize(800, 600);

    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);

    // --- Toolbar ---
    m_toolBar = new QToolBar(this);
    m_actRun  = m_toolBar->addAction("Run");
    m_actStop = m_toolBar->addAction("Stop");
    m_toolBar->addSeparator();
    m_actSave = m_toolBar->addAction("Save");
    m_actLoad = m_toolBar->addAction("Load");

    m_actRun->setShortcut(QKeySequence(Qt::Key_F5));
    m_actStop->setShortcut(QKeySequence(Qt::SHIFT | Qt::Key_F5));
    m_actSave->setShortcut(QKeySequence::Save);
    m_actLoad->setShortcut(QKeySequence::Open);

    m_actStop->setEnabled(false);

    connect(m_actRun,  &QAction::triggered, this, &ScriptDialog::onRun);
    connect(m_actStop, &QAction::triggered, this, &ScriptDialog::onStop);
    connect(m_actSave, &QAction::triggered, this, &ScriptDialog::onSave);
    connect(m_actLoad, &QAction::triggered, this, &ScriptDialog::onLoad);

    mainLayout->addWidget(m_toolBar);

    // --- Splitter: editor (top) + output (bottom) ---
    m_splitter = new QSplitter(Qt::Vertical, this);

    // Editor
    m_editor = new QPlainTextEdit(this);
    m_editor->setTabStopDistance(
        QFontMetricsF(m_editor->font()).horizontalAdvance(QLatin1Char(' ')) * 4);
    QFont editorFont(QStringLiteral("Monospace"));
    editorFont.setStyleHint(QFont::Monospace);
    editorFont.setPointSize(10);
    m_editor->setFont(editorFont);
    m_editor->setLineWrapMode(QPlainTextEdit::NoWrap);

    // Apply Pascal syntax highlighting
    m_highlighter = new PascalHighlighter(m_editor->document());

    // Output
    m_output = new QPlainTextEdit(this);
    m_output->setReadOnly(true);
    m_output->setFont(editorFont);
    m_output->setLineWrapMode(QPlainTextEdit::NoWrap);
    m_output->setPlaceholderText(QStringLiteral("Script output..."));

    m_splitter->addWidget(m_editor);
    m_splitter->addWidget(m_output);
    m_splitter->setStretchFactor(0, 3);
    m_splitter->setStretchFactor(1, 1);

    mainLayout->addWidget(m_splitter);
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

QString ScriptDialog::scriptText() const
{
    return m_editor->toPlainText();
}

void ScriptDialog::setScriptText(const QString& text)
{
    m_editor->setPlainText(text);
}

void ScriptDialog::appendOutput(const QString& text)
{
    m_output->appendPlainText(text);
}

void ScriptDialog::clearOutput()
{
    m_output->clear();
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------

void ScriptDialog::onRun()
{
    clearOutput();
    m_actRun->setEnabled(false);
    m_actStop->setEnabled(true);

    const QString script = m_editor->toPlainText();
    if (script.trimmed().isEmpty()) {
        appendOutput("No script to execute.");
        onStop();
        return;
    }

    appendOutput("Script execution not yet implemented.");
    appendOutput(QString("Script length: %1 characters").arg(script.length()));
    emit runRequested(script);

    // Since execution is not yet implemented, immediately reset to stopped state
    onStop();
}

void ScriptDialog::onStop()
{
    m_actRun->setEnabled(true);
    m_actStop->setEnabled(false);
    m_editor->setReadOnly(false);
    emit stopRequested();
}

void ScriptDialog::onSave()
{
    QString path = QFileDialog::getSaveFileName(
        this,
        QStringLiteral("Save Script"),
        QString(),
        QStringLiteral("Pascal files (*.pas);;All files (*)"));

    if (path.isEmpty())
        return;

    QFile file(path);
    if (file.open(QIODevice::WriteOnly | QIODevice::Text)) {
        file.write(m_editor->toPlainText().toUtf8());
        appendOutput(QStringLiteral("Saved: %1").arg(path));
    }
}

void ScriptDialog::onLoad()
{
    QString path = QFileDialog::getOpenFileName(
        this,
        QStringLiteral("Load Script"),
        QString(),
        QStringLiteral("Pascal files (*.pas);;All files (*)"));

    if (path.isEmpty())
        return;

    QFile file(path);
    if (file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        m_editor->setPlainText(QString::fromUtf8(file.readAll()));
        appendOutput(QStringLiteral("Loaded: %1").arg(path));
    }
}
