#include "models/SpreadsheetModel.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

SpreadsheetModel::SpreadsheetModel(QObject* parent)
    : QAbstractTableModel(parent)
{
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

void SpreadsheetModel::loadRecords(const QString& signature)
{
    beginResetModel();
    m_rows.clear();
    m_extraColumns.clear();
    m_extraHeaders.clear();
    m_signature = signature;

    buildExtraColumns(signature);

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_plugin_count || !ffi.xedit_plugin_group_count ||
        !ffi.xedit_group_signature || !ffi.xedit_group_record_count ||
        !ffi.xedit_record_form_id) {
        endResetModel();
        return;
    }

    const int32_t pluginCount = ffi.xedit_plugin_count();

    for (int32_t p = 0; p < pluginCount; ++p) {
        const QString pluginName = ffiString(ffi.xedit_plugin_filename, p);
        const int32_t groupCount = ffi.xedit_plugin_group_count(p);

        for (int32_t g = 0; g < groupCount; ++g) {
            // Check if this group's signature matches the requested one
            const QString groupSig = ffiString([&](char* buf, int32_t len) {
                return ffi.xedit_group_signature(p, g, buf, len);
            });

            if (groupSig != signature)
                continue;

            const int32_t recordCount = ffi.xedit_group_record_count(p, g);
            m_rows.reserve(m_rows.size() + recordCount);

            for (int32_t r = 0; r < recordCount; ++r) {
                RowData row;
                row.pluginIdx = p;
                row.groupIdx  = g;
                row.recordIdx = r;
                row.pluginName = pluginName;

                row.formId = ffi.xedit_record_form_id(p, g, r);

                if (ffi.xedit_record_editor_id) {
                    row.editorId = ffiString([&](char* buf, int32_t len) {
                        return ffi.xedit_record_editor_id(p, g, r, buf, len);
                    });
                }

                // Read the FULL subrecord for the display name
                row.name = readSubrecordText(p, g, r, QStringLiteral("FULL"));

                // Populate signature-specific extra columns
                row.extraFields.resize(m_extraColumns.size());
                for (int i = 0; i < m_extraColumns.size(); ++i) {
                    const auto& col = m_extraColumns[i];
                    row.extraFields[i] = readSubrecordText(p, g, r, col.subrecordSig);
                }

                m_rows.append(std::move(row));
            }
        }
    }

    endResetModel();
}

void SpreadsheetModel::clear()
{
    beginResetModel();
    m_signature.clear();
    m_rows.clear();
    m_extraColumns.clear();
    m_extraHeaders.clear();
    endResetModel();
}

bool SpreadsheetModel::recordAt(int row, RecordLocation& out) const
{
    if (row < 0 || row >= m_rows.size())
        return false;
    const auto& r = m_rows[row];
    out.pluginIdx  = r.pluginIdx;
    out.groupIdx   = r.groupIdx;
    out.recordIdx  = r.recordIdx;
    return true;
}

// ---------------------------------------------------------------------------
// QAbstractTableModel interface
// ---------------------------------------------------------------------------

int SpreadsheetModel::rowCount(const QModelIndex& parent) const
{
    if (parent.isValid())
        return 0;
    return m_rows.size();
}

int SpreadsheetModel::columnCount(const QModelIndex& parent) const
{
    if (parent.isValid())
        return 0;
    return FixedColumnCount + m_extraColumns.size();
}

QVariant SpreadsheetModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid())
        return {};

    const int row = index.row();
    const int col = index.column();

    if (row < 0 || row >= m_rows.size())
        return {};

    const auto& entry = m_rows[row];

    if (role == Qt::DisplayRole) {
        switch (col) {
        case ColPlugin:
            return entry.pluginName;
        case ColFormID:
            return QStringLiteral("%1")
                .arg(entry.formId, 8, 16, QLatin1Char('0'))
                .toUpper();
        case ColEditorID:
            return entry.editorId;
        case ColName:
            return entry.name;
        default: {
            const int extraIdx = col - FixedColumnCount;
            if (extraIdx >= 0 && extraIdx < entry.extraFields.size())
                return entry.extraFields[extraIdx];
            return {};
        }
        }
    }

    if (role == Qt::ToolTipRole) {
        if (col == ColFormID) {
            return QStringLiteral("FormID: 0x%1")
                .arg(entry.formId, 8, 16, QLatin1Char('0'))
                .toUpper();
        }
        if (col == ColEditorID && !entry.editorId.isEmpty()) {
            return entry.editorId;
        }
    }

    return {};
}

QVariant SpreadsheetModel::headerData(int section, Qt::Orientation orientation,
                                      int role) const
{
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole)
        return {};

    switch (section) {
    case ColPlugin:   return tr("Plugin");
    case ColFormID:   return tr("FormID");
    case ColEditorID: return tr("EditorID");
    case ColName:     return tr("Name");
    default: {
        const int extraIdx = section - FixedColumnCount;
        if (extraIdx >= 0 && extraIdx < m_extraHeaders.size())
            return m_extraHeaders[extraIdx];
        return {};
    }
    }
}

// ---------------------------------------------------------------------------
// Signature-specific column definitions
// ---------------------------------------------------------------------------
// These mirror the column layouts from the Delphi spreadsheet tabs in
// xeMainForm.pas (vstSpreadSheetWeaponInitNode, etc.).  The subrecord
// signatures listed here are the top-level subrecords whose text value is
// displayed.  Structured sub-elements inside DATA/DNAM cannot be individually
// addressed without deeper element-path FFI support, so we show the whole
// subrecord text where applicable.

void SpreadsheetModel::buildExtraColumns(const QString& signature)
{
    m_extraColumns.clear();
    m_extraHeaders.clear();

    auto addCol = [this](const QString& header, const QString& subSig) {
        ExtraColumnDef def;
        def.header = header;
        def.subrecordSig = subSig;
        def.elementIndex = -1;
        m_extraColumns.append(def);
        m_extraHeaders.append(header);
    };

    if (signature == QLatin1String("WEAP")) {
        // Matches Delphi columns: Enchantment, EquipType, Speed, Reach,
        // Value, Weight, Damage, EnchAmount, Skill, Stagger, CritData
        addCol(tr("Enchantment"), QStringLiteral("EITM"));
        addCol(tr("Equip Type"),  QStringLiteral("ETYP"));
        addCol(tr("DATA"),        QStringLiteral("DATA"));
        addCol(tr("DNAM"),        QStringLiteral("DNAM"));
        addCol(tr("CRDT"),        QStringLiteral("CRDT"));
        addCol(tr("Ench. Amount"),QStringLiteral("EAMT"));
        addCol(tr("Description"), QStringLiteral("DESC"));
    } else if (signature == QLatin1String("ARMO")) {
        // Matches Delphi columns: Enchantment, Body Template, Value, etc.
        addCol(tr("Enchantment"), QStringLiteral("EITM"));
        addCol(tr("Body Template"), QStringLiteral("BOD2"));
        addCol(tr("DATA"),        QStringLiteral("DATA"));
        addCol(tr("DNAM"),        QStringLiteral("DNAM"));
        addCol(tr("Description"), QStringLiteral("DESC"));
    } else if (signature == QLatin1String("AMMO")) {
        // Matches Delphi columns: Projectile, Value, Damage, Flags
        addCol(tr("DATA"),        QStringLiteral("DATA"));
        addCol(tr("Description"), QStringLiteral("DESC"));
    } else if (signature == QLatin1String("ALCH")) {
        addCol(tr("DATA"),        QStringLiteral("DATA"));
        addCol(tr("ENIT"),        QStringLiteral("ENIT"));
        addCol(tr("Description"), QStringLiteral("DESC"));
    } else if (signature == QLatin1String("INGR")) {
        addCol(tr("DATA"),        QStringLiteral("DATA"));
        addCol(tr("ENIT"),        QStringLiteral("ENIT"));
    } else if (signature == QLatin1String("BOOK")) {
        addCol(tr("DATA"),        QStringLiteral("DATA"));
        addCol(tr("Description"), QStringLiteral("DESC"));
    } else if (signature == QLatin1String("MISC")) {
        addCol(tr("DATA"),        QStringLiteral("DATA"));
    } else if (signature == QLatin1String("NPC_")) {
        addCol(tr("ACBS"),        QStringLiteral("ACBS"));
        addCol(tr("Race"),        QStringLiteral("RNAM"));
        addCol(tr("Class"),       QStringLiteral("CNAM"));
    } else if (signature == QLatin1String("SPELL") || signature == QLatin1String("SPEL")) {
        addCol(tr("SPIT"),        QStringLiteral("SPIT"));
        addCol(tr("Description"), QStringLiteral("DESC"));
    } else {
        // Generic fallback: show DATA and DESC if present
        addCol(tr("DATA"),        QStringLiteral("DATA"));
        addCol(tr("Description"), QStringLiteral("DESC"));
    }
}

// ---------------------------------------------------------------------------
// Subrecord helpers
// ---------------------------------------------------------------------------
// These iterate through a record's subrecords to find one whose 4-char
// signature matches |targetSig|.  This is necessary because the FFI layer
// does not currently expose a "find subrecord by signature" function.

QString SpreadsheetModel::readSubrecordText(int pluginIdx, int groupIdx,
                                            int recordIdx,
                                            const QString& targetSig)
{
    auto& ffi = XEditFFI::instance();

    if (!ffi.xedit_record_subrecord_count || !ffi.xedit_subrecord_signature ||
        !ffi.xedit_subrecord_data || !ffi.xedit_subrecord_size)
        return {};

    const int32_t subCount = ffi.xedit_record_subrecord_count(
        pluginIdx, groupIdx, recordIdx);

    for (int32_t s = 0; s < subCount; ++s) {
        const QString sig = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_subrecord_signature(pluginIdx, groupIdx,
                                                 recordIdx, s, buf, len);
        });

        if (sig != targetSig)
            continue;

        // Found it -- read the raw data and attempt text decode
        const int32_t size = ffi.xedit_subrecord_size(
            pluginIdx, groupIdx, recordIdx, s);
        if (size <= 0)
            return {};

        const int32_t readLen = qMin(size, static_cast<int32_t>(4096));
        QByteArray raw(readLen, Qt::Uninitialized);
        const int32_t bytesRead = ffi.xedit_subrecord_data(
            pluginIdx, groupIdx, recordIdx, s,
            raw.data(), readLen);
        if (bytesRead <= 0)
            return {};
        raw.truncate(bytesRead);

        // Strip trailing nulls (standard Bethesda text field padding)
        while (!raw.isEmpty() && raw.back() == '\0')
            raw.chop(1);

        if (raw.isEmpty())
            return {};

        return QString::fromUtf8(raw);
    }

    return {};
}

QByteArray SpreadsheetModel::readSubrecordRaw(int pluginIdx, int groupIdx,
                                              int recordIdx,
                                              const QString& targetSig)
{
    auto& ffi = XEditFFI::instance();

    if (!ffi.xedit_record_subrecord_count || !ffi.xedit_subrecord_signature ||
        !ffi.xedit_subrecord_data || !ffi.xedit_subrecord_size)
        return {};

    const int32_t subCount = ffi.xedit_record_subrecord_count(
        pluginIdx, groupIdx, recordIdx);

    for (int32_t s = 0; s < subCount; ++s) {
        const QString sig = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_subrecord_signature(pluginIdx, groupIdx,
                                                 recordIdx, s, buf, len);
        });

        if (sig != targetSig)
            continue;

        const int32_t size = ffi.xedit_subrecord_size(
            pluginIdx, groupIdx, recordIdx, s);
        if (size <= 0)
            return {};

        QByteArray raw(size, Qt::Uninitialized);
        const int32_t bytesRead = ffi.xedit_subrecord_data(
            pluginIdx, groupIdx, recordIdx, s,
            raw.data(), size);
        if (bytesRead <= 0)
            return {};
        raw.truncate(bytesRead);
        return raw;
    }

    return {};
}
