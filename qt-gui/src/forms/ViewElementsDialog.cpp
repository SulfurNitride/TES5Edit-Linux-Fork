#include "ViewElementsDialog.h"
#include "ElementDetailDialog.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"

#include <QHBoxLayout>
#include <QHeaderView>
#include <QPushButton>
#include <QStandardItemModel>
#include <QStringDecoder>
#include <QTreeView>
#include <QVBoxLayout>

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

ViewElementsDialog::ViewElementsDialog(int pluginIdx, int groupIdx,
                                       int recordIdx, QWidget* parent)
    : QDialog(parent)
    , m_pluginIdx(pluginIdx)
    , m_groupIdx(groupIdx)
    , m_recordIdx(recordIdx)
{
    setWindowTitle(tr("View Elements"));
    resize(800, 600);

    buildUi();
    loadElements();
}

// ---------------------------------------------------------------------------
// UI construction
// ---------------------------------------------------------------------------

void ViewElementsDialog::buildUi()
{
    auto* mainLayout = new QVBoxLayout(this);

    // Tree view
    m_treeView = new QTreeView(this);
    m_treeView->setAlternatingRowColors(true);
    m_treeView->setRootIsDecorated(true);
    m_treeView->setSortingEnabled(false);
    m_treeView->setSelectionMode(QAbstractItemView::SingleSelection);
    m_treeView->setEditTriggers(QAbstractItemView::NoEditTriggers);
    mainLayout->addWidget(m_treeView, 1);

    connect(m_treeView, &QTreeView::doubleClicked,
            this, &ViewElementsDialog::onDoubleClicked);

    // Close button
    auto* btnLayout = new QHBoxLayout;
    btnLayout->addStretch();
    m_btnClose = new QPushButton(tr("Close"), this);
    m_btnClose->setDefault(true);
    btnLayout->addWidget(m_btnClose);
    mainLayout->addLayout(btnLayout);

    connect(m_btnClose, &QPushButton::clicked, this, &QDialog::accept);
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

static QString tryDecodeValue(const QByteArray& data)
{
    if (data.isEmpty())
        return {};

    QByteArray trimmed = data;
    while (!trimmed.isEmpty() && trimmed.back() == '\0')
        trimmed.chop(1);

    if (trimmed.isEmpty())
        return {};

    auto toUtf16 = QStringDecoder(QStringDecoder::Utf8);
    QString result = toUtf16(trimmed);
    if (toUtf16.hasError())
        return data.toHex(' ').toUpper();

    for (const QChar& ch : result) {
        if (ch.unicode() < 0x20 && ch != u'\n' && ch != u'\r' && ch != u'\t')
            return data.toHex(' ').toUpper();
    }
    return result;
}

void ViewElementsDialog::loadElements()
{
    m_model = new QStandardItemModel(this);
    m_model->setHorizontalHeaderLabels({tr("Name"), tr("Value"), tr("Type")});

    auto& ffi = XEditFFI::instance();
    if (!ffi.isLoaded() || !ffi.xedit_record_subrecord_count) {
        m_treeView->setModel(m_model);
        return;
    }

    // Record-level root item
    QString recordSig;
    if (ffi.xedit_record_signature) {
        recordSig = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_record_signature(
                m_pluginIdx, m_groupIdx, m_recordIdx, buf, len);
        });
    }
    QString editorId;
    if (ffi.xedit_record_editor_id) {
        editorId = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_record_editor_id(
                m_pluginIdx, m_groupIdx, m_recordIdx, buf, len);
        });
    }
    uint32_t formId = 0;
    if (ffi.xedit_record_form_id) {
        formId = ffi.xedit_record_form_id(
            m_pluginIdx, m_groupIdx, m_recordIdx);
    }

    QString recordLabel = QStringLiteral("%1 - %2 [%3]")
        .arg(recordSig, editorId,
             QStringLiteral("%1").arg(formId, 8, 16, QLatin1Char('0')).toUpper());

    auto* rootName  = new QStandardItem(recordLabel);
    auto* rootValue = new QStandardItem();
    auto* rootType  = new QStandardItem(QStringLiteral("Record"));
    rootName->setEditable(false);
    rootValue->setEditable(false);
    rootType->setEditable(false);

    // Store subrecord index as user data (-1 for record root)
    rootName->setData(-1, Qt::UserRole);

    // Subrecords as children
    const int32_t subCount = ffi.xedit_record_subrecord_count(
        m_pluginIdx, m_groupIdx, m_recordIdx);

    for (int32_t i = 0; i < subCount; ++i) {
        QString sig;
        if (ffi.xedit_subrecord_signature) {
            sig = ffiString([&](char* buf, int32_t len) {
                return ffi.xedit_subrecord_signature(
                    m_pluginIdx, m_groupIdx, m_recordIdx, i, buf, len);
            });
        }

        int32_t size = 0;
        if (ffi.xedit_subrecord_size) {
            size = ffi.xedit_subrecord_size(
                m_pluginIdx, m_groupIdx, m_recordIdx, i);
        }

        // Read raw data for value preview
        QByteArray rawData;
        if (size > 0 && ffi.xedit_subrecord_data) {
            const int32_t readLen = qMin(size, static_cast<int32_t>(256));
            rawData.resize(readLen);
            int32_t bytesRead = ffi.xedit_subrecord_data(
                m_pluginIdx, m_groupIdx, m_recordIdx, i,
                rawData.data(), readLen);
            if (bytesRead < 0) bytesRead = 0;
            rawData.truncate(bytesRead);
        }

        QString decoded = tryDecodeValue(rawData);
        // Truncate long values for the tree display
        if (decoded.length() > 120)
            decoded = decoded.left(120) + QStringLiteral("...");

        auto* nameItem  = new QStandardItem(sig);
        auto* valueItem = new QStandardItem(decoded);
        auto* typeItem  = new QStandardItem(
            QStringLiteral("Subrecord (%1 bytes)").arg(size));

        nameItem->setEditable(false);
        valueItem->setEditable(false);
        typeItem->setEditable(false);

        // Store subrecord index for double-click handling
        nameItem->setData(i, Qt::UserRole);

        rootName->appendRow({nameItem, valueItem, typeItem});
    }

    m_model->appendRow({rootName, rootValue, rootType});

    m_treeView->setModel(m_model);
    m_treeView->expandAll();

    // Resize columns to content
    for (int col = 0; col < m_model->columnCount(); ++col)
        m_treeView->resizeColumnToContents(col);
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------

void ViewElementsDialog::onDoubleClicked(const QModelIndex& index)
{
    if (!index.isValid())
        return;

    // Get the Name column (column 0) for the clicked row to read user data
    QModelIndex nameIdx = index.sibling(index.row(), 0);
    int subIdx = nameIdx.data(Qt::UserRole).toInt();
    if (subIdx < 0)
        return; // record root row, not a subrecord

    auto* dlg = new ElementDetailDialog(
        m_pluginIdx, m_groupIdx, m_recordIdx, subIdx, this);
    dlg->setAttribute(Qt::WA_DeleteOnClose);
    dlg->exec();
}
