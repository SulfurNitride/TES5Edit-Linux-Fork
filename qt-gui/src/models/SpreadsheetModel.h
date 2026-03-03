#pragma once
#include <QAbstractTableModel>
#include <QString>
#include <QVector>

// ---------------------------------------------------------------------------
// SpreadsheetModel -- QAbstractTableModel for spreadsheet-style views of game
// records (WEAP, ARMO, AMMO, etc.).  Mirrors the Delphi
// tbsSpreadsheetShow / vstSpreadSheetGetText behaviour from xeMainForm.pas.
//
// Fixed columns: Plugin, FormID, EditorID, Name
// Additional columns are signature-specific (e.g. Damage, Weight, Value ...).
// ---------------------------------------------------------------------------

class SpreadsheetModel : public QAbstractTableModel {
    Q_OBJECT
public:
    // Fixed columns present for every signature type
    enum FixedColumn {
        ColPlugin   = 0,
        ColFormID   = 1,
        ColEditorID = 2,
        ColName     = 3,
        FixedColumnCount = 4
    };

    explicit SpreadsheetModel(QObject* parent = nullptr);

    // Load all records whose group signature matches |signature| (e.g. "WEAP")
    // across every loaded plugin.  Replaces any previously loaded data.
    void loadRecords(const QString& signature);

    // Discard all data.
    void clear();

    // The signature that was last loaded (empty if none).
    const QString& currentSignature() const { return m_signature; }

    // Locate a record by row -- returns false if row is out of range.
    struct RecordLocation {
        int pluginIdx;
        int groupIdx;
        int recordIdx;
    };
    bool recordAt(int row, RecordLocation& out) const;

    // QAbstractTableModel interface
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    QVariant headerData(int section, Qt::Orientation orientation,
                        int role = Qt::DisplayRole) const override;

private:
    // One row in the spreadsheet
    struct RowData {
        int pluginIdx;
        int groupIdx;
        int recordIdx;

        QString pluginName;
        uint32_t formId = 0;
        QString editorId;
        QString name;                   // FULL subrecord text

        // Signature-specific extra fields (variable length, matches m_extraHeaders)
        QVector<QString> extraFields;
    };

    // Signature-specific column definitions
    struct ExtraColumnDef {
        QString header;                 // column header text
        QString subrecordSig;           // 4-char subrecord signature to look up
        int elementIndex;               // element index inside the subrecord (-1 = whole value)
    };

    void buildExtraColumns(const QString& signature);

    // Read a subrecord's text value from a record, searching by subrecord sig.
    // Returns empty string if the subrecord is not found.
    static QString readSubrecordText(int pluginIdx, int groupIdx, int recordIdx,
                                     const QString& targetSig);

    // Read a subrecord's raw data.
    static QByteArray readSubrecordRaw(int pluginIdx, int groupIdx, int recordIdx,
                                       const QString& targetSig);

    QString m_signature;
    QVector<RowData> m_rows;
    QVector<ExtraColumnDef> m_extraColumns;
    QStringList m_extraHeaders;
};
