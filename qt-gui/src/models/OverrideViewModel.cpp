#include "OverrideViewModel.h"

#include "ffi/XEditFFI.h"
#include "util/ConflictColors.h"
#include "util/SignatureNames.h"
#include "util/StringBuffer.h"

#include <QByteArray>
#include <QDebug>
#include <QHash>
#include <QSet>
#include <QtEndian>

namespace {

struct ParsedSubrecord {
    QString signature;
    QByteArray previewData;
    int fullSize = 0;
    int index = -1;
};

static QString fieldLabelForSignature(const QString& signature)
{
    const QString friendly = SignatureNames::toFriendlyName(signature);
    if (friendly.isEmpty() || friendly == signature)
        return signature;
    return QStringLiteral("%1 - %2").arg(signature, friendly);
}

static QString hexPreview(const QByteArray& data, int fullSize)
{
    QString hex = data.toHex(' ').toUpper();
    if (fullSize > data.size())
        hex += QStringLiteral(" ...");
    return hex;
}

static QVector<ParsedSubrecord> loadSubrecordsForRecord(
    XEditFFI& ffi, int32_t pluginIdx, int32_t groupIdx, int32_t recordIdx)
{
    QVector<ParsedSubrecord> out;

    if (ffi.xedit_record_subrecords_batch) {
        QByteArray buf(4 * 1024 * 1024, '\0');
        int32_t bytesOrErr = -1;

        for (;;) {
            bytesOrErr = ffi.xedit_record_subrecords_batch(
                pluginIdx, groupIdx, recordIdx,
                reinterpret_cast<uint8_t*>(buf.data()), buf.size());

            if (bytesOrErr < 0 && bytesOrErr < -100) {
                const int32_t needed = -bytesOrErr;
                if (needed > buf.size()) {
                    buf.resize(needed);
                    continue;
                }
            }
            break;
        }

        if (bytesOrErr >= 0) {
            const int32_t usedBytes = (bytesOrErr > 0 && bytesOrErr <= buf.size())
                ? bytesOrErr
                : buf.size();
            const uint8_t* ptr = reinterpret_cast<const uint8_t*>(buf.constData());
            const uint8_t* end = ptr + usedBytes;

            auto readI32 = [&](int32_t& outI32) -> bool {
                if (end - ptr < static_cast<ptrdiff_t>(sizeof(int32_t)))
                    return false;
                outI32 = qFromLittleEndian<int32_t>(ptr);
                ptr += sizeof(int32_t);
                return true;
            };

            int32_t packedCount = 0;
            bool parseOk = readI32(packedCount) && packedCount >= 0;
            if (parseOk)
                out.reserve(packedCount);

            for (int32_t i = 0; parseOk && i < packedCount; ++i) {
                if (end - ptr < 4) {
                    parseOk = false;
                    break;
                }

                ParsedSubrecord item;
                item.signature = QString::fromLatin1(reinterpret_cast<const char*>(ptr), 4);
                item.index = i;
                ptr += 4;

                int32_t dataSize = 0;
                if (!readI32(dataSize) || dataSize < 0) {
                    parseOk = false;
                    break;
                }
                if (end - ptr < dataSize) {
                    parseOk = false;
                    break;
                }

                item.fullSize = dataSize;
                const int32_t previewLen = qMin(dataSize, static_cast<int32_t>(256));
                if (previewLen > 0)
                    item.previewData = QByteArray(reinterpret_cast<const char*>(ptr), previewLen);
                ptr += dataSize;
                out.push_back(std::move(item));
            }

            if (parseOk)
                return out;

            out.clear();
        }
    }

    if (!ffi.xedit_record_subrecord_count)
        return out;

    const int32_t count = ffi.xedit_record_subrecord_count(pluginIdx, groupIdx, recordIdx);
    if (count <= 0)
        return out;

    out.reserve(count);
    for (int32_t i = 0; i < count; ++i) {
        ParsedSubrecord item;
        item.index = i;

        if (ffi.xedit_subrecord_signature) {
            item.signature = ffiString([&](char* b, int32_t len) {
                return ffi.xedit_subrecord_signature(pluginIdx, groupIdx, recordIdx, i, b, len);
            });
        }

        if (ffi.xedit_subrecord_size)
            item.fullSize = ffi.xedit_subrecord_size(pluginIdx, groupIdx, recordIdx, i);

        const int32_t previewLen = qMin(item.fullSize, static_cast<int32_t>(256));
        if (previewLen > 0 && ffi.xedit_subrecord_data) {
            item.previewData.resize(previewLen);
            int32_t bytesRead = ffi.xedit_subrecord_data(
                pluginIdx, groupIdx, recordIdx, i, item.previewData.data(), previewLen);
            if (bytesRead < 0)
                bytesRead = 0;
            item.previewData.truncate(bytesRead);
        }

        out.push_back(std::move(item));
    }

    return out;
}

} // namespace

OverrideViewModel::OverrideViewModel(QObject* parent)
    : QAbstractItemModel(parent)
    , m_root(new ViewNode)
{
}

OverrideViewModel::~OverrideViewModel()
{
    delete m_root;
    m_root = nullptr;
}

void OverrideViewModel::loadRecord(uint32_t formId)
{
    beginResetModel();
    m_currentFormId = formId;
    m_pluginNames.clear();
    m_pluginLocations.clear();
    if (!m_root)
        m_root = new ViewNode;
    qDeleteAll(m_root->children);
    m_root->children.clear();
    buildTree(formId);
    endResetModel();
}

void OverrideViewModel::clear()
{
    beginResetModel();
    m_currentFormId = 0;
    m_pluginNames.clear();
    m_pluginLocations.clear();
    if (!m_root)
        m_root = new ViewNode;
    qDeleteAll(m_root->children);
    m_root->children.clear();
    endResetModel();
}

QModelIndex OverrideViewModel::index(int row, int column, const QModelIndex& parent) const
{
    if (!m_root || row < 0 || column < 0)
        return {};

    ViewNode* parentNode = nodeFromIndex(parent);
    if (!parentNode)
        return {};
    if (row >= parentNode->children.size() || column >= columnCount(parent))
        return {};

    ViewNode* childNode = parentNode->children.at(row);
    return createIndex(row, column, childNode);
}

QModelIndex OverrideViewModel::parent(const QModelIndex& child) const
{
    if (!child.isValid())
        return {};

    ViewNode* childNode = nodeFromIndex(child);
    if (!childNode || childNode == m_root)
        return {};

    ViewNode* parentNode = childNode->parent;
    if (!parentNode || parentNode == m_root)
        return {};

    return createIndex(parentNode->row, 0, parentNode);
}

int OverrideViewModel::rowCount(const QModelIndex& parent) const
{
    if (!m_root)
        return 0;
    if (parent.column() > 0)
        return 0;

    ViewNode* node = nodeFromIndex(parent);
    if (!node)
        return 0;
    return node->children.size();
}

int OverrideViewModel::columnCount(const QModelIndex& parent) const
{
    Q_UNUSED(parent);
    return 1 + m_pluginNames.size();
}

bool OverrideViewModel::hasChildren(const QModelIndex& parent) const
{
    if (!m_root)
        return false;
    if (parent.column() > 0)
        return false;

    ViewNode* node = nodeFromIndex(parent);
    if (!node)
        return false;
    return !node->children.isEmpty();
}

QVariant OverrideViewModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid())
        return {};

    ViewNode* node = nodeFromIndex(index);
    if (!node || node == m_root)
        return {};

    const int col = index.column();
    if (role == Qt::DisplayRole) {
        if (col == 0)
            return node->fieldName;

        const int pluginCol = col - 1;
        if (pluginCol >= 0 && pluginCol < node->values.size())
            return node->values.at(pluginCol);
        return {};
    }

    if (role == ConflictAllRole)
        return node->conflictAll;

    if (role == ConflictThisRole) {
        const int pluginCol = col - 1;
        if (pluginCol >= 0 && pluginCol < node->conflictThis.size())
            return node->conflictThis.at(pluginCol);
        return 0;
    }

    return {};
}

QVariant OverrideViewModel::headerData(
    int section, Qt::Orientation orientation, int role) const
{
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole)
        return {};

    if (section == 0)
        return tr("Field");

    const int pluginCol = section - 1;
    if (pluginCol >= 0 && pluginCol < m_pluginNames.size())
        return m_pluginNames.at(pluginCol);

    return {};
}

Qt::ItemFlags OverrideViewModel::flags(const QModelIndex& index) const
{
    Qt::ItemFlags baseFlags = QAbstractItemModel::flags(index);
    if (!index.isValid())
        return baseFlags;

    // Plugin value columns (1..N) are editable
    const int col = index.column();
    if (col >= 1 && col <= m_pluginNames.size()) {
        ViewNode* node = nodeFromIndex(index);
        if (node && node != m_root && node->subrecordIndex >= 0) {
            const int pluginCol = col - 1;
            if (pluginCol < node->values.size() && !node->values.at(pluginCol).isEmpty())
                return baseFlags | Qt::ItemIsEditable;
        }
    }

    return baseFlags;
}

bool OverrideViewModel::setData(const QModelIndex& index, const QVariant& value,
                                 int role)
{
    if (!index.isValid() || role != Qt::EditRole)
        return false;

    const int col = index.column();
    if (col < 1 || col > m_pluginNames.size())
        return false;

    ViewNode* node = nodeFromIndex(index);
    if (!node || node == m_root || node->subrecordIndex < 0)
        return false;

    const int pluginCol = col - 1;
    if (pluginCol < 0 || pluginCol >= m_pluginLocations.size())
        return false;

    const QString newText = value.toString();
    if (pluginCol < node->values.size() && newText == node->values.at(pluginCol))
        return false; // no change

    // Convert text to UTF-8 with null terminator
    QByteArray utf8 = newText.toUtf8();
    utf8.append('\0');

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_set_subrecord_data) {
        qWarning() << "OverrideViewModel::setData: xedit_set_subrecord_data not available";
        return false;
    }

    const auto& loc = m_pluginLocations.at(pluginCol);
    const int32_t result = ffi.xedit_set_subrecord_data(
        loc.pluginIdx, loc.groupIdx, loc.recordIdx,
        node->subrecordIndex,
        reinterpret_cast<const uint8_t*>(utf8.constData()),
        utf8.size());

    if (result != 0) {
        qWarning() << "OverrideViewModel::setData: FFI write failed with code" << result
                   << "for subrecord" << node->signature << "plugin column" << pluginCol;
        return false;
    }

    // Update in-memory value
    if (pluginCol < node->values.size())
        node->values[pluginCol] = newText;

    emit dataChanged(index, index, {Qt::DisplayRole, Qt::EditRole});
    return true;
}

ViewNode* OverrideViewModel::nodeFromIndex(const QModelIndex& index) const
{
    if (!index.isValid())
        return m_root;
    return static_cast<ViewNode*>(index.internalPointer());
}

void OverrideViewModel::buildTree(uint32_t formId)
{
    auto& ffi = XEditFFI::instance();
    if (!m_root || !ffi.xedit_find_overrides || !ffi.xedit_search_form_id)
        return;

    QVector<int32_t> overridePluginIndices(16, -1);
    int32_t overrideCount = ffi.xedit_find_overrides(
        nullptr,
        formId,
        reinterpret_cast<char*>(overridePluginIndices.data()),
        overridePluginIndices.size());

    if (overrideCount < 0)
        return;

    if (overrideCount > overridePluginIndices.size()) {
        overridePluginIndices.resize(overrideCount);
        overrideCount = ffi.xedit_find_overrides(
            nullptr,
            formId,
            reinterpret_cast<char*>(overridePluginIndices.data()),
            overridePluginIndices.size());
        if (overrideCount < 0)
            return;
    }

    overridePluginIndices.resize(qMin(overrideCount, overridePluginIndices.size()));
    if (overridePluginIndices.isEmpty())
        return;

    QVector<QVector<ParsedSubrecord>> subrecordsByPlugin;
    subrecordsByPlugin.reserve(overridePluginIndices.size());

    m_pluginLocations.clear();
    m_pluginLocations.reserve(overridePluginIndices.size());

    for (int32_t pluginIdx : overridePluginIndices) {
        int32_t groupIdx = -1;
        int32_t recordIdx = -1;
        if (ffi.xedit_search_form_id(pluginIdx, formId, &groupIdx, &recordIdx) < 0
            || groupIdx < 0 || recordIdx < 0) {
            continue;
        }

        QString pluginName = QString::number(pluginIdx);
        if (ffi.xedit_plugin_filename) {
            const QString name = ffiString(ffi.xedit_plugin_filename, pluginIdx);
            if (!name.isEmpty())
                pluginName = name;
        }

        m_pluginNames.push_back(pluginName);
        m_pluginLocations.push_back({pluginIdx, groupIdx, recordIdx});
        subrecordsByPlugin.push_back(
            loadSubrecordsForRecord(ffi, pluginIdx, groupIdx, recordIdx));
    }

    if (m_pluginLocations.isEmpty())
        return;

    QHash<QString, ViewNode*> rowsByKey;
    QVector<ViewNode*> orderedRows;
    orderedRows.reserve(64);

    for (int p = 0; p < subrecordsByPlugin.size(); ++p) {
        const QVector<ParsedSubrecord>& subrecords = subrecordsByPlugin.at(p);
        QHash<QString, int> signatureCounts;

        for (const ParsedSubrecord& sub : subrecords) {
            const int occurrence = signatureCounts.value(sub.signature, 0) + 1;
            signatureCounts.insert(sub.signature, occurrence);

            const QString key = QStringLiteral("%1:%2").arg(sub.signature).arg(occurrence);
            ViewNode* node = rowsByKey.value(key, nullptr);
            if (!node) {
                node = new ViewNode;
                node->signature = sub.signature;
                node->fieldName = fieldLabelForSignature(sub.signature);
                node->subrecordIndex = sub.index;
                node->values = QVector<QString>(m_pluginNames.size());
                node->conflictThis = QVector<int>(m_pluginNames.size(), 0);
                node->parent = m_root;
                node->row = m_root->children.size();
                m_root->children.push_back(node);
                rowsByKey.insert(key, node);
                orderedRows.push_back(node);
            }

            if (p >= 0 && p < node->values.size()) {
                QString decoded;
                if (ffi.xedit_subrecord_display_value) {
                    const auto& loc = m_pluginLocations.at(p);
                    decoded = ffiString([&](char* buf, int32_t len) {
                        return ffi.xedit_subrecord_display_value(
                            loc.pluginIdx, loc.groupIdx, loc.recordIdx,
                            sub.index,
                            reinterpret_cast<uint8_t*>(buf),
                            len);
                    });
                }
                const QString displayValue = decoded.isEmpty()
                    ? hexPreview(sub.previewData, sub.fullSize)
                    : decoded;

                // If decoded value has multiple fields separated by " | ",
                // create child nodes for each field
                if (!decoded.isEmpty() && decoded.contains(QStringLiteral(" | "))) {
                    const QStringList fields = decoded.split(QStringLiteral(" | "));

                    if (node->children.isEmpty()) {
                        // First plugin with multi-field data creates child structure
                        for (int f = 0; f < fields.size(); ++f) {
                            auto* child = new ViewNode;
                            const QString& fieldStr = fields.at(f).trimmed();
                            int colonPos = fieldStr.indexOf(QStringLiteral(": "));
                            if (colonPos > 0) {
                                child->fieldName = fieldStr.left(colonPos);
                                child->values = QVector<QString>(m_pluginNames.size());
                                child->conflictThis = QVector<int>(m_pluginNames.size(), 0);
                                child->parent = node;
                                child->row = f;
                                child->values[p] = fieldStr.mid(colonPos + 2);
                            } else {
                                child->fieldName = QStringLiteral("Field %1").arg(f + 1);
                                child->values = QVector<QString>(m_pluginNames.size());
                                child->conflictThis = QVector<int>(m_pluginNames.size(), 0);
                                child->parent = node;
                                child->row = f;
                                child->values[p] = fieldStr;
                            }
                            node->children.push_back(child);
                        }
                    } else {
                        // Fill in values for subsequent plugins, matching by field name
                        for (int f = 0; f < fields.size(); ++f) {
                            const QString& fieldStr = fields.at(f).trimmed();
                            QString fieldName;
                            QString fieldValue;
                            int colonPos = fieldStr.indexOf(QStringLiteral(": "));
                            if (colonPos > 0) {
                                fieldName = fieldStr.left(colonPos);
                                fieldValue = fieldStr.mid(colonPos + 2);
                            } else {
                                fieldValue = fieldStr;
                            }

                            // Try to match by field name first
                            ViewNode* target = nullptr;
                            if (!fieldName.isEmpty()) {
                                for (ViewNode* child : node->children) {
                                    if (child->fieldName == fieldName) {
                                        target = child;
                                        break;
                                    }
                                }
                            }
                            // Fall back to positional match
                            if (!target && f < node->children.size())
                                target = node->children[f];

                            if (target)
                                target->values[p] = fieldValue;
                        }
                    }
                    // Parent shows empty — user expands to see individual fields
                    node->values[p] = QString();
                } else {
                    node->values[p] = displayValue;
                }
            }
        }
    }

    // Helper lambda to compute conflict colors for a node using proper enum values
    auto computeConflicts = [](ViewNode* n) {
        // Collect non-empty values and find master (first) and winning (last) values
        QSet<QString> distinct;
        int filledCount = 0;
        QString masterValue;
        int masterIdx = -1;
        QString winningValue;
        int winningIdx = -1;

        for (int i = 0; i < n->values.size(); ++i) {
            const QString& v = n->values.at(i);
            if (!v.isEmpty()) {
                distinct.insert(v);
                filledCount++;
                if (masterIdx < 0) {
                    masterIdx = i;
                    masterValue = v;
                }
                winningIdx = i;
                winningValue = v;
            }
        }

        // Determine ConflictAll (row-level)
        if (filledCount == 0) {
            n->conflictAll = static_cast<int>(ConflictAll::Unknown);
        } else if (filledCount == 1) {
            n->conflictAll = static_cast<int>(ConflictAll::OnlyOne);
        } else if (distinct.size() == 1) {
            n->conflictAll = static_cast<int>(ConflictAll::NoConflict);
        } else {
            n->conflictAll = static_cast<int>(ConflictAll::Override);
        }

        // Determine ConflictThis (per-cell)
        for (int i = 0; i < n->values.size(); ++i) {
            const QString& value = n->values.at(i);
            if (value.isEmpty()) {
                n->conflictThis[i] = static_cast<int>(ConflictThis::NotDefined);
            } else if (i == masterIdx) {
                n->conflictThis[i] = static_cast<int>(ConflictThis::Master);
            } else if (filledCount == 1) {
                n->conflictThis[i] = static_cast<int>(ConflictThis::OnlyOne);
            } else if (distinct.size() == 1) {
                // All values identical
                n->conflictThis[i] = static_cast<int>(ConflictThis::IdenticalToMaster);
            } else if (value == winningValue && value == masterValue) {
                n->conflictThis[i] = static_cast<int>(ConflictThis::IdenticalToMasterWinsConflict);
            } else if (value == winningValue) {
                n->conflictThis[i] = static_cast<int>(ConflictThis::Override);
            } else if (value == masterValue) {
                n->conflictThis[i] = static_cast<int>(ConflictThis::IdenticalToMaster);
            } else {
                n->conflictThis[i] = static_cast<int>(ConflictThis::ConflictLoses);
            }
        }
    };

    int totalChildren = 0;
    for (ViewNode* node : orderedRows) {
        computeConflicts(node);

        // Also compute conflicts for child nodes (multi-field subrecords)
        for (ViewNode* child : node->children)
            computeConflicts(child);

        if (!node->children.isEmpty()) {
            totalChildren += node->children.size();
            qDebug() << "OverrideViewModel: node" << node->fieldName
                     << "has" << node->children.size() << "child fields";
        }
    }
    if (totalChildren > 0)
        qDebug() << "OverrideViewModel: total expandable children:" << totalChildren;
    else
        qDebug() << "OverrideViewModel: no expandable children found (no ' | ' in decoded values)";
}
