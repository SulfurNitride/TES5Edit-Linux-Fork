#include "RecordViewModel.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"

#include <QDebug>
#include <QtEndian>
#include <QStringEncoder>
#include <QSet>

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

RecordViewModel::RecordViewModel(QObject* parent)
    : QAbstractTableModel(parent)
{
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

void RecordViewModel::setRecord(int pluginIdx, int groupIdx, int recordIdx)
{
    m_pluginIdx  = pluginIdx;
    m_groupIdx   = groupIdx;
    m_recordIdx  = recordIdx;
    loadSubrecords();
    emit layoutChanged();
}

void RecordViewModel::clear()
{
    beginResetModel();
    m_pluginIdx  = -1;
    m_groupIdx   = -1;
    m_recordIdx  = -1;
    m_subrecords.clear();
    endResetModel();
}

// ---------------------------------------------------------------------------
// QAbstractTableModel interface
// ---------------------------------------------------------------------------

int RecordViewModel::rowCount(const QModelIndex& parent) const
{
    if (parent.isValid())
        return 0;
    return m_subrecords.size();
}

int RecordViewModel::columnCount(const QModelIndex& parent) const
{
    if (parent.isValid())
        return 0;
    return ColumnCount;
}

QVariant RecordViewModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid())
        return {};

    const int row = index.row();
    const int col = index.column();

    if (row < 0 || row >= m_subrecords.size())
        return {};

    const SubrecordInfo& sub = m_subrecords.at(row);

    if (role == Qt::DisplayRole) {
        switch (col) {
        case ColSignature:
            return sub.signature;
        case ColSize:
            return QString::number(sub.size);
        case ColData: {
            QString hex = sub.rawData.toHex(' ').toUpper();
            if (sub.size > 256)
                hex += QStringLiteral(" ...");
            return hex;
        }
        case ColText:
            return sub.textPreview;
        default:
            return {};
        }
    }

    if (role == Qt::ToolTipRole && col == ColData) {
        // Show full hex for truncated data; for small data the tooltip
        // matches the display text so Qt will suppress it automatically.
        if (sub.size > 256) {
            return QStringLiteral("Showing first 256 of %1 bytes").arg(sub.size);
        }
        return sub.rawData.toHex(' ').toUpper();
    }

    return {};
}

QVariant RecordViewModel::headerData(int section, Qt::Orientation orientation,
                                     int role) const
{
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole)
        return {};

    switch (section) {
    case ColSignature: return tr("Signature");
    case ColSize:      return tr("Size");
    case ColData:      return tr("Data (Hex)");
    case ColText:      return tr("Text");
    default:           return {};
    }
}

Qt::ItemFlags RecordViewModel::flags(const QModelIndex& index) const
{
    Qt::ItemFlags baseFlags = QAbstractTableModel::flags(index);
    if (!index.isValid())
        return baseFlags;

    // Only the Text column (ColText) is editable, and only for text-bearing
    // subrecords that actually have decoded text content.
    if (index.column() == ColText) {
        const int row = index.row();
        if (row >= 0 && row < m_subrecords.size()) {
            const SubrecordInfo& sub = m_subrecords.at(row);
            if (isTextSubrecord(sub.signature) && !sub.textPreview.isEmpty())
                return baseFlags | Qt::ItemIsEditable;
        }
    }

    return baseFlags;
}

bool RecordViewModel::setData(const QModelIndex& index, const QVariant& value,
                              int role)
{
    if (!index.isValid() || role != Qt::EditRole)
        return false;

    if (index.column() != ColText)
        return false;

    const int row = index.row();
    if (row < 0 || row >= m_subrecords.size())
        return false;

    const SubrecordInfo& sub = m_subrecords.at(row);
    const QString newText = value.toString();

    // Log the edit attempt -- actual FFI write support will be added later
    qDebug() << "RecordViewModel::setData: edit attempted on subrecord"
             << sub.signature << "row" << row
             << "plugin" << m_pluginIdx << "group" << m_groupIdx
             << "record" << m_recordIdx
             << "newValue:" << newText;

    // TODO: Call FFI write function once available, e.g.:
    //   ffi.xedit_subrecord_set_text(m_pluginIdx, m_groupIdx, m_recordIdx, row,
    //                                newText.toUtf8().constData());
    // For now, update the in-memory preview so the UI reflects the change.
    m_subrecords[row].textPreview = newText;
    emit dataChanged(index, index, {Qt::DisplayRole, Qt::EditRole});
    return true;
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

void RecordViewModel::loadSubrecords()
{
    beginResetModel();
    m_subrecords.clear();

    auto& ffi = XEditFFI::instance();

    if (m_pluginIdx < 0 || !ffi.xedit_record_subrecord_count) {
        endResetModel();
        return;
    }

    const int32_t count = ffi.xedit_record_subrecord_count(
        m_pluginIdx, m_groupIdx, m_recordIdx);

    if (count <= 0) {
        endResetModel();
        return;
    }

    m_subrecords.reserve(count);

    const auto loadLegacyLoop = [&]() {
        for (int32_t i = 0; i < count; ++i) {
            SubrecordInfo info;

            // Signature
            info.signature = ffiString(
                [&](char* buf, int32_t len) {
                    return ffi.xedit_subrecord_signature(
                        m_pluginIdx, m_groupIdx, m_recordIdx, i, buf, len);
                });

            // Size
            info.size = ffi.xedit_subrecord_size
                ? ffi.xedit_subrecord_size(m_pluginIdx, m_groupIdx, m_recordIdx, i)
                : 0;

            // Raw data -- read up to 256 bytes
            const int32_t readLen = qMin(info.size, static_cast<int32_t>(256));
            if (readLen > 0 && ffi.xedit_subrecord_data) {
                info.rawData.resize(readLen);
                int32_t bytesRead = ffi.xedit_subrecord_data(
                    m_pluginIdx, m_groupIdx, m_recordIdx, i,
                    info.rawData.data(), readLen);
                if (bytesRead < 0)
                    bytesRead = 0;
                info.rawData.truncate(bytesRead);
            }

            // Attempt text decode for known text subrecords
            if (isTextSubrecord(info.signature)) {
                info.textPreview = tryDecodeText(info.rawData);
            }

            m_subrecords.append(std::move(info));
        }
    };

    if (ffi.xedit_record_subrecords_batch) {
        QByteArray buf(4 * 1024 * 1024, '\0'); // 4MB initial buffer
        int32_t bytesOrErr = -1;

        for (;;) {
            bytesOrErr = ffi.xedit_record_subrecords_batch(
                m_pluginIdx, m_groupIdx, m_recordIdx,
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

            auto readI32 = [&](int32_t& out) -> bool {
                if (end - ptr < static_cast<ptrdiff_t>(sizeof(int32_t)))
                    return false;
                out = qFromLittleEndian<int32_t>(ptr);
                ptr += sizeof(int32_t);
                return true;
            };

            int32_t packedCount = 0;
            bool parseOk = readI32(packedCount) && packedCount >= 0;
            if (parseOk)
                m_subrecords.reserve(packedCount);

            for (int32_t i = 0; parseOk && i < packedCount; ++i) {
                if (end - ptr < 4) {
                    parseOk = false;
                    break;
                }

                SubrecordInfo info;
                info.signature = QString::fromLatin1(reinterpret_cast<const char*>(ptr), 4);
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

                info.size = dataSize;
                const int32_t previewLen = qMin(dataSize, static_cast<int32_t>(256));
                if (previewLen > 0)
                    info.rawData = QByteArray(reinterpret_cast<const char*>(ptr), previewLen);

                if (isTextSubrecord(info.signature))
                    info.textPreview = tryDecodeText(info.rawData);

                ptr += dataSize;
                m_subrecords.append(std::move(info));
            }

            if (!parseOk) {
                m_subrecords.clear();
                loadLegacyLoop();
            }
        } else {
            loadLegacyLoop();
        }
    } else {
        loadLegacyLoop();
    }

    endResetModel();
}

QString RecordViewModel::tryDecodeText(const QByteArray& data)
{
    if (data.isEmpty())
        return {};

    // Strip trailing null bytes (common in Bethesda text fields)
    QByteArray trimmed = data;
    while (!trimmed.isEmpty() && trimmed.back() == '\0')
        trimmed.chop(1);

    if (trimmed.isEmpty())
        return {};

    // Try UTF-8 decode
    auto toUtf16 = QStringDecoder(QStringDecoder::Utf8);
    QString result = toUtf16(trimmed);

    if (toUtf16.hasError())
        return {};

    // Reject strings that contain non-printable control chars (except newline/tab)
    for (const QChar& ch : result) {
        if (ch.unicode() < 0x20 && ch != u'\n' && ch != u'\r' && ch != u'\t')
            return {};
    }

    return result;
}

bool RecordViewModel::isTextSubrecord(const QString& sig)
{
    // Static set of known text-bearing subrecord signatures
    static const QSet<QString> textSigs = {
        QStringLiteral("EDID"),
        QStringLiteral("FULL"),
        QStringLiteral("DESC"),
        QStringLiteral("MODL"),
        QStringLiteral("ICON"),
        QStringLiteral("MICO"),
        QStringLiteral("TX00"),
        QStringLiteral("TX01"),
        QStringLiteral("TX02"),
        QStringLiteral("TX03"),
        QStringLiteral("TX04"),
        QStringLiteral("TX05"),
        QStringLiteral("TX06"),
        QStringLiteral("TX07"),
        QStringLiteral("NNAM"),
        QStringLiteral("ANAM"),
        QStringLiteral("BNAM"),
        QStringLiteral("CNAM"),
        QStringLiteral("DNAM"),
        QStringLiteral("ENAM"),
        QStringLiteral("FNAM"),
        QStringLiteral("ONAM"),
        QStringLiteral("INAM"),
        QStringLiteral("RNAM"),
        QStringLiteral("SNAM"),
        QStringLiteral("TNAM"),
        QStringLiteral("WNAM"),
        QStringLiteral("XNAM"),
        QStringLiteral("YNAM"),
        QStringLiteral("ZNAM"),
        QStringLiteral("MOD2"),
        QStringLiteral("MOD3"),
        QStringLiteral("MOD4"),
        QStringLiteral("MOD5"),
        QStringLiteral("ICO2"),
        QStringLiteral("NAM0"),
        QStringLiteral("NAM1"),
        QStringLiteral("NAM2"),
        QStringLiteral("SHRT"),
    };
    return textSigs.contains(sig);
}
