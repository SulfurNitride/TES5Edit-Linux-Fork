#include "ModGroupEditDialog.h"
#include <QHBoxLayout>
#include <QInputDialog>
#include <QLabel>
#include <QMessageBox>
#include <QVBoxLayout>

ModGroupEditDialog::ModGroupEditDialog(const QString& modGroupName,
                                       const QStringList& plugins,
                                       QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Edit Mod Group");
    resize(500, 400);
    setupUI();

    m_edName->setText(modGroupName);
    setPlugins(plugins);
    validateState();
}

void ModGroupEditDialog::setupUI()
{
    auto* mainLayout = new QVBoxLayout(this);

    // Mod group name
    auto* nameLayout = new QHBoxLayout();
    nameLayout->addWidget(new QLabel("Mod Group Name:", this));
    m_edName = new QLineEdit(this);
    m_edName->setPlaceholderText("Enter mod group name...");
    nameLayout->addWidget(m_edName, 1);
    mainLayout->addLayout(nameLayout);

    // Plugin list + side buttons
    auto* middleLayout = new QHBoxLayout();

    m_lstPlugins = new QListWidget(this);
    m_lstPlugins->setDragDropMode(QAbstractItemView::InternalMove);
    middleLayout->addWidget(m_lstPlugins, 1);

    // Side buttons
    auto* sideLayout = new QVBoxLayout();
    m_btnAdd = new QPushButton("Add...", this);
    m_btnRemove = new QPushButton("Remove", this);
    m_btnMoveUp = new QPushButton("Move Up", this);
    m_btnMoveDown = new QPushButton("Move Down", this);

    sideLayout->addWidget(m_btnAdd);
    sideLayout->addWidget(m_btnRemove);
    sideLayout->addSpacing(12);
    sideLayout->addWidget(m_btnMoveUp);
    sideLayout->addWidget(m_btnMoveDown);
    sideLayout->addStretch();
    middleLayout->addLayout(sideLayout);

    mainLayout->addLayout(middleLayout, 1);

    // OK / Cancel buttons
    auto* bottomLayout = new QHBoxLayout();
    bottomLayout->addStretch();
    m_btnOk = new QPushButton("OK", this);
    m_btnCancel = new QPushButton("Cancel", this);
    m_btnOk->setDefault(true);
    bottomLayout->addWidget(m_btnOk);
    bottomLayout->addWidget(m_btnCancel);
    mainLayout->addLayout(bottomLayout);

    // Connections
    connect(m_btnAdd, &QPushButton::clicked, this, &ModGroupEditDialog::onAddPlugin);
    connect(m_btnRemove, &QPushButton::clicked, this, &ModGroupEditDialog::onRemovePlugin);
    connect(m_btnMoveUp, &QPushButton::clicked, this, &ModGroupEditDialog::onMoveUp);
    connect(m_btnMoveDown, &QPushButton::clicked, this, &ModGroupEditDialog::onMoveDown);
    connect(m_edName, &QLineEdit::textChanged, this, &ModGroupEditDialog::onNameChanged);
    connect(m_lstPlugins, &QListWidget::itemSelectionChanged, this, &ModGroupEditDialog::onSelectionChanged);
    connect(m_btnOk, &QPushButton::clicked, this, &QDialog::accept);
    connect(m_btnCancel, &QPushButton::clicked, this, &QDialog::reject);

    // Initial button state
    m_btnRemove->setEnabled(false);
    m_btnMoveUp->setEnabled(false);
    m_btnMoveDown->setEnabled(false);
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------
QString ModGroupEditDialog::modGroupName() const
{
    return m_edName->text().trimmed();
}

QStringList ModGroupEditDialog::plugins() const
{
    QStringList result;
    for (int i = 0; i < m_lstPlugins->count(); ++i) {
        auto* item = m_lstPlugins->item(i);
        if (item)
            result.append(item->text());
    }
    return result;
}

void ModGroupEditDialog::setModGroupName(const QString& name)
{
    m_edName->setText(name);
}

void ModGroupEditDialog::setPlugins(const QStringList& plugins)
{
    m_lstPlugins->clear();
    m_lstPlugins->addItems(plugins);
    validateState();
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------
void ModGroupEditDialog::onAddPlugin()
{
    bool ok = false;
    QString plugin = QInputDialog::getText(
        this, "Add Plugin", "Plugin filename:", QLineEdit::Normal, {}, &ok);
    if (ok && !plugin.trimmed().isEmpty()) {
        m_lstPlugins->addItem(plugin.trimmed());
        validateState();
    }
}

void ModGroupEditDialog::onRemovePlugin()
{
    auto items = m_lstPlugins->selectedItems();
    for (auto* item : items)
        delete item;
    validateState();
}

void ModGroupEditDialog::onMoveUp()
{
    int row = m_lstPlugins->currentRow();
    if (row <= 0)
        return;
    auto* item = m_lstPlugins->takeItem(row);
    m_lstPlugins->insertItem(row - 1, item);
    m_lstPlugins->setCurrentRow(row - 1);
}

void ModGroupEditDialog::onMoveDown()
{
    int row = m_lstPlugins->currentRow();
    if (row < 0 || row >= m_lstPlugins->count() - 1)
        return;
    auto* item = m_lstPlugins->takeItem(row);
    m_lstPlugins->insertItem(row + 1, item);
    m_lstPlugins->setCurrentRow(row + 1);
}

void ModGroupEditDialog::onNameChanged(const QString& /*text*/)
{
    validateState();
}

void ModGroupEditDialog::onSelectionChanged()
{
    updateMoveButtons();
    m_btnRemove->setEnabled(!m_lstPlugins->selectedItems().isEmpty());
}

void ModGroupEditDialog::validateState()
{
    bool valid = !m_edName->text().trimmed().isEmpty()
              && m_lstPlugins->count() >= 2;
    m_btnOk->setEnabled(valid);
}

void ModGroupEditDialog::updateMoveButtons()
{
    int row = m_lstPlugins->currentRow();
    m_btnMoveUp->setEnabled(row > 0);
    m_btnMoveDown->setEnabled(row >= 0 && row < m_lstPlugins->count() - 1);
}
