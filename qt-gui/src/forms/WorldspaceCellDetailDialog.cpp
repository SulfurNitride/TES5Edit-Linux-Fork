#include "WorldspaceCellDetailDialog.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"

#include <QComboBox>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QRadioButton>
#include <QSpinBox>
#include <QSplitter>
#include <QTableWidget>
#include <QVBoxLayout>

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

WorldspaceCellDetailDialog::WorldspaceCellDetailDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle(tr("Worldspace Cell Details"));
    resize(900, 600);

    buildUi();
    loadWorldspaces();
    populateCellGrid();
    updateSummary();
}

// ---------------------------------------------------------------------------
// Public accessors
// ---------------------------------------------------------------------------

bool WorldspaceCellDetailDialog::isPersistent() const
{
    return m_rbPersistent->isChecked();
}

int WorldspaceCellDetailDialog::cellX() const
{
    return m_spinX->value();
}

int WorldspaceCellDetailDialog::cellY() const
{
    return m_spinY->value();
}

QString WorldspaceCellDetailDialog::selectedWorldspace() const
{
    return m_cboWorldspace->currentText();
}

// ---------------------------------------------------------------------------
// UI construction
// ---------------------------------------------------------------------------

void WorldspaceCellDetailDialog::buildUi()
{
    auto* mainLayout = new QVBoxLayout(this);

    // --- Top row: worldspace selector + cell type + coordinates ---
    auto* topLayout = new QHBoxLayout;

    topLayout->addWidget(new QLabel(tr("Worldspace:"), this));
    m_cboWorldspace = new QComboBox(this);
    m_cboWorldspace->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    topLayout->addWidget(m_cboWorldspace);

    topLayout->addSpacing(16);

    m_rbPersistent = new QRadioButton(tr("&Persistent"), this);
    m_rbTemporary  = new QRadioButton(tr("&Temporary"), this);
    m_rbPersistent->setChecked(true);
    topLayout->addWidget(m_rbPersistent);
    topLayout->addWidget(m_rbTemporary);

    topLayout->addSpacing(16);

    topLayout->addWidget(new QLabel(tr("&X:"), this));
    m_spinX = new QSpinBox(this);
    m_spinX->setRange(-9999, 9999);
    m_spinX->setValue(0);
    topLayout->addWidget(m_spinX);

    topLayout->addWidget(new QLabel(tr("&Y:"), this));
    m_spinY = new QSpinBox(this);
    m_spinY->setRange(-9999, 9999);
    m_spinY->setValue(0);
    topLayout->addWidget(m_spinY);

    mainLayout->addLayout(topLayout);

    // --- Splitter: cell grid (left) | info panel (right) ---
    auto* splitter = new QSplitter(Qt::Horizontal, this);

    // Cell grid
    m_cellGrid = new QTableWidget(this);
    m_cellGrid->setSelectionMode(QAbstractItemView::SingleSelection);
    m_cellGrid->setEditTriggers(QAbstractItemView::NoEditTriggers);
    m_cellGrid->horizontalHeader()->setDefaultSectionSize(36);
    m_cellGrid->verticalHeader()->setDefaultSectionSize(24);
    m_cellGrid->horizontalHeader()->setMinimumSectionSize(30);
    m_cellGrid->verticalHeader()->setMinimumSectionSize(20);
    splitter->addWidget(m_cellGrid);

    // Info panel
    auto* infoWidget = new QWidget(this);
    auto* infoLayout = new QVBoxLayout(infoWidget);
    infoLayout->setContentsMargins(4, 0, 0, 0);

    auto* summaryGroup = new QGroupBox(tr("Summary"), infoWidget);
    auto* summaryLayout = new QFormLayout(summaryGroup);
    m_lblCellCount   = new QLabel(QStringLiteral("0"), summaryGroup);
    m_lblLoadedCells = new QLabel(QStringLiteral("0 / 0"), summaryGroup);
    summaryLayout->addRow(tr("Total Cells:"),  m_lblCellCount);
    summaryLayout->addRow(tr("Loaded / Total:"), m_lblLoadedCells);
    infoLayout->addWidget(summaryGroup);

    infoLayout->addWidget(new QLabel(tr("Cell Info:"), infoWidget));
    m_txtCellInfo = new QPlainTextEdit(infoWidget);
    m_txtCellInfo->setReadOnly(true);
    m_txtCellInfo->setFont(QFont(QStringLiteral("monospace"), 9));
    m_txtCellInfo->setPlaceholderText(tr("Select a cell in the grid..."));
    infoLayout->addWidget(m_txtCellInfo, 1);

    splitter->addWidget(infoWidget);
    splitter->setStretchFactor(0, 3); // grid gets more space
    splitter->setStretchFactor(1, 1);

    mainLayout->addWidget(splitter, 1);

    // --- Buttons ---
    auto* btnLayout = new QHBoxLayout;
    btnLayout->addStretch();

    m_btnOk = new QPushButton(tr("&OK"), this);
    m_btnOk->setDefault(true);
    btnLayout->addWidget(m_btnOk);

    m_btnCancel = new QPushButton(tr("&Cancel"), this);
    btnLayout->addWidget(m_btnCancel);

    mainLayout->addLayout(btnLayout);

    // --- Connections ---
    connect(m_cboWorldspace, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &WorldspaceCellDetailDialog::onWorldspaceChanged);
    connect(m_cellGrid, &QTableWidget::cellClicked,
            this, &WorldspaceCellDetailDialog::onCellSelected);
    connect(m_rbPersistent, &QRadioButton::toggled,
            this, &WorldspaceCellDetailDialog::onCellTypeChanged);
    connect(m_rbTemporary,  &QRadioButton::toggled,
            this, &WorldspaceCellDetailDialog::onCellTypeChanged);
    connect(m_btnOk,     &QPushButton::clicked, this, &QDialog::accept);
    connect(m_btnCancel, &QPushButton::clicked, this, &QDialog::reject);
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

void WorldspaceCellDetailDialog::loadWorldspaces()
{
    auto& ffi = XEditFFI::instance();
    if (!ffi.isLoaded())
        return;

    // Scan all plugins for WRLD groups and collect worldspace names
    const int32_t pluginCount = ffi.xedit_plugin_count
        ? ffi.xedit_plugin_count() : 0;

    QStringList worldspaces;

    for (int32_t p = 0; p < pluginCount; ++p) {
        const int32_t groupCount = ffi.xedit_plugin_group_count
            ? ffi.xedit_plugin_group_count(p) : 0;

        for (int32_t g = 0; g < groupCount; ++g) {
            QString sig;
            if (ffi.xedit_group_signature) {
                sig = ffiString([&](char* buf, int32_t len) {
                    return ffi.xedit_group_signature(p, g, buf, len);
                });
            }
            if (sig != QStringLiteral("WRLD"))
                continue;

            // Each record in a WRLD group is a worldspace
            const int32_t recCount = ffi.xedit_group_record_count
                ? ffi.xedit_group_record_count(p, g) : 0;

            for (int32_t r = 0; r < recCount; ++r) {
                QString edid;
                if (ffi.xedit_record_editor_id) {
                    edid = ffiString([&](char* buf, int32_t len) {
                        return ffi.xedit_record_editor_id(p, g, r, buf, len);
                    });
                }
                if (!edid.isEmpty() && !worldspaces.contains(edid))
                    worldspaces.append(edid);
            }
        }
    }

    worldspaces.sort();
    m_cboWorldspace->addItems(worldspaces);

    if (worldspaces.isEmpty())
        m_cboWorldspace->addItem(tr("(no worldspaces found)"));
}

void WorldspaceCellDetailDialog::populateCellGrid()
{
    // Build a coordinate grid from -kGridRadius to +kGridRadius
    const int dim = 2 * kGridRadius + 1;
    m_cellGrid->setRowCount(dim);
    m_cellGrid->setColumnCount(dim);

    // Set coordinate headers
    QStringList headers;
    headers.reserve(dim);
    for (int i = -kGridRadius; i <= kGridRadius; ++i)
        headers.append(QString::number(i));

    m_cellGrid->setHorizontalHeaderLabels(headers);
    m_cellGrid->setVerticalHeaderLabels(headers);

    // Fill cells with placeholder items
    for (int row = 0; row < dim; ++row) {
        for (int col = 0; col < dim; ++col) {
            auto* item = new QTableWidgetItem();
            item->setTextAlignment(Qt::AlignCenter);
            // Mark cells as empty by default
            item->setBackground(QColor(240, 240, 240));
            m_cellGrid->setItem(row, col, item);
        }
    }

    // Scroll to center (0,0)
    m_cellGrid->scrollToItem(
        m_cellGrid->item(kGridRadius, kGridRadius),
        QAbstractItemView::PositionAtCenter);
}

void WorldspaceCellDetailDialog::updateCellInfo(int gridX, int gridY)
{
    QString info;
    info += QStringLiteral("Worldspace: %1\n").arg(m_cboWorldspace->currentText());
    info += QStringLiteral("Cell Type:  %1\n").arg(
        m_rbPersistent->isChecked() ? tr("Persistent") : tr("Temporary"));
    info += QStringLiteral("Grid X:     %1\n").arg(gridX);
    info += QStringLiteral("Grid Y:     %1\n").arg(gridY);
    info += QStringLiteral("\nCoordinates correspond to exterior cell blocks.\n");
    info += QStringLiteral("Each cell covers a 4096x4096 unit area in game.\n");
    info += QStringLiteral("World position: X=%1..%2, Y=%3..%4")
        .arg(gridX * 4096).arg((gridX + 1) * 4096 - 1)
        .arg(gridY * 4096).arg((gridY + 1) * 4096 - 1);

    m_txtCellInfo->setPlainText(info);
}

void WorldspaceCellDetailDialog::updateSummary()
{
    // Without worldspace-specific FFI, show the grid dimensions
    const int dim = 2 * kGridRadius + 1;
    const int totalCells = dim * dim;
    m_lblCellCount->setText(QString::number(totalCells));
    m_lblLoadedCells->setText(QStringLiteral("0 / %1").arg(totalCells));
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------

void WorldspaceCellDetailDialog::onWorldspaceChanged(int index)
{
    Q_UNUSED(index)
    populateCellGrid();
    updateSummary();
    m_txtCellInfo->clear();
}

void WorldspaceCellDetailDialog::onCellSelected(int row, int col)
{
    int gridX = col - kGridRadius;
    int gridY = row - kGridRadius;

    m_spinX->setValue(gridX);
    m_spinY->setValue(gridY);

    updateCellInfo(gridX, gridY);
}

void WorldspaceCellDetailDialog::onCellTypeChanged()
{
    // When switching between persistent/temporary, refresh the grid
    populateCellGrid();
    updateSummary();
}
