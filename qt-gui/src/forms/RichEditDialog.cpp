#include "RichEditDialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPushButton>
#include <QComboBox>
#include <QTextCharFormat>
#include <QFont>

RichEditDialog::RichEditDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Edit Text");
    resize(600, 400);

    auto* mainLayout = new QVBoxLayout(this);

    // --- Formatting toolbar ---
    m_toolbar = new QToolBar(this);

    m_boldAction = m_toolbar->addAction("B");
    QFont boldFont = m_boldAction->font();
    boldFont.setBold(true);
    m_boldAction->setFont(boldFont);
    m_boldAction->setCheckable(true);
    m_boldAction->setToolTip("Bold");
    m_boldAction->setShortcut(QKeySequence::Bold);

    m_italicAction = m_toolbar->addAction("I");
    QFont italicFont = m_italicAction->font();
    italicFont.setItalic(true);
    m_italicAction->setFont(italicFont);
    m_italicAction->setCheckable(true);
    m_italicAction->setToolTip("Italic");
    m_italicAction->setShortcut(QKeySequence::Italic);

    m_underlineAction = m_toolbar->addAction("U");
    QFont underlineFont = m_underlineAction->font();
    underlineFont.setUnderline(true);
    m_underlineAction->setFont(underlineFont);
    m_underlineAction->setCheckable(true);
    m_underlineAction->setToolTip("Underline");
    m_underlineAction->setShortcut(QKeySequence::Underline);

    m_toolbar->addSeparator();

    auto* sizeCombo = new QComboBox(this);
    for (int s : {8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 72})
        sizeCombo->addItem(QString::number(s));
    sizeCombo->setCurrentText("12");
    sizeCombo->setEditable(true);
    sizeCombo->setToolTip("Font Size");
    m_toolbar->addWidget(sizeCombo);

    mainLayout->addWidget(m_toolbar);

    // --- Text editor ---
    m_textEdit = new QTextEdit(this);
    m_textEdit->setAcceptRichText(true);
    mainLayout->addWidget(m_textEdit, 1);

    // --- OK / Cancel buttons ---
    auto* btnLayout = new QHBoxLayout();
    btnLayout->addStretch();
    auto* btnOk = new QPushButton("OK", this);
    btnOk->setDefault(true);
    auto* btnCancel = new QPushButton("Cancel", this);
    btnLayout->addWidget(btnOk);
    btnLayout->addWidget(btnCancel);
    mainLayout->addLayout(btnLayout);

    connect(btnOk,     &QPushButton::clicked, this, &QDialog::accept);
    connect(btnCancel, &QPushButton::clicked, this, &QDialog::reject);

    connect(m_boldAction,      &QAction::triggered, this, &RichEditDialog::toggleBold);
    connect(m_italicAction,    &QAction::triggered, this, &RichEditDialog::toggleItalic);
    connect(m_underlineAction, &QAction::triggered, this, &RichEditDialog::toggleUnderline);
    connect(sizeCombo, &QComboBox::currentTextChanged, this, &RichEditDialog::changeFontSize);

    connect(m_textEdit, &QTextEdit::currentCharFormatChanged,
            this, [this](const QTextCharFormat&) { updateFormatActions(); });
    connect(m_textEdit, &QTextEdit::cursorPositionChanged,
            this, &RichEditDialog::updateFormatActions);
}

QString RichEditDialog::toHtml() const
{
    return m_textEdit->toHtml();
}

QString RichEditDialog::toPlainText() const
{
    return m_textEdit->toPlainText();
}

void RichEditDialog::setHtml(const QString& html)
{
    m_textEdit->setHtml(html);
}

void RichEditDialog::toggleBold()
{
    QTextCharFormat fmt;
    fmt.setFontWeight(m_boldAction->isChecked() ? QFont::Bold : QFont::Normal);
    m_textEdit->mergeCurrentCharFormat(fmt);
}

void RichEditDialog::toggleItalic()
{
    QTextCharFormat fmt;
    fmt.setFontItalic(m_italicAction->isChecked());
    m_textEdit->mergeCurrentCharFormat(fmt);
}

void RichEditDialog::toggleUnderline()
{
    QTextCharFormat fmt;
    fmt.setFontUnderline(m_underlineAction->isChecked());
    m_textEdit->mergeCurrentCharFormat(fmt);
}

void RichEditDialog::changeFontSize(const QString& sizeText)
{
    bool ok = false;
    double size = sizeText.toDouble(&ok);
    if (ok && size > 0.0) {
        QTextCharFormat fmt;
        fmt.setFontPointSize(size);
        m_textEdit->mergeCurrentCharFormat(fmt);
    }
}

void RichEditDialog::updateFormatActions()
{
    QTextCharFormat fmt = m_textEdit->currentCharFormat();
    m_boldAction->setChecked(fmt.fontWeight() >= QFont::Bold);
    m_italicAction->setChecked(fmt.fontItalic());
    m_underlineAction->setChecked(fmt.fontUnderline());
}
