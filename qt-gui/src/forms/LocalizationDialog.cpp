#include "LocalizationDialog.h"

#include <QFileDialog>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QMessageBox>
#include <QVBoxLayout>

// Column indices
static constexpr int ColID    = 0;
static constexpr int ColType  = 1;
static constexpr int ColValue = 2;

LocalizationDialog::LocalizationDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Localization");
    resize(800, 500);
    setupUi();
}

void LocalizationDialog::setupUi()
{
    auto* mainLayout = new QVBoxLayout(this);

    // --- Top toolbar: filter + import/export ---
    auto* toolbarLayout = new QHBoxLayout;

    toolbarLayout->addWidget(new QLabel("Filter:", this));
    m_filterEdit = new QLineEdit(this);
    m_filterEdit->setPlaceholderText("Search by string content...");
    m_filterEdit->setClearButtonEnabled(true);
    toolbarLayout->addWidget(m_filterEdit, 1);

    m_btnImport = new QPushButton("Import...", this);
    m_btnExport = new QPushButton("Export...", this);
    toolbarLayout->addWidget(m_btnImport);
    toolbarLayout->addWidget(m_btnExport);

    mainLayout->addLayout(toolbarLayout);

    // --- Splitter: table (left) | text editor (right) ---
    m_splitter = new QSplitter(Qt::Horizontal, this);

    // Left: table view
    auto* leftWidget = new QWidget(m_splitter);
    auto* leftLayout = new QVBoxLayout(leftWidget);
    leftLayout->setContentsMargins(0, 0, 0, 0);

    m_model = new QStandardItemModel(0, 3, this);
    m_model->setHorizontalHeaderLabels({"ID", "Type", "String Value"});

    m_proxyModel = new QSortFilterProxyModel(this);
    m_proxyModel->setSourceModel(m_model);
    m_proxyModel->setFilterCaseSensitivity(Qt::CaseInsensitive);
    m_proxyModel->setFilterKeyColumn(ColValue);

    m_tableView = new QTableView(leftWidget);
    m_tableView->setModel(m_proxyModel);
    m_tableView->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_tableView->setSelectionMode(QAbstractItemView::SingleSelection);
    m_tableView->setEditTriggers(QAbstractItemView::NoEditTriggers);
    m_tableView->setSortingEnabled(true);
    m_tableView->verticalHeader()->hide();
    m_tableView->horizontalHeader()->setStretchLastSection(true);
    m_tableView->setAlternatingRowColors(true);

    leftLayout->addWidget(m_tableView);
    m_splitter->addWidget(leftWidget);

    // Right: text editor + save
    auto* rightWidget = new QWidget(m_splitter);
    auto* rightLayout = new QVBoxLayout(rightWidget);
    rightLayout->setContentsMargins(0, 0, 0, 0);

    m_textEdit = new QPlainTextEdit(rightWidget);
    m_textEdit->setReadOnly(true);
    rightLayout->addWidget(m_textEdit);

    auto* saveLayout = new QHBoxLayout;
    saveLayout->addStretch();
    m_btnSave = new QPushButton("Save", rightWidget);
    m_btnSave->setVisible(false);
    saveLayout->addWidget(m_btnSave);
    rightLayout->addLayout(saveLayout);

    m_splitter->addWidget(rightWidget);
    m_splitter->setStretchFactor(0, 2);
    m_splitter->setStretchFactor(1, 3);

    mainLayout->addWidget(m_splitter, 1);

    // --- Connections ---
    connect(m_filterEdit, &QLineEdit::textChanged,
            this, &LocalizationDialog::onFilterChanged);
    connect(m_tableView->selectionModel(), &QItemSelectionModel::currentRowChanged,
            this, [this]() { onSelectionChanged(); });
    connect(m_textEdit, &QPlainTextEdit::textChanged,
            this, &LocalizationDialog::onTextChanged);
    connect(m_btnSave,   &QPushButton::clicked, this, &LocalizationDialog::onSave);
    connect(m_btnExport, &QPushButton::clicked, this, &LocalizationDialog::onExport);
    connect(m_btnImport, &QPushButton::clicked, this, &LocalizationDialog::onImport);
}

void LocalizationDialog::setEntries(const QVector<StringEntry>& entries)
{
    m_model->removeRows(0, m_model->rowCount());
    m_model->setRowCount(entries.size());

    for (int i = 0; i < entries.size(); ++i) {
        const auto& e = entries[i];
        m_model->setItem(i, ColID,    new QStandardItem(
            QStringLiteral("%1").arg(e.id, 8, 16, QLatin1Char('0')).toUpper()));
        m_model->setItem(i, ColType,  new QStandardItem(e.type));
        m_model->setItem(i, ColValue, new QStandardItem(e.value));

        // Store the numeric ID in UserRole for lookup
        m_model->item(i, ColID)->setData(e.id, Qt::UserRole);
    }

    m_tableView->resizeColumnsToContents();
}

void LocalizationDialog::editValue(const QString& fileName, quint32 id)
{
    // Search for the row matching the given ID; optionally filter by file name prefix
    const QString prefix = fileName.section('.', 0, 0) + '_';

    for (int row = 0; row < m_proxyModel->rowCount(); ++row) {
        auto idIndex = m_proxyModel->index(row, ColID);
        quint32 rowId = m_proxyModel->data(idIndex, Qt::UserRole).toUInt();
        if (rowId == id) {
            m_tableView->selectRow(row);
            m_tableView->scrollTo(idIndex);
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------

void LocalizationDialog::onSelectionChanged()
{
    auto current = m_tableView->currentIndex();
    if (!current.isValid()) {
        m_textEdit->clear();
        m_textEdit->setReadOnly(true);
        m_btnSave->setVisible(false);
        return;
    }

    auto valueIndex = m_proxyModel->index(current.row(), ColValue);
    QString text = m_proxyModel->data(valueIndex, Qt::DisplayRole).toString();

    m_textEdit->setReadOnly(false);
    m_textEdit->setPlainText(text);
    m_btnSave->setVisible(false);
}

void LocalizationDialog::onTextChanged()
{
    if (!m_textEdit->isReadOnly())
        m_btnSave->setVisible(true);
}

void LocalizationDialog::onSave()
{
    auto current = m_tableView->currentIndex();
    if (!current.isValid())
        return;

    // Map proxy index back to source model
    auto sourceIndex = m_proxyModel->mapToSource(
        m_proxyModel->index(current.row(), ColValue));
    m_model->setData(sourceIndex, m_textEdit->toPlainText(), Qt::DisplayRole);
    m_btnSave->setVisible(false);
}

void LocalizationDialog::onExport()
{
    QString filter = "String Files (*.STRINGS *.DLSTRINGS *.ILSTRINGS);;Text Files (*.txt);;All Files (*)";
    QString path = QFileDialog::getSaveFileName(this, "Export Localization", QString(), filter);
    if (path.isEmpty())
        return;

    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text)) {
        QMessageBox::warning(this, "Export Error",
            QStringLiteral("Could not open file for writing:\n%1").arg(path));
        return;
    }

    QTextStream out(&file);
    for (int row = 0; row < m_model->rowCount(); ++row) {
        QString id    = m_model->item(row, ColID)->text();
        QString type  = m_model->item(row, ColType)->text();
        QString value = m_model->item(row, ColValue)->text();
        out << id << '\t' << type << '\t' << value << '\n';
    }
}

void LocalizationDialog::onImport()
{
    QString filter = "String Files (*.STRINGS *.DLSTRINGS *.ILSTRINGS);;Text Files (*.txt);;All Files (*)";
    QString path = QFileDialog::getOpenFileName(this, "Import Localization", QString(), filter);
    if (path.isEmpty())
        return;

    QFile file(path);
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        QMessageBox::warning(this, "Import Error",
            QStringLiteral("Could not open file for reading:\n%1").arg(path));
        return;
    }

    QVector<StringEntry> entries;
    QTextStream in(&file);
    while (!in.atEnd()) {
        QString line = in.readLine();
        QStringList parts = line.split('\t');
        if (parts.size() >= 3) {
            StringEntry e;
            bool ok = false;
            e.id    = parts[0].toUInt(&ok, 16);
            e.type  = parts[1];
            e.value = parts[2];
            if (ok)
                entries.append(e);
        }
    }

    setEntries(entries);
}

void LocalizationDialog::onFilterChanged(const QString& text)
{
    m_proxyModel->setFilterFixedString(text);
}
