#pragma once
#include <QAbstractTableModel>
#include <QString>
#include <QVector>

class RefByTableModel : public QAbstractTableModel {
    Q_OBJECT
public:
    enum Column {
        ColRecord = 0,
        ColSignature,
        ColFormID,
        ColFile,
        ColumnCount
    };

    explicit RefByTableModel(QObject* parent = nullptr);

    // Set the record whose references to display
    void setRecord(int pluginIdx, int groupIdx, int recordIdx);
    void clear();

    // Get entry data for navigation
    struct RefEntry {
        int pluginIdx;
        int groupIdx;
        int recordIdx;
        QString editorId;
        QString signature;
        uint32_t formId;
        QString filename;
    };
    const RefEntry* entryAt(int row) const;

    // QAbstractTableModel interface
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    QVariant headerData(int section, Qt::Orientation orientation, int role = Qt::DisplayRole) const override;

private:
    QVector<RefEntry> m_entries;
};
