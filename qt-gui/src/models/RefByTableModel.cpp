#include "models/RefByTableModel.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"

#include <cstring>

RefByTableModel::RefByTableModel(QObject* parent)
    : QAbstractTableModel(parent)
{
}

void RefByTableModel::setRecord(int pluginIdx, int groupIdx, int recordIdx)
{
    auto& ffi = XEditFFI::instance();

    if (!ffi.xedit_record_refby_count) {
        clear();
        return;
    }

    int32_t count = ffi.xedit_record_refby_count(pluginIdx, groupIdx, recordIdx);
    if (count <= 0) {
        clear();
        return;
    }

    beginResetModel();
    m_entries.clear();
    m_entries.reserve(count);

    // Fast path: batch function returns all entries + metadata in one FFI call
    if (ffi.xedit_record_refby_batch) {
        // Allocate generous buffer: ~512 bytes per entry should be plenty
        const int32_t bufSize = count * 512;
        QByteArray buf(bufSize, Qt::Uninitialized);

        int32_t got = ffi.xedit_record_refby_batch(
            pluginIdx, groupIdx, recordIdx,
            reinterpret_cast<uint8_t*>(buf.data()), bufSize);

        if (got > 0) {
            const uint8_t* ptr = reinterpret_cast<const uint8_t*>(buf.constData());
            const uint8_t* end = ptr + bufSize;

            for (int32_t i = 0; i < got; ++i) {
                // Each entry: plugin_idx(4) + group_idx(4) + record_idx(4) + form_id(4)
                //           + sig_len(2) + sig(N) + edid_len(2) + edid(N) + fname_len(2) + fname(N)
                if (ptr + 16 > end) break;  // need at least the fixed fields

                RefEntry entry;
                auto readI32 = [&]() -> int32_t {
                    int32_t v;
                    memcpy(&v, ptr, 4);
                    ptr += 4;
                    return v;
                };
                auto readU32 = [&]() -> uint32_t {
                    uint32_t v;
                    memcpy(&v, ptr, 4);
                    ptr += 4;
                    return v;
                };
                auto readU16 = [&]() -> uint16_t {
                    uint16_t v;
                    memcpy(&v, ptr, 2);
                    ptr += 2;
                    return v;
                };
                auto readStr = [&]() -> QString {
                    if (ptr + 2 > end) return {};
                    uint16_t len = readU16();
                    if (ptr + len > end) return {};
                    QString s = QString::fromUtf8(reinterpret_cast<const char*>(ptr), len);
                    ptr += len;
                    return s;
                };

                entry.pluginIdx = readI32();
                entry.groupIdx  = readI32();
                entry.recordIdx = readI32();
                entry.formId    = readU32();
                entry.signature = readStr();
                entry.editorId  = readStr();
                entry.filename  = readStr();

                m_entries.append(std::move(entry));
            }

            endResetModel();
            return;
        }
        // If batch returned error, fall through to per-entry path
    }

    // Slow path: per-entry FFI calls (fallback if batch not available)
    for (int32_t i = 0; i < count; ++i) {
        int32_t outPlugin = 0, outGroup = 0, outRecord = 0;
        int32_t ok = ffi.xedit_record_refby_entry(
            pluginIdx, groupIdx, recordIdx, i,
            &outPlugin, &outGroup, &outRecord);
        if (ok < 0)
            continue;

        RefEntry entry;
        entry.pluginIdx = outPlugin;
        entry.groupIdx  = outGroup;
        entry.recordIdx = outRecord;

        entry.editorId = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_record_editor_id(outPlugin, outGroup, outRecord, buf, len);
        });

        entry.signature = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_record_signature(outPlugin, outGroup, outRecord, buf, len);
        });

        entry.formId = ffi.xedit_record_form_id(outPlugin, outGroup, outRecord);

        entry.filename = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_plugin_filename(outPlugin, buf, len);
        });

        m_entries.append(std::move(entry));
    }

    endResetModel();
}

void RefByTableModel::clear()
{
    beginResetModel();
    m_entries.clear();
    endResetModel();
}

int RefByTableModel::rowCount(const QModelIndex& parent) const
{
    if (parent.isValid())
        return 0;
    return m_entries.size();
}

int RefByTableModel::columnCount(const QModelIndex& parent) const
{
    if (parent.isValid())
        return 0;
    return ColumnCount;
}

QVariant RefByTableModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() >= m_entries.size())
        return {};

    if (role != Qt::DisplayRole)
        return {};

    const auto& entry = m_entries[index.row()];

    switch (index.column()) {
    case ColRecord:
        return QStringLiteral("[%1] %2")
            .arg(entry.formId, 8, 16, QLatin1Char('0'))
            .arg(entry.editorId)
            .toUpper();
    case ColSignature:
        return entry.signature;
    case ColFormID:
        return QStringLiteral("%1").arg(entry.formId, 8, 16, QLatin1Char('0')).toUpper();
    case ColFile:
        return entry.filename;
    default:
        return {};
    }
}

QVariant RefByTableModel::headerData(int section, Qt::Orientation orientation, int role) const
{
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole)
        return {};

    switch (section) {
    case ColRecord:    return QStringLiteral("Record");
    case ColSignature: return QStringLiteral("Signature");
    case ColFormID:    return QStringLiteral("FormID");
    case ColFile:      return QStringLiteral("File");
    default:           return {};
    }
}

const RefByTableModel::RefEntry* RefByTableModel::entryAt(int row) const
{
    if (row < 0 || row >= m_entries.size())
        return nullptr;
    return &m_entries[row];
}
