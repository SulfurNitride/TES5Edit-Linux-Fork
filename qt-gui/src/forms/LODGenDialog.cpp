#include "LODGenDialog.h"
#include "../ffi/XEditFFI.h"
#include <mutex>
#include <string>
#include <QFileDialog>
#include <QFormLayout>
#include <QGridLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QMenu>
#include <QMessageBox>
#include <QProgressDialog>
#include <QScrollArea>
#include <QSplitter>
#include <QTimer>
#include <QVBoxLayout>

LODGenDialog::LODGenDialog(QWidget* parent)
    : QDialog(parent)
    , m_gameMode("SkyrimSE")
{
    setWindowTitle("LODGen Options");
    resize(860, 520);
    setupUI();
}

void LODGenDialog::setupUI()
{
    auto* mainLayout = new QVBoxLayout(this);

    // Splitter: worldspace list on left, options on right
    auto* splitter = new QSplitter(Qt::Horizontal, this);
    splitter->addWidget(createWorldspacePanel());
    splitter->addWidget(createOptionsPanel());
    splitter->setStretchFactor(0, 1);
    splitter->setStretchFactor(1, 2);
    mainLayout->addWidget(splitter, 1);

    // Output directory row
    auto* outputLayout = new QHBoxLayout();
    outputLayout->addWidget(new QLabel("Output Directory:", this));
    m_edOutputDir = new QLineEdit(this);
    m_edOutputDir->setPlaceholderText("Default output location...");
    outputLayout->addWidget(m_edOutputDir, 1);
    m_btnBrowseOutput = new QPushButton("Browse...", this);
    outputLayout->addWidget(m_btnBrowseOutput);
    mainLayout->addLayout(outputLayout);

    // Info label
    auto* lblInfo = new QLabel(
        "Only worldspaces that have existing lodsettings file are shown, "
        "either in the Data folder or in BSA/BA2 archives. "
        "Archives are loaded same as game from game INI file and plugin names.", this);
    lblInfo->setWordWrap(true);
    auto infoFont = lblInfo->font();
    infoFont.setPointSize(infoFont.pointSize() - 1);
    lblInfo->setFont(infoFont);
    mainLayout->addWidget(lblInfo);

    // Separator line
    auto* separator = new QFrame(this);
    separator->setFrameShape(QFrame::HLine);
    separator->setFrameShadow(QFrame::Sunken);
    mainLayout->addWidget(separator);

    // Bottom buttons
    auto* bottomLayout = new QHBoxLayout();
    bottomLayout->addStretch();
    m_btnGenerate = new QPushButton("Generate", this);
    m_btnCancel = new QPushButton("Cancel", this);
    m_btnGenerate->setDefault(true);
    bottomLayout->addWidget(m_btnGenerate);
    bottomLayout->addWidget(m_btnCancel);
    mainLayout->addLayout(bottomLayout);

    // Connections
    connect(m_chkObjectsLOD, &QCheckBox::toggled, this, &LODGenDialog::onObjectsLODToggled);
    connect(m_chkTreesLOD, &QCheckBox::toggled, this, &LODGenDialog::onTreesLODToggled);
    connect(m_chkTerrainLOD, &QCheckBox::toggled, this, &LODGenDialog::onTerrainLODToggled);
    connect(m_btnBrowseOutput, &QPushButton::clicked, this, &LODGenDialog::onBrowseOutputDir);
    connect(m_btnGenerate, &QPushButton::clicked, this, &LODGenDialog::onGenerateClicked);
    connect(m_btnCancel, &QPushButton::clicked, this, &QDialog::reject);
    connect(m_chkChunk, &QCheckBox::toggled, this, &LODGenDialog::onChunkToggled);
}

QWidget* LODGenDialog::createWorldspacePanel()
{
    auto* panel = new QWidget(this);
    auto* layout = new QVBoxLayout(panel);
    layout->setContentsMargins(0, 0, 0, 0);

    auto* lblHeader = new QLabel("Worldspace", panel);
    auto headerFont = lblHeader->font();
    headerFont.setBold(true);
    lblHeader->setFont(headerFont);
    layout->addWidget(lblHeader);

    m_lstWorldspaces = new QListWidget(panel);
    m_lstWorldspaces->setSelectionMode(QAbstractItemView::ExtendedSelection);
    // Context menu for select all/none
    m_lstWorldspaces->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(m_lstWorldspaces, &QWidget::customContextMenuRequested, this, [this](const QPoint& pos) {
        QMenu menu(m_lstWorldspaces);
        menu.addAction("Select All", this, &LODGenDialog::onSelectAllWorldspaces);
        menu.addAction("Select None", this, &LODGenDialog::onSelectNoneWorldspaces);
        menu.exec(m_lstWorldspaces->mapToGlobal(pos));
    });
    layout->addWidget(m_lstWorldspaces, 1);

    // Select All / Select None buttons
    auto* btnLayout = new QHBoxLayout();
    auto* btnSelectAll = new QPushButton("Select All", panel);
    auto* btnSelectNone = new QPushButton("Select None", panel);
    btnLayout->addWidget(btnSelectAll);
    btnLayout->addWidget(btnSelectNone);
    btnLayout->addStretch();
    layout->addLayout(btnLayout);

    connect(btnSelectAll, &QPushButton::clicked, this, &LODGenDialog::onSelectAllWorldspaces);
    connect(btnSelectNone, &QPushButton::clicked, this, &LODGenDialog::onSelectNoneWorldspaces);

    return panel;
}

QWidget* LODGenDialog::createOptionsPanel()
{
    auto* scrollArea = new QScrollArea(this);
    scrollArea->setWidgetResizable(true);
    scrollArea->setFrameShape(QFrame::NoFrame);

    auto* panel = new QWidget();
    auto* layout = new QVBoxLayout(panel);
    layout->setSpacing(4);

    // ============================================================
    // Objects LOD
    // ============================================================
    m_chkObjectsLOD = new QCheckBox("Objects LOD", panel);
    m_chkObjectsLOD->setChecked(true);
    m_chkObjectsLOD->setToolTip("Generate distant LOD for static objects (mountains, rocks, houses, castles)");
    auto boldFont = m_chkObjectsLOD->font();
    boldFont.setBold(true);
    m_chkObjectsLOD->setFont(boldFont);
    layout->addWidget(m_chkObjectsLOD);

    m_grpObjectsOptions = new QGroupBox(panel);
    auto* objLayout = new QVBoxLayout(m_grpObjectsOptions);
    objLayout->setSpacing(4);

    // Atlas checkbox + size row
    m_chkBuildAtlas = new QCheckBox("Build atlas", m_grpObjectsOptions);
    m_chkBuildAtlas->setChecked(true);
    m_chkBuildAtlas->setToolTip("Put LOD textures on texture atlas(es) for better performance");
    objLayout->addWidget(m_chkBuildAtlas);

    auto* atlasRow = new QHBoxLayout();
    atlasRow->addWidget(new QLabel("Atlas size:", m_grpObjectsOptions));
    m_cmbAtlasWidth = new QComboBox(m_grpObjectsOptions);
    m_cmbAtlasHeight = new QComboBox(m_grpObjectsOptions);
    for (int sz = 1024; sz <= 8192; sz *= 2) {
        m_cmbAtlasWidth->addItem(QString::number(sz));
        m_cmbAtlasHeight->addItem(QString::number(sz));
    }
    m_cmbAtlasWidth->setCurrentIndex(1);   // 2048
    m_cmbAtlasHeight->setCurrentIndex(1);  // 2048
    atlasRow->addWidget(m_cmbAtlasWidth);
    atlasRow->addWidget(new QLabel("x", m_grpObjectsOptions));
    atlasRow->addWidget(m_cmbAtlasHeight);
    atlasRow->addStretch();
    objLayout->addLayout(atlasRow);

    // Max texture size + UV range row
    auto* texRow = new QHBoxLayout();
    texRow->addWidget(new QLabel("Max texture size:", m_grpObjectsOptions));
    m_cmbAtlasTextureSize = new QComboBox(m_grpObjectsOptions);
    for (int sz = 128; sz <= 2048; sz *= 2)
        m_cmbAtlasTextureSize->addItem(QString::number(sz));
    m_cmbAtlasTextureSize->setCurrentText("512");
    texRow->addWidget(m_cmbAtlasTextureSize);
    texRow->addSpacing(10);
    texRow->addWidget(new QLabel("in UV range:", m_grpObjectsOptions));
    m_cmbAtlasUVRange = new QComboBox(m_grpObjectsOptions);
    for (int i = 10; i <= 100; i++)
        m_cmbAtlasUVRange->addItem(QString::number(i / 10.0, 'f', 1));
    m_cmbAtlasUVRange->setCurrentText("1.5");
    m_cmbAtlasUVRange->setToolTip("UV range for tiling detection. Textures with UVs beyond this are excluded from atlas.");
    texRow->addWidget(m_cmbAtlasUVRange);
    texRow->addStretch();
    objLayout->addLayout(texRow);

    // Compression row
    auto* compRow = new QHBoxLayout();
    compRow->addWidget(new QLabel("Compression:", m_grpObjectsOptions));
    const QStringList compressionFormats = {"888", "8888", "565", "DXT1", "DXT3", "DXT5", "BC4", "BC5"};
    compRow->addWidget(new QLabel("Diffuse:", m_grpObjectsOptions));
    m_cmbCompDiffuse = new QComboBox(m_grpObjectsOptions);
    m_cmbCompDiffuse->addItems(compressionFormats);
    m_cmbCompDiffuse->setCurrentText("DXT3");
    compRow->addWidget(m_cmbCompDiffuse);
    compRow->addWidget(new QLabel("Normal:", m_grpObjectsOptions));
    m_cmbCompNormal = new QComboBox(m_grpObjectsOptions);
    m_cmbCompNormal->addItems(compressionFormats);
    m_cmbCompNormal->setCurrentText("DXT1");
    compRow->addWidget(m_cmbCompNormal);
    m_lblSpecular = new QLabel("Spec:", m_grpObjectsOptions);
    compRow->addWidget(m_lblSpecular);
    m_cmbCompSpecular = new QComboBox(m_grpObjectsOptions);
    m_cmbCompSpecular->addItems(compressionFormats);
    m_cmbCompSpecular->setCurrentText("BC5");
    compRow->addWidget(m_cmbCompSpecular);
    compRow->addStretch();
    objLayout->addLayout(compRow);

    // Alpha threshold row
    auto* alphaRow = new QHBoxLayout();
    alphaRow->addWidget(new QLabel("Default Alpha threshold:", m_grpObjectsOptions));
    m_cmbDefaultAlphaThreshold = new QComboBox(m_grpObjectsOptions);
    for (int i = 0; i <= 255; i += 8)
        m_cmbDefaultAlphaThreshold->addItem(QString::number(i));
    m_cmbDefaultAlphaThreshold->setCurrentText("128");
    m_cmbDefaultAlphaThreshold->setEditable(true);
    alphaRow->addWidget(m_cmbDefaultAlphaThreshold);
    alphaRow->addStretch();
    objLayout->addLayout(alphaRow);

    // FO4-specific options
    m_chkUseAlphaThreshold = new QCheckBox("Use source alpha threshold", m_grpObjectsOptions);
    m_chkUseAlphaThreshold->setToolTip("Use alpha threshold from source LOD material instead of default");
    objLayout->addWidget(m_chkUseAlphaThreshold);

    m_chkUseBacklightPower = new QCheckBox("Use backlight power", m_grpObjectsOptions);
    m_chkUseBacklightPower->setToolTip("Use backlight from source LOD material (for double-sided leaves)");
    objLayout->addWidget(m_chkUseBacklightPower);

    // Additional options
    m_chkNoVertexColors = new QCheckBox("No vertex colors", m_grpObjectsOptions);
    m_chkNoVertexColors->setToolTip("Reduces size of generated LOD at the expense of quality");
    objLayout->addWidget(m_chkNoVertexColors);

    m_chkNoTangents = new QCheckBox("No tangents", m_grpObjectsOptions);
    m_chkNoTangents->setToolTip("Reduces size of generated LOD at the expense of quality");
    objLayout->addWidget(m_chkNoTangents);

    layout->addWidget(m_grpObjectsOptions);

    // ============================================================
    // LOD Levels
    // ============================================================
    auto* lodLevelGroup = new QGroupBox("LOD Levels", panel);
    auto* lodLayout = new QHBoxLayout(lodLevelGroup);
    m_chkLOD4 = new QCheckBox("LOD 4", lodLevelGroup);
    m_chkLOD8 = new QCheckBox("LOD 8", lodLevelGroup);
    m_chkLOD16 = new QCheckBox("LOD 16", lodLevelGroup);
    m_chkLOD32 = new QCheckBox("LOD 32", lodLevelGroup);
    m_chkLOD4->setChecked(true);
    m_chkLOD8->setChecked(true);
    m_chkLOD16->setChecked(true);
    m_chkLOD32->setChecked(true);
    lodLayout->addWidget(m_chkLOD4);
    lodLayout->addWidget(m_chkLOD8);
    lodLayout->addWidget(m_chkLOD16);
    lodLayout->addWidget(m_chkLOD32);
    layout->addWidget(lodLevelGroup);

    // ============================================================
    // Specific Chunk
    // ============================================================
    m_chkChunk = new QCheckBox("Specific chunk", panel);
    m_chkChunk->setChecked(false);
    m_chkChunk->setToolTip("Generate LOD for a specific area only");
    layout->addWidget(m_chkChunk);

    m_chunkCoordsWidget = new QWidget(panel);
    auto* chunkLayout = new QHBoxLayout(m_chunkCoordsWidget);
    chunkLayout->setContentsMargins(20, 0, 0, 0);
    chunkLayout->addWidget(new QLabel("LOD Level:", m_chunkCoordsWidget));
    m_cmbChunkLODLevel = new QComboBox(m_chunkCoordsWidget);
    m_cmbChunkLODLevel->addItems({"", "4", "8", "16"});
    chunkLayout->addWidget(m_cmbChunkLODLevel);
    chunkLayout->addSpacing(10);
    chunkLayout->addWidget(new QLabel("W:", m_chunkCoordsWidget));
    m_edLODX = new QLineEdit(m_chunkCoordsWidget);
    m_edLODX->setMaximumWidth(50);
    chunkLayout->addWidget(m_edLODX);
    chunkLayout->addWidget(new QLabel("S:", m_chunkCoordsWidget));
    m_edLODY = new QLineEdit(m_chunkCoordsWidget);
    m_edLODY->setMaximumWidth(50);
    chunkLayout->addWidget(m_edLODY);
    m_lblE = new QLabel("E:", m_chunkCoordsWidget);
    chunkLayout->addWidget(m_lblE);
    m_edLODX2 = new QLineEdit(m_chunkCoordsWidget);
    m_edLODX2->setMaximumWidth(50);
    chunkLayout->addWidget(m_edLODX2);
    m_lblN = new QLabel("N:", m_chunkCoordsWidget);
    chunkLayout->addWidget(m_lblN);
    m_edLODY2 = new QLineEdit(m_chunkCoordsWidget);
    m_edLODY2->setMaximumWidth(50);
    chunkLayout->addWidget(m_edLODY2);
    chunkLayout->addStretch();
    layout->addWidget(m_chunkCoordsWidget);
    m_chunkCoordsWidget->setVisible(false);

    // ============================================================
    // Trees LOD
    // ============================================================
    m_chkTreesLOD = new QCheckBox("Trees LOD", panel);
    m_chkTreesLOD->setChecked(true);
    m_chkTreesLOD->setFont(boldFont);
    m_chkTreesLOD->setToolTip("Generate distant LOD for trees. Requires 2D billboard images.");
    layout->addWidget(m_chkTreesLOD);

    auto* treesRow = new QHBoxLayout();
    treesRow->setContentsMargins(20, 0, 0, 0);
    treesRow->addWidget(new QLabel("LOD brightness:", panel));
    m_cmbTreesBrightness = new QComboBox(panel);
    for (int i = -30; i <= 30; ++i)
        m_cmbTreesBrightness->addItem(QString::number(i));
    m_cmbTreesBrightness->setCurrentText("0");
    m_cmbTreesBrightness->setToolTip("Adjust brightness of distant trees, for ENB preset compatibility");
    treesRow->addWidget(m_cmbTreesBrightness);
    treesRow->addSpacing(10);
    m_chkTrees3D = new QCheckBox("Generate as Objects LOD", panel);
    m_chkTrees3D->setChecked(true);
    m_chkTrees3D->setToolTip(
        "Insert trees into Objects LOD by using provided LOD nifs or placing "
        "billboard image on a flat mesh. Allows distant trees to be affected by lighting.");
    treesRow->addWidget(m_chkTrees3D);
    treesRow->addStretch();
    layout->addLayout(treesRow);

    // ============================================================
    // Terrain LOD
    // ============================================================
    m_chkTerrainLOD = new QCheckBox("Terrain LOD", panel);
    m_chkTerrainLOD->setChecked(false);
    m_chkTerrainLOD->setFont(boldFont);
    layout->addWidget(m_chkTerrainLOD);

    layout->addStretch();

    // Initial state
    updateObjectsOptionsEnabled();
    updateTreesOptionsEnabled();
    applyGameModeVisibility();

    scrollArea->setWidget(panel);
    return scrollArea;
}

// ---------------------------------------------------------------------------
// Worldspace management
// ---------------------------------------------------------------------------
void LODGenDialog::setWorldspaces(const QStringList& worldspaces)
{
    m_lstWorldspaces->clear();
    for (const QString& ws : worldspaces) {
        auto* item = new QListWidgetItem(ws, m_lstWorldspaces);
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(Qt::Unchecked);
    }
}

QStringList LODGenDialog::selectedWorldspaces() const
{
    QStringList result;
    for (int i = 0; i < m_lstWorldspaces->count(); ++i) {
        auto* item = m_lstWorldspaces->item(i);
        if (item && item->checkState() == Qt::Checked)
            result.append(item->text());
    }
    return result;
}

void LODGenDialog::setGameMode(const QString& gameMode)
{
    m_gameMode = gameMode;
    applyGameModeVisibility();
}

QString LODGenDialog::outputDirectory() const       { return m_edOutputDir->text(); }
void LODGenDialog::setOutputDirectory(const QString& dir) { m_edOutputDir->setText(dir); }

// ---------------------------------------------------------------------------
// JSON serialization for FFI
// ---------------------------------------------------------------------------
QByteArray LODGenDialog::toJson() const
{
    QJsonObject obj;
    obj["worldspaces"] = QJsonArray::fromStringList(selectedWorldspaces());
    obj["output_dir"]  = m_edOutputDir->text();

    obj["objects_lod"]  = m_chkObjectsLOD->isChecked();
    obj["trees_lod"]    = m_chkTreesLOD->isChecked();
    obj["terrain_lod"]  = m_chkTerrainLOD->isChecked();

    QJsonArray levels;
    if (m_chkLOD4->isChecked())  levels.append(4);
    if (m_chkLOD8->isChecked())  levels.append(8);
    if (m_chkLOD16->isChecked()) levels.append(16);
    if (m_chkLOD32->isChecked()) levels.append(32);
    obj["lod_levels"] = levels;

    obj["build_atlas"]           = m_chkBuildAtlas->isChecked();
    obj["atlas_width"]           = m_cmbAtlasWidth->currentText().toInt();
    obj["atlas_height"]          = m_cmbAtlasHeight->currentText().toInt();
    obj["atlas_texture_size"]    = m_cmbAtlasTextureSize->currentText().toInt();
    obj["atlas_uv_range"]        = m_cmbAtlasUVRange->currentText().toDouble();
    obj["compression_diffuse"]   = m_cmbCompDiffuse->currentText();
    obj["compression_normal"]    = m_cmbCompNormal->currentText();
    obj["compression_specular"]  = m_cmbCompSpecular->currentText();
    obj["default_alpha_threshold"] = m_cmbDefaultAlphaThreshold->currentText().toInt();
    obj["use_alpha_threshold"]   = m_chkUseAlphaThreshold->isChecked();
    obj["use_backlight_power"]   = m_chkUseBacklightPower->isChecked();
    obj["no_tangents"]           = m_chkNoTangents->isChecked();
    obj["no_vertex_colors"]      = m_chkNoVertexColors->isChecked();

    obj["trees_brightness"] = m_cmbTreesBrightness->currentText().toInt();
    obj["trees_3d"]         = m_chkTrees3D->isChecked();

    // Specific chunk options
    obj["chunk"] = m_chkChunk->isChecked();
    if (m_chkChunk->isChecked()) {
        obj["chunk_lod_level"] = m_cmbChunkLODLevel->currentText();
        obj["chunk_x"]  = m_edLODX->text();
        obj["chunk_y"]  = m_edLODY->text();
        obj["chunk_x2"] = m_edLODX2->text();
        obj["chunk_y2"] = m_edLODY2->text();
    }

    return QJsonDocument(obj).toJson(QJsonDocument::Compact);
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------
void LODGenDialog::onObjectsLODToggled(bool)
{
    updateObjectsOptionsEnabled();
}

void LODGenDialog::onTreesLODToggled(bool)
{
    updateTreesOptionsEnabled();
}

void LODGenDialog::onTerrainLODToggled(bool)
{
}

void LODGenDialog::onChunkToggled(bool checked)
{
    m_chunkCoordsWidget->setVisible(checked);
}

void LODGenDialog::onBrowseOutputDir()
{
    QString dir = QFileDialog::getExistingDirectory(
        this, "Select Output Directory", m_edOutputDir->text());
    if (!dir.isEmpty())
        m_edOutputDir->setText(dir);
}

void LODGenDialog::onSelectAllWorldspaces()
{
    for (int i = 0; i < m_lstWorldspaces->count(); ++i) {
        auto* item = m_lstWorldspaces->item(i);
        if (item)
            item->setCheckState(Qt::Checked);
    }
}

void LODGenDialog::onSelectNoneWorldspaces()
{
    for (int i = 0; i < m_lstWorldspaces->count(); ++i) {
        auto* item = m_lstWorldspaces->item(i);
        if (item)
            item->setCheckState(Qt::Unchecked);
    }
}

// Thread-safe progress message storage for LOD generation callback
static std::mutex s_lodProgressMutex;
static std::string s_lodProgressMessage;
static double s_lodProgressFraction = 0.0;

extern "C" void lodProgressCallback(const char* message, double progress) {
    std::lock_guard<std::mutex> lock(s_lodProgressMutex);
    if (message)
        s_lodProgressMessage = message;
    s_lodProgressFraction = progress;
}

void LODGenDialog::onGenerateClicked()
{
    if (selectedWorldspaces().isEmpty()) {
        QMessageBox::warning(this, "LOD Generation",
            "Select worldspace(s) for LOD generation.");
        return;
    }

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_lod_generate) {
        QMessageBox::warning(this, "LOD Generation",
            "LOD generation not available (FFI symbol missing).");
        return;
    }

    QByteArray json = toJson();

    // Clear progress state
    {
        std::lock_guard<std::mutex> lock(s_lodProgressMutex);
        s_lodProgressMessage = "Scanning references...";
        s_lodProgressFraction = 0.0;
    }

    // Start async LOD generation with progress callback
    int32_t result = ffi.xedit_lod_generate(
        json.constData(), reinterpret_cast<void*>(&lodProgressCallback));
    if (result != 0) {
        QMessageBox::critical(this, "LOD Generation",
            "Failed to start LOD generation.");
        return;
    }

    // Poll for completion with a progress dialog
    auto* progressDlg = new QProgressDialog("Scanning references...", "Cancel", 0, 100, this);
    progressDlg->setWindowModality(Qt::WindowModal);
    progressDlg->setMinimumDuration(0);
    progressDlg->setMinimumWidth(450);
    progressDlg->show();

    auto* timer = new QTimer(this);
    connect(timer, &QTimer::timeout, this, [this, timer, progressDlg, &ffi]() {
        if (progressDlg->wasCanceled()) {
            if (ffi.xedit_lod_cancel)
                ffi.xedit_lod_cancel();
            timer->stop();
            progressDlg->close();
            progressDlg->deleteLater();
            return;
        }

        // Update progress text and bar from callback data
        {
            std::lock_guard<std::mutex> lock(s_lodProgressMutex);
            if (!s_lodProgressMessage.empty())
                progressDlg->setLabelText(QString::fromStdString(s_lodProgressMessage));
            progressDlg->setValue(static_cast<int>(s_lodProgressFraction * 100.0));
        }

        int32_t status = ffi.xedit_lod_status ? ffi.xedit_lod_status() : -2;

        if (status == 1) {
            // Done successfully
            timer->stop();
            progressDlg->close();
            progressDlg->deleteLater();
            QMessageBox::information(this, "LOD Generation",
                "LOD generation completed successfully.");
            accept();
        } else if (status == -1) {
            // Error
            timer->stop();
            progressDlg->close();
            progressDlg->deleteLater();

            QString errorMsg = "LOD generation failed.";
            if (ffi.xedit_lod_error) {
                QByteArray errBuf(4096, 0);
                int32_t errLen = ffi.xedit_lod_error(
                    reinterpret_cast<uint8_t*>(errBuf.data()), errBuf.size());
                if (errLen > 0)
                    errorMsg = QString::fromUtf8(errBuf.constData(), errLen);
            }
            QMessageBox::critical(this, "LOD Generation", errorMsg);
        }
        // status == 0: still running, keep polling
    });
    timer->start(200);
}

void LODGenDialog::updateObjectsOptionsEnabled()
{
    bool enabled = m_chkObjectsLOD->isChecked();
    m_grpObjectsOptions->setEnabled(enabled);
    m_grpObjectsOptions->setVisible(enabled);
}

void LODGenDialog::updateTreesOptionsEnabled()
{
    bool enabled = m_chkTreesLOD->isChecked();
    m_cmbTreesBrightness->setEnabled(enabled);
    m_chkTrees3D->setEnabled(enabled);
}

void LODGenDialog::applyGameModeVisibility()
{
    bool isFO4 = (m_gameMode == "Fallout4");
    bool isFO3FNV = (m_gameMode == "Fallout3" || m_gameMode == "FalloutNV");
    bool isSkyrim = (m_gameMode == "SkyrimSE" || m_gameMode == "SkyrimLE");

    // FO4-specific controls
    m_chkUseAlphaThreshold->setVisible(isFO4);
    m_chkUseBacklightPower->setVisible(isFO4);
    m_cmbCompSpecular->setEnabled(isFO4);
    m_lblSpecular->setEnabled(isFO4);

    // Trees 3D only for Skyrim
    m_chkTrees3D->setVisible(isSkyrim);

    // FO3/FNV: E/N coordinate fields visible, some options disabled
    m_lblE->setVisible(isFO3FNV);
    m_edLODX2->setVisible(isFO3FNV);
    m_lblN->setVisible(isFO3FNV);
    m_edLODY2->setVisible(isFO3FNV);

    // FO3/FNV: atlas is mandatory, disable toggle
    if (isFO3FNV) {
        m_chkBuildAtlas->setChecked(true);
        m_chkBuildAtlas->setEnabled(false);
    } else {
        m_chkBuildAtlas->setEnabled(true);
    }
}
