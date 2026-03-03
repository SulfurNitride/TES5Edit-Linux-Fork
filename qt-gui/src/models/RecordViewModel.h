#pragma once
#include <QAbstractTableModel>
#include <QByteArray>
#include <QString>
#include <QVector>

class RecordViewModel : public QAbstractTableModel {
    Q_OBJECT
public:
    enum Column {
        ColSignature = 0,
        ColSize,
        ColData,
        ColText,
        ColumnCount
    };

    explicit RecordViewModel(QObject* parent = nullptr);

    // Set the record to display
    void setRecord(int pluginIdx, int groupIdx, int recordIdx);
    void clear();

    // QAbstractTableModel interface
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    QVariant headerData(int section, Qt::Orientation orientation, int role = Qt::DisplayRole) const override;
    Qt::ItemFlags flags(const QModelIndex& index) const override;
    bool setData(const QModelIndex& index, const QVariant& value, int role = Qt::EditRole) override;

private:
    struct SubrecordInfo {
        QString signature;
        int32_t size;
        QByteArray rawData;     // up to 256 bytes cached
        QString textPreview;    // attempted text decode
    };

    void loadSubrecords();
    static QString tryDecodeText(const QByteArray& data);
    static bool isTextSubrecord(const QString& sig);

    int m_pluginIdx = -1;
    int m_groupIdx = -1;
    int m_recordIdx = -1;
    QVector<SubrecordInfo> m_subrecords;
};
