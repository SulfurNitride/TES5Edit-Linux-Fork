#include "TipDialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPushButton>
#include <QFont>
#include <QRandomGenerator>

TipDialog::TipDialog(const QStringList& tips, QWidget* parent)
    : QDialog(parent)
    , m_tips(tips)
    , m_currentIndex(0)
{
    setWindowTitle("Tip of the Day");
    resize(500, 300);

    auto* mainLayout = new QVBoxLayout(this);

    // --- Title ---
    auto* titleLabel = new QLabel("Tip", this);
    QFont titleFont = titleLabel->font();
    titleFont.setPointSize(16);
    titleFont.setBold(true);
    titleLabel->setFont(titleFont);
    mainLayout->addWidget(titleLabel);

    // --- Tip text ---
    m_tipLabel = new QLabel(this);
    m_tipLabel->setWordWrap(true);
    m_tipLabel->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    QFont tipFont = m_tipLabel->font();
    tipFont.setPointSize(11);
    m_tipLabel->setFont(tipFont);
    mainLayout->addWidget(m_tipLabel, 1);

    // --- Show at startup ---
    m_showAtStartupCheck = new QCheckBox("Show tips at startup", this);
    m_showAtStartupCheck->setChecked(true);
    mainLayout->addWidget(m_showAtStartupCheck);

    // --- Navigation + Close buttons ---
    auto* btnLayout = new QHBoxLayout();
    auto* btnPrev = new QPushButton("< Previous Tip", this);
    auto* btnNext = new QPushButton("Next Tip >", this);
    btnLayout->addWidget(btnPrev);
    btnLayout->addWidget(btnNext);
    btnLayout->addStretch();
    auto* btnClose = new QPushButton("Close", this);
    btnClose->setDefault(true);
    btnLayout->addWidget(btnClose);
    mainLayout->addLayout(btnLayout);

    connect(btnPrev,  &QPushButton::clicked, this, &TipDialog::previousTip);
    connect(btnNext,  &QPushButton::clicked, this, &TipDialog::nextTip);
    connect(btnClose, &QPushButton::clicked, this, &QDialog::accept);

    // Start at a random tip (mirrors Delphi behaviour)
    if (!m_tips.isEmpty())
        m_currentIndex = QRandomGenerator::global()->bounded(m_tips.size());

    displayCurrentTip();
}

bool TipDialog::showTipsAtStartup() const
{
    return m_showAtStartupCheck->isChecked();
}

void TipDialog::nextTip()
{
    if (m_tips.isEmpty())
        return;
    m_currentIndex = (m_currentIndex + 1) % m_tips.size();
    displayCurrentTip();
}

void TipDialog::previousTip()
{
    if (m_tips.isEmpty())
        return;
    m_currentIndex = (m_currentIndex - 1 + m_tips.size()) % m_tips.size();
    displayCurrentTip();
}

void TipDialog::displayCurrentTip()
{
    if (m_tips.isEmpty()) {
        m_tipLabel->setText("No tips available.");
        return;
    }
    m_tipLabel->setText(m_tips.at(m_currentIndex));
}
