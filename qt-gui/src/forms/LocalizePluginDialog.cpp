#include "LocalizePluginDialog.h"

#include <QHeaderView>
#include <QVBoxLayout>

// Column indices
static constexpr int ColStringID       = 0;
static constexpr int ColCurrentValue   = 1;
static constexpr int ColLocalizedValue = 2;

LocalizePluginDialog::LocalizePluginDialog(const QString& pluginName,
                                           QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Localize Plugin");
    resize(700, 500);
    setupUi(pluginName);
}

void LocalizePluginDialog::setupUi(const QString& pluginName)
{
    auto* mainLayout = new QVBoxLayout(this);

    // --- Description label (mirrors Delphi Label1) ---
    m_descriptionLabel = new QLabel(this);
    m_descriptionLabel->setWordWrap(true);
    m_descriptionLabel->setText(
        QStringLiteral(
            "Localization will create .STRINGS files with localizable strings from "
            "<b>%1</b> and replace their values in the plugin with string indexes. "
            "Use other specialized utilities like StrEdit to translate the generated "
            "strings files.")
            .arg(pluginName));
    mainLayout->addWidget(m_descriptionLabel);

    // --- Table view ---
    m_model = new QStandardItemModel(0, 3, this);
    m_model->setHorizontalHeaderLabels({"String ID", "Current Value", "Localized Value"});

    m_tableView = new QTableView(this);
    m_tableView->setModel(m_model);
    m_tableView->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_tableView->setSelectionMode(QAbstractItemView::SingleSelection);
    m_tableView->setSortingEnabled(true);
    m_tableView->verticalHeader()->hide();
    m_tableView->horizontalHeader()->setStretchLastSection(true);
    m_tableView->setAlternatingRowColors(true);

    // Current Value is read-only; Localized Value is editable
    // (handled via item flags in setStrings)

    mainLayout->addWidget(m_tableView, 1);

    // --- Buttons ---
    m_buttonBox = new QDialogButtonBox(this);

    m_btnLocalize = new QPushButton("Localize", this);
    m_btnCancel   = new QPushButton("Cancel", this);

    m_buttonBox->addButton(m_btnLocalize, QDialogButtonBox::AcceptRole);
    m_buttonBox->addButton(m_btnCancel,   QDialogButtonBox::RejectRole);

    connect(m_buttonBox, &QDialogButtonBox::accepted,
            this, &LocalizePluginDialog::onLocalize);
    connect(m_buttonBox, &QDialogButtonBox::rejected,
            this, &QDialog::reject);

    mainLayout->addWidget(m_buttonBox);
}

void LocalizePluginDialog::setStrings(const QVector<LocalizableString>& strings)
{
    m_model->removeRows(0, m_model->rowCount());
    m_model->setRowCount(strings.size());

    for (int i = 0; i < strings.size(); ++i) {
        const auto& s = strings[i];

        // String ID column (hex, read-only)
        auto* idItem = new QStandardItem(
            QStringLiteral("%1").arg(s.stringId, 8, 16, QLatin1Char('0')).toUpper());
        idItem->setFlags(idItem->flags() & ~Qt::ItemIsEditable);
        idItem->setData(s.stringId, Qt::UserRole);
        m_model->setItem(i, ColStringID, idItem);

        // Current Value column (read-only)
        auto* currentItem = new QStandardItem(s.currentValue);
        currentItem->setFlags(currentItem->flags() & ~Qt::ItemIsEditable);
        m_model->setItem(i, ColCurrentValue, currentItem);

        // Localized Value column (editable)
        auto* localizedItem = new QStandardItem(s.localizedValue);
        m_model->setItem(i, ColLocalizedValue, localizedItem);
    }

    m_tableView->resizeColumnsToContents();
}

QVector<LocalizePluginDialog::LocalizableString> LocalizePluginDialog::strings() const
{
    QVector<LocalizableString> result;
    result.reserve(m_model->rowCount());

    for (int row = 0; row < m_model->rowCount(); ++row) {
        LocalizableString s;
        s.stringId       = m_model->item(row, ColStringID)->data(Qt::UserRole).toUInt();
        s.currentValue   = m_model->item(row, ColCurrentValue)->text();
        s.localizedValue = m_model->item(row, ColLocalizedValue)->text();
        result.append(s);
    }

    return result;
}

void LocalizePluginDialog::onLocalize()
{
    accept();
}
