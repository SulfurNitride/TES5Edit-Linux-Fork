#pragma once
#include <QVector>
#include <QString>
#include "ffi/XEditTypes.h"

class NavTreeItem {
public:
    NavTreeItem(NodeType type, int pluginIdx, int groupIdx = -1, int recordIdx = -1, NavTreeItem* parent = nullptr);
    ~NavTreeItem();

    void appendChild(NavTreeItem* child);
    NavTreeItem* child(int row) const;
    int childCount() const;
    NavTreeItem* parentItem() const;
    int row() const { return m_row; }

    NodeType nodeType() const { return m_type; }
    int pluginIndex() const { return m_pluginIdx; }
    int groupIndex() const { return m_groupIdx; }
    int recordIndex() const { return m_recordIdx; }

    bool childrenLoaded() const { return m_childrenLoaded; }
    void setChildrenLoaded(bool loaded) { m_childrenLoaded = loaded; }

    int cachedRecordCount() const { return m_cachedRecordCount; }
    void setCachedRecordCount(int count) { m_cachedRecordCount = count; }

    int fetchedCount() const { return m_fetchedCount; }
    void setFetchedCount(int count) { m_fetchedCount = count; }

    // Cached display text — set once, returned instantly by data()
    const QString& displayText() const { return m_displayText; }
    void setDisplayText(const QString& text) { m_displayText = text; }

private:
    NodeType m_type;
    int m_pluginIdx;
    int m_groupIdx;
    int m_recordIdx;
    int m_row = 0;               // cached row index in parent
    bool m_childrenLoaded = false;
    int m_cachedRecordCount = 0;
    int m_fetchedCount = 0;
    QString m_displayText;       // cached display string
    QVector<NavTreeItem*> m_children;
    NavTreeItem* m_parent = nullptr;
};
