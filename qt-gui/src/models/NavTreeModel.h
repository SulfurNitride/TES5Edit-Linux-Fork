#pragma once
#include <QAbstractItemModel>
#include <QMimeData>
#include <QStringList>

class NavTreeItem;

class NavTreeModel : public QAbstractItemModel {
    Q_OBJECT
public:
    explicit NavTreeModel(QObject* parent = nullptr);
    ~NavTreeModel() override;

    // QAbstractItemModel interface
    QModelIndex index(int row, int column, const QModelIndex& parent = {}) const override;
    QModelIndex parent(const QModelIndex& index) const override;
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    bool hasChildren(const QModelIndex& parent = {}) const override;
    bool canFetchMore(const QModelIndex& parent) const override;
    void fetchMore(const QModelIndex& parent) override;
    Qt::ItemFlags flags(const QModelIndex& index) const override;

    // Drag and drop support
    Qt::DropActions supportedDragActions() const override;
    Qt::DropActions supportedDropActions() const override;
    QStringList mimeTypes() const override;
    QMimeData* mimeData(const QModelIndexList& indexes) const override;
    bool canDropMimeData(const QMimeData* data, Qt::DropAction action,
                         int row, int column, const QModelIndex& parent) const override;
    bool dropMimeData(const QMimeData* data, Qt::DropAction action,
                      int row, int column, const QModelIndex& parent) override;

    // Custom API
    void addPlugin(int pluginIndex);
    void clear();
    NavTreeItem* itemFromIndex(const QModelIndex& index) const;
    QModelIndex findRecord(int pluginIdx, int groupIdx, int recordIdx) const;

    static constexpr const char* MimeType = "application/x-xedit-navitem";

private:
    void populateGroups(NavTreeItem* pluginItem);
    NavTreeItem* m_rootItem = nullptr;
};
