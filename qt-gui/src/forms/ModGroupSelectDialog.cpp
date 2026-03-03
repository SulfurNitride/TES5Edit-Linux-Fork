#include "ModGroupSelectDialog.h"
#include <QHBoxLayout>
#include <QVBoxLayout>

ModGroupSelectDialog::ModGroupSelectDialog(const QStringList& availableModGroups,
                                           QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Select Mod Groups");
    resize(400, 500);

    auto* mainLayout = new QVBoxLayout(this);

    // Filter input
    m_filterEdit = new QLineEdit(this);
    m_filterEdit->setPlaceholderText("Filter mod groups...");
    m_filterEdit->setClearButtonEnabled(true);
    mainLayout->addWidget(m_filterEdit);

    // Mod group list
    m_lstModGroups = new QListWidget(this);
    mainLayout->addWidget(m_lstModGroups, 1);

    // Selection buttons row
    auto* selLayout = new QHBoxLayout();
    auto* btnSelectAll = new QPushButton("Select All", this);
    auto* btnSelectNone = new QPushButton("Select None", this);
    auto* btnInvert = new QPushButton("Invert", this);
    selLayout->addWidget(btnSelectAll);
    selLayout->addWidget(btnSelectNone);
    selLayout->addWidget(btnInvert);
    selLayout->addStretch();
    mainLayout->addLayout(selLayout);

    // OK / Cancel buttons
    auto* bottomLayout = new QHBoxLayout();
    bottomLayout->addStretch();
    m_btnOk = new QPushButton("OK", this);
    m_btnCancel = new QPushButton("Cancel", this);
    m_btnOk->setDefault(true);
    bottomLayout->addWidget(m_btnOk);
    bottomLayout->addWidget(m_btnCancel);
    mainLayout->addLayout(bottomLayout);

    // Populate
    populateList(availableModGroups);

    // Connections
    connect(m_filterEdit, &QLineEdit::textChanged, this, &ModGroupSelectDialog::onFilterChanged);
    connect(btnSelectAll, &QPushButton::clicked, this, &ModGroupSelectDialog::onSelectAll);
    connect(btnSelectNone, &QPushButton::clicked, this, &ModGroupSelectDialog::onSelectNone);
    connect(btnInvert, &QPushButton::clicked, this, &ModGroupSelectDialog::onInvertSelection);
    connect(m_btnOk, &QPushButton::clicked, this, &QDialog::accept);
    connect(m_btnCancel, &QPushButton::clicked, this, &QDialog::reject);
}

void ModGroupSelectDialog::populateList(const QStringList& modGroups)
{
    m_lstModGroups->clear();
    for (const QString& name : modGroups) {
        auto* item = new QListWidgetItem(name, m_lstModGroups);
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(Qt::Unchecked);
    }
}

QStringList ModGroupSelectDialog::selectedModGroups() const
{
    QStringList result;
    for (int i = 0; i < m_lstModGroups->count(); ++i) {
        auto* item = m_lstModGroups->item(i);
        if (item && !item->isHidden() && item->checkState() == Qt::Checked)
            result.append(item->text());
    }
    return result;
}

void ModGroupSelectDialog::setCheckedModGroups(const QStringList& modGroups)
{
    for (int i = 0; i < m_lstModGroups->count(); ++i) {
        auto* item = m_lstModGroups->item(i);
        if (item) {
            item->setCheckState(
                modGroups.contains(item->text()) ? Qt::Checked : Qt::Unchecked);
        }
    }
}

void ModGroupSelectDialog::onSelectAll()
{
    for (int i = 0; i < m_lstModGroups->count(); ++i) {
        auto* item = m_lstModGroups->item(i);
        if (item && !item->isHidden())
            item->setCheckState(Qt::Checked);
    }
}

void ModGroupSelectDialog::onSelectNone()
{
    for (int i = 0; i < m_lstModGroups->count(); ++i) {
        auto* item = m_lstModGroups->item(i);
        if (item && !item->isHidden())
            item->setCheckState(Qt::Unchecked);
    }
}

void ModGroupSelectDialog::onInvertSelection()
{
    for (int i = 0; i < m_lstModGroups->count(); ++i) {
        auto* item = m_lstModGroups->item(i);
        if (item && !item->isHidden()) {
            item->setCheckState(
                item->checkState() == Qt::Checked ? Qt::Unchecked : Qt::Checked);
        }
    }
}

void ModGroupSelectDialog::onFilterChanged(const QString& text)
{
    const QString filter = text.toLower();
    for (int i = 0; i < m_lstModGroups->count(); ++i) {
        auto* item = m_lstModGroups->item(i);
        if (item) {
            item->setHidden(
                !filter.isEmpty() && !item->text().toLower().contains(filter));
        }
    }
}
