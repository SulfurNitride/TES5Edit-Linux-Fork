#include "ModuleSelectDialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QSpacerItem>

ModuleSelectDialog::ModuleSelectDialog(const QStringList& availablePlugins, QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Select Modules");
    resize(600, 500);

    auto* mainLayout = new QVBoxLayout(this);

    // Filter input
    m_filterEdit = new QLineEdit(this);
    m_filterEdit->setPlaceholderText("Filter plugins...");
    m_filterEdit->setClearButtonEnabled(true);
    mainLayout->addWidget(m_filterEdit);

    // Source model
    m_model = new QStandardItemModel(this);

    // Proxy model for filtering
    m_proxyModel = new QSortFilterProxyModel(this);
    m_proxyModel->setSourceModel(m_model);
    m_proxyModel->setFilterCaseSensitivity(Qt::CaseInsensitive);

    // Tree view
    m_treeView = new QTreeView(this);
    m_treeView->setModel(m_proxyModel);
    m_treeView->setRootIsDecorated(false);
    m_treeView->setHeaderHidden(true);
    m_treeView->setEditTriggers(QAbstractItemView::NoEditTriggers);
    mainLayout->addWidget(m_treeView);

    // Bottom button row
    auto* bottomLayout = new QHBoxLayout();

    auto* btnSelectAll = new QPushButton("Select All", this);
    auto* btnSelectNone = new QPushButton("Select None", this);
    auto* btnInvert = new QPushButton("Invert Selection", this);

    bottomLayout->addWidget(btnSelectAll);
    bottomLayout->addWidget(btnSelectNone);
    bottomLayout->addWidget(btnInvert);
    bottomLayout->addStretch();

    m_btnOk = new QPushButton("OK", this);
    m_btnCancel = new QPushButton("Cancel", this);
    m_btnOk->setDefault(true);

    bottomLayout->addWidget(m_btnOk);
    bottomLayout->addWidget(m_btnCancel);

    mainLayout->addLayout(bottomLayout);

    // Populate items
    populateModel(availablePlugins);

    // Connections
    connect(m_filterEdit, &QLineEdit::textChanged, this, &ModuleSelectDialog::onFilterChanged);
    connect(btnSelectAll, &QPushButton::clicked, this, &ModuleSelectDialog::onSelectAll);
    connect(btnSelectNone, &QPushButton::clicked, this, &ModuleSelectDialog::onSelectNone);
    connect(btnInvert, &QPushButton::clicked, this, &ModuleSelectDialog::onInvertSelection);
    connect(m_btnOk, &QPushButton::clicked, this, &QDialog::accept);
    connect(m_btnCancel, &QPushButton::clicked, this, &QDialog::reject);
}

void ModuleSelectDialog::populateModel(const QStringList& plugins)
{
    m_model->clear();
    for (const QString& plugin : plugins) {
        auto* item = new QStandardItem(plugin);
        item->setCheckable(true);
        item->setCheckState(Qt::Unchecked);
        item->setFlags(Qt::ItemIsEnabled | Qt::ItemIsUserCheckable);
        m_model->appendRow(item);
    }
}

QStringList ModuleSelectDialog::selectedPlugins() const
{
    QStringList result;
    for (int i = 0; i < m_model->rowCount(); ++i) {
        QStandardItem* item = m_model->item(i);
        if (item && item->checkState() == Qt::Checked) {
            result.append(item->text());
        }
    }
    return result;
}

void ModuleSelectDialog::onFilterChanged(const QString& text)
{
    m_proxyModel->setFilterFixedString(text);
}

void ModuleSelectDialog::onSelectAll()
{
    for (int i = 0; i < m_model->rowCount(); ++i) {
        QStandardItem* item = m_model->item(i);
        if (item)
            item->setCheckState(Qt::Checked);
    }
}

void ModuleSelectDialog::onSelectNone()
{
    for (int i = 0; i < m_model->rowCount(); ++i) {
        QStandardItem* item = m_model->item(i);
        if (item)
            item->setCheckState(Qt::Unchecked);
    }
}

void ModuleSelectDialog::onInvertSelection()
{
    for (int i = 0; i < m_model->rowCount(); ++i) {
        QStandardItem* item = m_model->item(i);
        if (item) {
            item->setCheckState(
                item->checkState() == Qt::Checked ? Qt::Unchecked : Qt::Checked
            );
        }
    }
}
