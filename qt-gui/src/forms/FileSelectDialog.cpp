#include "FileSelectDialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>

FileSelectDialog::FileSelectDialog(const QStringList& files, QWidget* parent)
    : QDialog(parent)
    , m_allFiles(files)
{
    setWindowTitle("Select Files");
    resize(500, 400);

    auto* mainLayout = new QVBoxLayout(this);

    // --- Filter row ---
    auto* filterLayout = new QHBoxLayout();
    filterLayout->addWidget(new QLabel("Filter:", this));
    m_filterEdit = new QLineEdit(this);
    m_filterEdit->setPlaceholderText("Type to filter...");
    m_filterEdit->setClearButtonEnabled(true);
    filterLayout->addWidget(m_filterEdit);
    mainLayout->addLayout(filterLayout);

    // --- Check list ---
    m_listWidget = new QListWidget(this);
    m_listWidget->setSelectionMode(QAbstractItemView::ExtendedSelection);
    mainLayout->addWidget(m_listWidget, 1);

    // --- Context menu ---
    m_contextMenu = new QMenu(this);
    m_contextMenu->addAction("Select All",  this, &FileSelectDialog::selectAll);
    m_contextMenu->addAction("Select None", this, &FileSelectDialog::selectNone);
    m_contextMenu->addAction("Invert Selection", this, &FileSelectDialog::invertSelection);
    m_listWidget->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(m_listWidget, &QWidget::customContextMenuRequested,
            this, [this](const QPoint& pos) { m_contextMenu->popup(m_listWidget->mapToGlobal(pos)); });

    // --- Buttons ---
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
    connect(m_filterEdit, &QLineEdit::textChanged, this, &FileSelectDialog::onFilterChanged);

    // Double-click selects single item and accepts
    connect(m_listWidget, &QListWidget::itemDoubleClicked, this, [this](QListWidgetItem* item) {
        selectNone();
        item->setCheckState(Qt::Checked);
        accept();
    });

    rebuildList({});
}

QStringList FileSelectDialog::selectedFiles() const
{
    QStringList result;
    for (int i = 0; i < m_listWidget->count(); ++i) {
        auto* item = m_listWidget->item(i);
        if (item->checkState() == Qt::Checked)
            result.append(item->text());
    }
    return result;
}

void FileSelectDialog::onFilterChanged(const QString& text)
{
    // Preserve checked state across filter changes
    QSet<QString> checked;
    for (int i = 0; i < m_listWidget->count(); ++i) {
        auto* item = m_listWidget->item(i);
        if (item->checkState() == Qt::Checked)
            checked.insert(item->text());
    }

    rebuildList(text);

    for (int i = 0; i < m_listWidget->count(); ++i) {
        auto* item = m_listWidget->item(i);
        if (checked.contains(item->text()))
            item->setCheckState(Qt::Checked);
    }
}

void FileSelectDialog::selectAll()
{
    for (int i = 0; i < m_listWidget->count(); ++i)
        m_listWidget->item(i)->setCheckState(Qt::Checked);
}

void FileSelectDialog::selectNone()
{
    for (int i = 0; i < m_listWidget->count(); ++i)
        m_listWidget->item(i)->setCheckState(Qt::Unchecked);
}

void FileSelectDialog::invertSelection()
{
    for (int i = 0; i < m_listWidget->count(); ++i) {
        auto* item = m_listWidget->item(i);
        item->setCheckState(item->checkState() == Qt::Checked ? Qt::Unchecked : Qt::Checked);
    }
}

void FileSelectDialog::rebuildList(const QString& filter)
{
    m_listWidget->clear();
    const QString trimmed = filter.trimmed();
    for (const QString& file : m_allFiles) {
        if (!trimmed.isEmpty() && !file.contains(trimmed, Qt::CaseInsensitive))
            continue;
        auto* item = new QListWidgetItem(file, m_listWidget);
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(Qt::Unchecked);
    }
}
