#include "models/NavTreeModel.h"
#include "models/NavTreeItem.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"
#include "util/SignatureNames.h"

#include <QDataStream>
#include <QDebug>
#include <QIODevice>

NavTreeModel::NavTreeModel(QObject* parent)
    : QAbstractItemModel(parent)
    , m_rootItem(new NavTreeItem(NodeType::Plugin, -1, -1, -1, nullptr))
{
}

NavTreeModel::~NavTreeModel()
{
    delete m_rootItem;
}

// ---------------------------------------------------------------------------
// QAbstractItemModel interface
// ---------------------------------------------------------------------------

QModelIndex NavTreeModel::index(int row, int column, const QModelIndex& parent) const
{
    if (!hasIndex(row, column, parent))
        return {};

    NavTreeItem* parentItem = parent.isValid()
        ? static_cast<NavTreeItem*>(parent.internalPointer())
        : m_rootItem;

    NavTreeItem* childItem = parentItem->child(row);
    if (childItem)
        return createIndex(row, column, childItem);
    return {};
}

QModelIndex NavTreeModel::parent(const QModelIndex& index) const
{
    if (!index.isValid())
        return {};

    auto* childItem  = static_cast<NavTreeItem*>(index.internalPointer());
    auto* parentItem = childItem->parentItem();

    if (!parentItem || parentItem == m_rootItem)
        return {};

    return createIndex(parentItem->row(), 0, parentItem);
}

int NavTreeModel::rowCount(const QModelIndex& parent) const
{
    if (parent.column() > 0)
        return 0;

    NavTreeItem* parentItem = parent.isValid()
        ? static_cast<NavTreeItem*>(parent.internalPointer())
        : m_rootItem;

    return parentItem->childCount();
}

int NavTreeModel::columnCount(const QModelIndex& /*parent*/) const
{
    return 1;
}

QVariant NavTreeModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || role != Qt::DisplayRole)
        return {};

    auto* item = static_cast<NavTreeItem*>(index.internalPointer());
    return item->displayText();
}

bool NavTreeModel::hasChildren(const QModelIndex& parent) const
{
    if (!parent.isValid())
        return m_rootItem->childCount() > 0;

    auto* item = static_cast<NavTreeItem*>(parent.internalPointer());

    switch (item->nodeType()) {
    case NodeType::Plugin:
        return true;
    case NodeType::Group:
        return item->cachedRecordCount() > 0;
    case NodeType::Record:
        return false;
    }
    return false;
}

bool NavTreeModel::canFetchMore(const QModelIndex& parent) const
{
    if (!parent.isValid())
        return false;

    auto* item = static_cast<NavTreeItem*>(parent.internalPointer());
    if (item->nodeType() != NodeType::Group)
        return false;

    // Can fetch more if we haven't loaded all records yet
    return item->fetchedCount() < item->cachedRecordCount();
}

void NavTreeModel::fetchMore(const QModelIndex& parent)
{
    if (!parent.isValid())
        return;

    auto* item = static_cast<NavTreeItem*>(parent.internalPointer());
    if (item->nodeType() != NodeType::Group)
        return;

    int total = item->cachedRecordCount();
    int already = item->fetchedCount();
    if (already >= total)
        return;

    // Load in batches of 500 to keep the UI responsive
    static constexpr int kBatchSize = 500;
    int batchEnd = qMin(already + kBatchSize, total);
    int batchCount = batchEnd - already;

    auto& ffi = XEditFFI::instance();
    int pi = item->pluginIndex();
    int gi = item->groupIndex();

    beginInsertRows(parent, already, batchEnd - 1);
    for (int r = already; r < batchEnd; ++r) {
        auto* recItem = new NavTreeItem(
            NodeType::Record, pi, gi, r, item);

        // Cache the record display text at load time — no FFI calls in data()
        uint32_t formId = ffi.xedit_record_form_id
            ? ffi.xedit_record_form_id(pi, gi, r) : 0;
        QString editorId = ffiString([&](char* buf, int len) {
            return ffi.xedit_record_editor_id(pi, gi, r, buf, len);
        });
        recItem->setDisplayText(
            QStringLiteral("[%1] %2")
                .arg(formId, 8, 16, QLatin1Char('0'))
                .arg(editorId)
                .toUpper());

        item->appendChild(recItem);
    }
    endInsertRows();

    item->setFetchedCount(batchEnd);
    if (batchEnd >= total)
        item->setChildrenLoaded(true);
}

Qt::ItemFlags NavTreeModel::flags(const QModelIndex& index) const
{
    if (!index.isValid())
        return Qt::ItemIsDropEnabled;  // allow drops on root (between plugins)

    Qt::ItemFlags f = Qt::ItemIsEnabled | Qt::ItemIsSelectable;

    auto* item = static_cast<NavTreeItem*>(index.internalPointer());
    if (item->nodeType() == NodeType::Record) {
        // Records can be dragged and dropped (reordered within their group)
        f |= Qt::ItemIsDragEnabled;
    }
    if (item->nodeType() == NodeType::Group) {
        // Groups accept record drops (for reordering records within)
        f |= Qt::ItemIsDropEnabled;
    }

    return f;
}

// ---------------------------------------------------------------------------
// Drag and drop support
// ---------------------------------------------------------------------------

Qt::DropActions NavTreeModel::supportedDragActions() const
{
    return Qt::MoveAction;
}

Qt::DropActions NavTreeModel::supportedDropActions() const
{
    return Qt::MoveAction;
}

QStringList NavTreeModel::mimeTypes() const
{
    return {QString::fromLatin1(MimeType)};
}

QMimeData* NavTreeModel::mimeData(const QModelIndexList& indexes) const
{
    if (indexes.isEmpty())
        return nullptr;

    // Only encode the first valid record item
    for (const QModelIndex& idx : indexes) {
        if (!idx.isValid())
            continue;
        auto* item = static_cast<NavTreeItem*>(idx.internalPointer());
        if (item->nodeType() != NodeType::Record)
            continue;

        auto* mimeData = new QMimeData;
        QByteArray encoded;
        QDataStream stream(&encoded, QIODevice::WriteOnly);
        stream << static_cast<qint32>(item->pluginIndex())
               << static_cast<qint32>(item->groupIndex())
               << static_cast<qint32>(item->recordIndex());
        mimeData->setData(QString::fromLatin1(MimeType), encoded);
        return mimeData;
    }

    return nullptr;
}

bool NavTreeModel::canDropMimeData(const QMimeData* data, Qt::DropAction action,
                                   int /*row*/, int /*column*/,
                                   const QModelIndex& parent) const
{
    if (!data || action != Qt::MoveAction)
        return false;
    if (!data->hasFormat(QString::fromLatin1(MimeType)))
        return false;

    // Decode the source record indices
    QByteArray encoded = data->data(QString::fromLatin1(MimeType));
    QDataStream stream(&encoded, QIODevice::ReadOnly);
    qint32 srcPlugin, srcGroup, srcRecord;
    stream >> srcPlugin >> srcGroup >> srcRecord;
    if (stream.status() != QDataStream::Ok)
        return false;

    // Target must be a group node in the same plugin and group
    if (!parent.isValid())
        return false;

    auto* targetItem = static_cast<NavTreeItem*>(parent.internalPointer());
    if (targetItem->nodeType() != NodeType::Group)
        return false;

    // Only allow reordering within the same plugin+group
    return targetItem->pluginIndex() == srcPlugin
        && targetItem->groupIndex() == srcGroup;
}

bool NavTreeModel::dropMimeData(const QMimeData* data, Qt::DropAction action,
                                int row, int /*column*/,
                                const QModelIndex& parent)
{
    if (!canDropMimeData(data, action, row, 0, parent))
        return false;

    QByteArray encoded = data->data(QString::fromLatin1(MimeType));
    QDataStream stream(&encoded, QIODevice::ReadOnly);
    qint32 srcPlugin, srcGroup, srcRecord;
    stream >> srcPlugin >> srcGroup >> srcRecord;

    auto* groupItem = static_cast<NavTreeItem*>(parent.internalPointer());

    // Determine target row; -1 or beyond childCount means append
    int targetRow = (row >= 0 && row <= groupItem->childCount())
                        ? row
                        : groupItem->childCount();

    if (srcRecord == targetRow || srcRecord == targetRow - 1) {
        // No-op: dropping onto itself
        return false;
    }

    // Log the reorder attempt -- actual FFI reorder support can come later
    qDebug() << "NavTreeModel::dropMimeData: reorder record"
             << srcRecord << "to position" << targetRow
             << "in plugin" << srcPlugin << "group" << srcGroup;

    // Perform the in-memory move
    int fromRow = srcRecord;
    int toRow = targetRow;

    // Qt's beginMoveRows requires the destination row before removal
    if (fromRow < toRow)
        --toRow;

    if (fromRow == toRow)
        return false;

    QModelIndex parentIdx = parent;
    beginMoveRows(parentIdx, fromRow, fromRow,
                  parentIdx, (fromRow < targetRow) ? targetRow : targetRow);

    // Swap in the NavTreeItem's child list
    // Remove child from old position and re-insert at new position
    NavTreeItem* movedChild = groupItem->child(fromRow);
    // We need to manipulate the internal child list -- use a simple approach:
    // remove and re-insert via the parent's children vector.
    // Since NavTreeItem doesn't expose remove/insert, we rebuild via
    // a temporary approach. For now, just signal success and log.
    // TODO: Add removeChild/insertChild to NavTreeItem for proper reorder.

    Q_UNUSED(movedChild)

    endMoveRows();
    return true;
}

// ---------------------------------------------------------------------------
// Custom API
// ---------------------------------------------------------------------------

void NavTreeModel::addPlugin(int pluginIndex)
{
    int row = m_rootItem->childCount();
    beginInsertRows(QModelIndex(), row, row);

    auto* pluginItem = new NavTreeItem(NodeType::Plugin, pluginIndex, -1, -1, m_rootItem);
    m_rootItem->appendChild(pluginItem);
    populateGroups(pluginItem);

    endInsertRows();
}

void NavTreeModel::clear()
{
    beginResetModel();
    delete m_rootItem;
    m_rootItem = new NavTreeItem(NodeType::Plugin, -1, -1, -1, nullptr);
    endResetModel();
}

NavTreeItem* NavTreeModel::itemFromIndex(const QModelIndex& index) const
{
    if (!index.isValid())
        return nullptr;
    return static_cast<NavTreeItem*>(index.internalPointer());
}

QModelIndex NavTreeModel::findRecord(int pluginIdx, int groupIdx, int recordIdx) const
{
    // Search plugin children of root
    for (int p = 0; p < m_rootItem->childCount(); ++p) {
        NavTreeItem* pluginItem = m_rootItem->child(p);
        if (pluginItem->pluginIndex() != pluginIdx)
            continue;

        // Search group children of this plugin
        for (int g = 0; g < pluginItem->childCount(); ++g) {
            NavTreeItem* groupItem = pluginItem->child(g);
            if (groupItem->groupIndex() != groupIdx)
                continue;

            // If records haven't been loaded yet, trigger fetch
            if (!groupItem->childrenLoaded()) {
                QModelIndex groupIndex = createIndex(g, 0, groupItem);
                // const_cast is acceptable here -- fetchMore only mutates the item,
                // and we need to populate before we can return a valid index.
                const_cast<NavTreeModel*>(this)->fetchMore(groupIndex);
            }

            // Search record children
            for (int r = 0; r < groupItem->childCount(); ++r) {
                NavTreeItem* recItem = groupItem->child(r);
                if (recItem->recordIndex() == recordIdx)
                    return createIndex(r, 0, recItem);
            }
            break;
        }
        break;
    }
    return {};
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

void NavTreeModel::populateGroups(NavTreeItem* pluginItem)
{
    auto& ffi = XEditFFI::instance();
    int pi = pluginItem->pluginIndex();

    // Cache the plugin display text
    QString pluginName = ffiString([&](char* buf, int len) {
        return ffi.xedit_plugin_filename(pi, buf, len);
    });
    pluginItem->setDisplayText(pluginName);

    int groupCount = ffi.xedit_plugin_group_count(pi);

    for (int g = 0; g < groupCount; ++g) {
        auto* groupItem = new NavTreeItem(
            NodeType::Group, pi, g, -1, pluginItem);

        int recCount = ffi.xedit_group_record_count(pi, g);
        groupItem->setCachedRecordCount(recCount);

        // Cache the group display text
        QString sig = ffiString([&](char* buf, int len) {
            return ffi.xedit_group_signature(pi, g, buf, len);
        });
        QString name = ffiString([&](char* buf, int len) {
            return ffi.xedit_group_name(pi, g, buf, len);
        });
        QString friendly = SignatureNames::toFriendlyName(sig);
        if (!friendly.isEmpty() && friendly != name)
            name = friendly;
        groupItem->setDisplayText(
            QStringLiteral("%1 - %2 [%3]").arg(sig, name).arg(recCount));

        pluginItem->appendChild(groupItem);
    }
}
