#include "DeveloperMessageDialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPushButton>

DeveloperMessageDialog::DeveloperMessageDialog(const QString& htmlContent, QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Developer Message");
    resize(600, 400);

    auto* mainLayout = new QVBoxLayout(this);

    // --- Rich text browser ---
    m_browser = new QTextBrowser(this);
    m_browser->setOpenExternalLinks(true);
    m_browser->setHtml(htmlContent);
    mainLayout->addWidget(m_browser, 1);

    // --- Don't show again ---
    m_dontShowCheck = new QCheckBox("Don't show again", this);
    mainLayout->addWidget(m_dontShowCheck);

    // --- Close button ---
    auto* btnLayout = new QHBoxLayout();
    btnLayout->addStretch();
    auto* btnClose = new QPushButton("Close", this);
    btnClose->setDefault(true);
    btnLayout->addWidget(btnClose);
    mainLayout->addLayout(btnLayout);

    connect(btnClose, &QPushButton::clicked, this, &QDialog::accept);
}

bool DeveloperMessageDialog::dontShowAgain() const
{
    return m_dontShowCheck->isChecked();
}
