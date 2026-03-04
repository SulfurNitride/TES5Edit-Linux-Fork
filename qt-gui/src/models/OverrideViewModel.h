#pragma once

#include <QAbstractItemModel>
#include <QString>
#include <QStringList>
#include <QVector>

#include <cstdint>

#include "delegates/ConflictColorDelegate.h"

struct PluginLocation {
    int32_t pluginIdx  = -1;
    int32_t groupIdx   = -1;
    int32_t recordIdx  = -1;
};

struct ViewNode {
    QString fieldName;
    QString signature;
    int subrecordIndex = -1;
    QVector<QString> values;
    QVector<int> conflictThis;
    int conflictAll = 0;
    QVector<ViewNode*> children;
    ViewNode* parent = nullptr;
    int row = 0;

    ~ViewNode() { qDeleteAll(children); }
};

class OverrideViewModel : public QAbstractItemModel {
    Q_OBJECT
public:
    explicit OverrideViewModel(QObject* parent = nullptr);
    ~OverrideViewModel() override;

    // Load data for a specific FormID across all overriding plugins
    void loadRecord(uint32_t formId);
    void clear();

    // QAbstractItemModel interface
    QModelIndex index(int row, int column, const QModelIndex& parent = {}) const override;
    QModelIndex parent(const QModelIndex& child) const override;
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    bool hasChildren(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    QVariant headerData(int section, Qt::Orientation orientation, int role = Qt::DisplayRole) const override;
    Qt::ItemFlags flags(const QModelIndex& index) const override;
    bool setData(const QModelIndex& index, const QVariant& value, int role = Qt::EditRole) override;

private:
    ViewNode* nodeFromIndex(const QModelIndex& index) const;
    void buildTree(uint32_t formId);

    ViewNode* m_root = nullptr;
    QStringList m_pluginNames;                 // column headers (plugin filenames)
    QVector<PluginLocation> m_pluginLocations; // (plugin, group, record) per column
    uint32_t m_currentFormId = 0;
};
