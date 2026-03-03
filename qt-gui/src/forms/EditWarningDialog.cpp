#include "EditWarningDialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QStyle>

EditWarningDialog::EditWarningDialog(const QString& message, QWidget* parent)
    : QDialog(parent)
    , m_countdown(3)
{
    setWindowTitle("Edit Warning");
    setModal(true);

    auto* mainLayout = new QVBoxLayout(this);

    // --- Warning icon + message ---
    auto* topLayout = new QHBoxLayout();
    auto* iconLabel = new QLabel(this);
    iconLabel->setPixmap(style()->standardPixmap(QStyle::SP_MessageBoxWarning));
    iconLabel->setFixedSize(48, 48);
    iconLabel->setScaledContents(true);
    topLayout->addWidget(iconLabel, 0, Qt::AlignTop);

    m_messageLabel = new QLabel(message, this);
    m_messageLabel->setWordWrap(true);
    topLayout->addWidget(m_messageLabel, 1);
    mainLayout->addLayout(topLayout);

    mainLayout->addSpacing(8);

    // --- Don't show again ---
    m_dontShowCheck = new QCheckBox("Don't show this warning again", this);
    mainLayout->addWidget(m_dontShowCheck);

    mainLayout->addSpacing(8);

    // --- Buttons ---
    auto* btnLayout = new QHBoxLayout();
    btnLayout->addStretch();

    m_confirmButton = new QPushButton(this);
    m_confirmButton->setEnabled(false);
    m_confirmButton->setText(QStringLiteral("Yes, I'm sure (%1)").arg(m_countdown));

    m_cancelButton = new QPushButton("Cancel", this);
    m_cancelButton->setDefault(true);

    btnLayout->addWidget(m_confirmButton);
    btnLayout->addWidget(m_cancelButton);
    mainLayout->addLayout(btnLayout);

    connect(m_confirmButton, &QPushButton::clicked, this, &QDialog::accept);
    connect(m_cancelButton,  &QPushButton::clicked, this, &QDialog::reject);

    // --- Countdown timer (mirrors Delphi TimerCount behaviour) ---
    m_timer = new QTimer(this);
    m_timer->setInterval(1000);
    connect(m_timer, &QTimer::timeout, this, &EditWarningDialog::onTimerTick);
    m_timer->start();
}

bool EditWarningDialog::dontShowAgain() const
{
    return m_dontShowCheck->isChecked();
}

bool EditWarningDialog::confirmEdit(QWidget* parent, const QString& message)
{
    EditWarningDialog dlg(message, parent);
    return dlg.exec() == QDialog::Accepted;
}

void EditWarningDialog::onTimerTick()
{
    --m_countdown;
    if (m_countdown <= 0) {
        m_timer->stop();
        m_confirmButton->setEnabled(true);
        m_confirmButton->setText("Yes, I'm sure");
        m_confirmButton->setDefault(true);
    } else {
        m_confirmButton->setText(QStringLiteral("Yes, I'm sure (%1)").arg(m_countdown));
    }
}
