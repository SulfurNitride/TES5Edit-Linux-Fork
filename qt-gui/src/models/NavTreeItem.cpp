#include "models/NavTreeItem.h"

NavTreeItem::NavTreeItem(NodeType type, int pluginIdx, int groupIdx, int recordIdx, NavTreeItem* parent)
    : m_type(type)
    , m_pluginIdx(pluginIdx)
    , m_groupIdx(groupIdx)
    , m_recordIdx(recordIdx)
    , m_parent(parent)
{
}

NavTreeItem::~NavTreeItem()
{
    qDeleteAll(m_children);
}

void NavTreeItem::appendChild(NavTreeItem* child)
{
    child->m_row = m_children.size();
    m_children.append(child);
}

void NavTreeItem::removeChild(int row)
{
    if (row < 0 || row >= m_children.size())
        return;
    delete m_children.takeAt(row);
    // Re-index rows after removal
    for (int i = row; i < m_children.size(); ++i)
        m_children[i]->m_row = i;
}

NavTreeItem* NavTreeItem::child(int row) const
{
    if (row < 0 || row >= m_children.size())
        return nullptr;
    return m_children.at(row);
}

int NavTreeItem::childCount() const
{
    return m_children.size();
}

NavTreeItem* NavTreeItem::parentItem() const
{
    return m_parent;
}
