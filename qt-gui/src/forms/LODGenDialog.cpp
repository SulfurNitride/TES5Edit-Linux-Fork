#include "LODGenDialog.h"
#include "../ffi/XEditFFI.h"
#include <QFile>
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
#include <QScrollBar>
#include <QSplitter>
#include <QStandardItemModel>
#include <QCoreApplication>
#include <QTimer>
#include <QThread>
#include <QtConcurrent/QtConcurrent>
#include <QVBoxLayout>

// Compression format list shared across Objects/Trees/Terrain
static const QStringList kCompressionFormats = {
    "888", "8888", "565", "DXT1", "DXT3", "DXT5", "BC4", "BC5"
};

LODGenDialog::LODGenDialog(QWidget* parent)
    : QDialog(parent)
    , m_gameMode("SkyrimSE")
{
    setWindowTitle("LODGen Options");
    resize(900, 640);
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
    splitter->setStretchFactor(1, 3);
    mainLayout->addWidget(splitter, 1);

    // Bottom panel (output dir, chunk, seasons, buttons)
    mainLayout->addWidget(createBottomPanel());

    // Log/progress panel (hidden until generation starts)
    m_logPanel = new QWidget(this);
    auto* logLayout = new QVBoxLayout(m_logPanel);
    logLayout->setContentsMargins(0, 4, 0, 0);

    m_progressBar = new QProgressBar(m_logPanel);
    m_progressBar->setRange(0, 100);
    m_progressBar->setValue(0);
    m_progressBar->setTextVisible(true);
    m_progressBar->setFormat("%p% - %v of 100");
    logLayout->addWidget(m_progressBar);

    m_logOutput = new QPlainTextEdit(m_logPanel);
    m_logOutput->setReadOnly(true);
    m_logOutput->setMaximumBlockCount(5000);
    m_logOutput->setFont(QFont("Monospace", 9));
    m_logOutput->setMinimumHeight(120);
    m_logOutput->setMaximumHeight(200);
    logLayout->addWidget(m_logOutput);

    m_logPanel->setVisible(false);
    mainLayout->addWidget(m_logPanel);

    // Connections - LOD type toggles
    connect(m_chkObjectsLOD, &QCheckBox::toggled, this, &LODGenDialog::onObjectsLODToggled);
    connect(m_chkTreesLOD, &QCheckBox::toggled, this, &LODGenDialog::onTreesLODToggled);
    connect(m_chkTerrainLOD, &QCheckBox::toggled, this, &LODGenDialog::onTerrainLODToggled);
    connect(m_chkOcclusion, &QCheckBox::toggled, this, &LODGenDialog::onOcclusionToggled);
    connect(m_chkChunk, &QCheckBox::toggled, this, &LODGenDialog::onChunkToggled);
    connect(m_chkSeasons, &QCheckBox::toggled, this, &LODGenDialog::onSeasonsToggled);
    connect(m_chkTrees3D, &QCheckBox::toggled, this, &LODGenDialog::onTrees3DToggled);
    connect(m_chkTreeNormalMap, &QCheckBox::toggled, this, &LODGenDialog::onTreeNormalMapToggled);
    connect(m_chkBakeNormalMaps, &QCheckBox::toggled, this, &LODGenDialog::onBakeNormalMapsToggled);
    connect(m_cmbTerrainLevel, &QComboBox::currentIndexChanged, this, [this]() { onTerrainLevelChanged(); });

    // Compression change affects alpha threshold
    connect(m_cmbCompDiffuse, &QComboBox::currentTextChanged, this, [this]() {
        updateAlphaThresholdEnabled();
    });

    // Button connections
    connect(m_btnBrowseOutput, &QPushButton::clicked, this, &LODGenDialog::onBrowseOutputDir);
    connect(m_btnGenerate, &QPushButton::clicked, this, &LODGenDialog::onGenerateClicked);
    connect(m_btnCancel, &QPushButton::clicked, this, &QDialog::reject);

    // Initial visibility/enabled state (must be after all widgets are created)
    updateObjectsOptionsEnabled();
    updateTreesOptionsEnabled();
    updateTerrainOptionsEnabled();
    updateOcclusionOptionsEnabled();
    applyGameModeVisibility();
}

// ==========================================================================
// Worldspace panel (left side)
// ==========================================================================
QWidget* LODGenDialog::createWorldspacePanel()
{
    auto* panel = new QWidget(this);
    auto* layout = new QVBoxLayout(panel);
    layout->setContentsMargins(0, 0, 0, 0);

    auto* lblHeader = new QLabel("(Select worldspace(s) to generate LOD for)", panel);
    auto headerFont = lblHeader->font();
    headerFont.setBold(true);
    lblHeader->setFont(headerFont);
    layout->addWidget(lblHeader);

    m_lstWorldspaces = new QListWidget(panel);
    m_lstWorldspaces->setSelectionMode(QAbstractItemView::ExtendedSelection);
    m_lstWorldspaces->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(m_lstWorldspaces, &QWidget::customContextMenuRequested, this, [this](const QPoint& pos) {
        QMenu menu(m_lstWorldspaces);
        menu.addAction("Select All", this, &LODGenDialog::onSelectAllWorldspaces);
        menu.addAction("Select None", this, &LODGenDialog::onSelectNoneWorldspaces);
        menu.exec(m_lstWorldspaces->mapToGlobal(pos));
    });
    layout->addWidget(m_lstWorldspaces, 1);

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

// ==========================================================================
// Options panel (right side, scrollable)
// ==========================================================================
QWidget* LODGenDialog::createOptionsPanel()
{
    auto* scrollArea = new QScrollArea(this);
    scrollArea->setWidgetResizable(true);
    scrollArea->setFrameShape(QFrame::NoFrame);

    auto* panel = new QWidget();
    auto* layout = new QVBoxLayout(panel);
    layout->setSpacing(4);

    auto boldFont = panel->font();
    boldFont.setBold(true);

    // ==================================================================
    // Objects LOD
    // ==================================================================
    m_chkObjectsLOD = new QCheckBox("Objects LOD", panel);
    m_chkObjectsLOD->setChecked(true);
    m_chkObjectsLOD->setFont(boldFont);
    m_chkObjectsLOD->setToolTip("Create distant LOD for static objects like mountains, rocks, houses, castles");
    layout->addWidget(m_chkObjectsLOD);

    m_grpObjectsOptions = new QGroupBox(panel);
    {
        auto* objLayout = new QVBoxLayout(m_grpObjectsOptions);
        objLayout->setSpacing(4);

        // Build atlas + atlas size row
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
        m_cmbAtlasHeight->setCurrentIndex(1);   // 2048
        atlasRow->addWidget(m_cmbAtlasWidth);
        atlasRow->addWidget(new QLabel("x", m_grpObjectsOptions));
        atlasRow->addWidget(m_cmbAtlasHeight);
        atlasRow->addStretch();
        objLayout->addLayout(atlasRow);

        // Max texture size + UV range
        auto* texRow = new QHBoxLayout();
        m_lblMaxTextureSize = new QLabel("Max texture size:", m_grpObjectsOptions);
        texRow->addWidget(m_lblMaxTextureSize);
        m_cmbAtlasTextureSize = new QComboBox(m_grpObjectsOptions);
        for (int sz = 256; sz <= 1024; sz *= 2)
            m_cmbAtlasTextureSize->addItem(QString::number(sz));
        m_cmbAtlasTextureSize->setCurrentText("512");
        texRow->addWidget(m_cmbAtlasTextureSize);
        texRow->addSpacing(10);
        m_lblUVRange = new QLabel("in UV range:", m_grpObjectsOptions);
        texRow->addWidget(m_lblUVRange);
        m_cmbAtlasUVRange = new QComboBox(m_grpObjectsOptions);
        for (int i = 10; i <= 100; i++)
            m_cmbAtlasUVRange->addItem(QString::number(i / 10.0, 'f', 1));
        m_cmbAtlasUVRange->setCurrentText("1.5");
        m_cmbAtlasUVRange->setToolTip("UV range for tiling detection. Textures with UVs beyond this are excluded from atlas.");
        texRow->addWidget(m_cmbAtlasUVRange);
        texRow->addStretch();
        objLayout->addLayout(texRow);

        // Compression: Diffuse / Normal / Specular
        auto* compRow = new QHBoxLayout();
        compRow->addWidget(new QLabel("Compression:", m_grpObjectsOptions));
        compRow->addWidget(new QLabel("Diffuse:", m_grpObjectsOptions));
        m_cmbCompDiffuse = new QComboBox(m_grpObjectsOptions);
        m_cmbCompDiffuse->addItems(kCompressionFormats);
        m_cmbCompDiffuse->setCurrentText("DXT3");
        compRow->addWidget(m_cmbCompDiffuse);
        compRow->addWidget(new QLabel("Normal:", m_grpObjectsOptions));
        m_cmbCompNormal = new QComboBox(m_grpObjectsOptions);
        m_cmbCompNormal->addItems(kCompressionFormats);
        m_cmbCompNormal->setCurrentText("DXT1");
        compRow->addWidget(m_cmbCompNormal);
        m_lblSpecular = new QLabel("Spec:", m_grpObjectsOptions);
        compRow->addWidget(m_lblSpecular);
        m_cmbCompSpecular = new QComboBox(m_grpObjectsOptions);
        m_cmbCompSpecular->addItems(kCompressionFormats);
        m_cmbCompSpecular->setCurrentText("BC5");
        compRow->addWidget(m_cmbCompSpecular);
        compRow->addStretch();
        objLayout->addLayout(compRow);

        // Default Alpha threshold
        auto* alphaRow = new QHBoxLayout();
        alphaRow->addWidget(new QLabel("Default Alpha threshold:", m_grpObjectsOptions));
        m_cmbDefaultAlphaThreshold = new QComboBox(m_grpObjectsOptions);
        for (int i = 0; i <= 255; i++)
            m_cmbDefaultAlphaThreshold->addItem(QString::number(i));
        m_cmbDefaultAlphaThreshold->setCurrentText("128");
        alphaRow->addWidget(m_cmbDefaultAlphaThreshold);
        alphaRow->addStretch();
        objLayout->addLayout(alphaRow);

        // FO4-specific
        m_chkUseAlphaThreshold = new QCheckBox("Use source alpha threshold", m_grpObjectsOptions);
        m_chkUseAlphaThreshold->setToolTip("Set transparency threshold to value from source LOD model/material instead of default");
        objLayout->addWidget(m_chkUseAlphaThreshold);

        m_chkUseBacklightPower = new QCheckBox("Use backlight power", m_grpObjectsOptions);
        m_chkUseBacklightPower->setToolTip("Use backlight power to value from source LOD model/material. Improves static tree LOD that uses doublesided foliage, requires custom models/materials setting backlight.");
        objLayout->addWidget(m_chkUseBacklightPower);

        // No vertex colors / No tangents
        m_chkNoVertexColors = new QCheckBox("No vertex colors", m_grpObjectsOptions);
        m_chkNoVertexColors->setToolTip("Reduces size of generated LOD and frees memory for the game at the expense of LOD quality");
        objLayout->addWidget(m_chkNoVertexColors);

        m_chkNoTangents = new QCheckBox("No tangents", m_grpObjectsOptions);
        m_chkNoTangents->setToolTip("Reduces size of generated LOD and frees memory for the game at the expense of LOD quality");
        objLayout->addWidget(m_chkNoTangents);

        // Snow
        m_chkSnow = new QCheckBox("Snow", m_grpObjectsOptions);
        m_chkSnow->setToolTip("Apply snow LOD shader to everything in the worldspace");
        objLayout->addWidget(m_chkSnow);

        // Keep Specular
        m_chkKeepSpecular = new QCheckBox("Keep Specular", m_grpObjectsOptions);
        objLayout->addWidget(m_chkKeepSpecular);
    }
    layout->addWidget(m_grpObjectsOptions);

    // ==================================================================
    // Trees LOD
    // ==================================================================
    m_chkTreesLOD = new QCheckBox("Trees LOD", panel);
    m_chkTreesLOD->setChecked(true);
    m_chkTreesLOD->setFont(boldFont);
    m_chkTreesLOD->setToolTip("Create distant LOD for trees. Requires 2D billboard images to be present in the Data folder or in BSA archives.");
    layout->addWidget(m_chkTreesLOD);

    m_grpTreesOptions = new QGroupBox(panel);
    {
        auto* treesLayout = new QVBoxLayout(m_grpTreesOptions);
        treesLayout->setSpacing(4);

        // Brightness + Generate as Objects LOD
        auto* treesRow1 = new QHBoxLayout();
        treesRow1->addWidget(new QLabel("LOD brightness:", m_grpTreesOptions));
        m_cmbTreesBrightness = new QComboBox(m_grpTreesOptions);
        for (int i = -30; i <= 30; ++i)
            m_cmbTreesBrightness->addItem(QString::number(i));
        m_cmbTreesBrightness->setCurrentText("0");
        m_cmbTreesBrightness->setToolTip("Some ENB presets might require different brightness of distant trees for better look, usually negative darker brightness");
        treesRow1->addWidget(m_cmbTreesBrightness);
        treesRow1->addSpacing(10);
        m_chkTrees3D = new QCheckBox("Generate as Objects LOD", m_grpTreesOptions);
        m_chkTrees3D->setChecked(true);
        m_chkTrees3D->setToolTip("Insert trees into Objects LOD by using provided LOD nifs or placing billboard image on a flat mesh. This allows distant trees to be affected by lighting.");
        treesRow1->addWidget(m_chkTrees3D);
        treesRow1->addStretch();
        treesLayout->addLayout(treesRow1);

        // Normal checkbox
        m_chkTreeNormalMap = new QCheckBox("Normal", m_grpTreesOptions);
        m_chkTreeNormalMap->setToolTip("Build normal map texture atlas. Requires billboard normal map textures or flat normals will be used.");
        treesLayout->addWidget(m_chkTreeNormalMap);

        // Compression: Diffuse / Normal
        auto* treeCompRow = new QHBoxLayout();
        m_lblTreeCompDiffuse = new QLabel("Compression: Diffuse:", m_grpTreesOptions);
        treeCompRow->addWidget(m_lblTreeCompDiffuse);
        m_cmbTreeCompDiffuse = new QComboBox(m_grpTreesOptions);
        m_cmbTreeCompDiffuse->addItems(kCompressionFormats);
        m_cmbTreeCompDiffuse->setCurrentText("DXT3");
        treeCompRow->addWidget(m_cmbTreeCompDiffuse);
        treeCompRow->addSpacing(10);
        m_lblTreeCompNormal = new QLabel("Normal:", m_grpTreesOptions);
        treeCompRow->addWidget(m_lblTreeCompNormal);
        m_cmbTreeCompNormal = new QComboBox(m_grpTreesOptions);
        m_cmbTreeCompNormal->addItems(kCompressionFormats);
        m_cmbTreeCompNormal->setCurrentText("DXT1");
        treeCompRow->addWidget(m_cmbTreeCompNormal);
        treeCompRow->addStretch();
        treesLayout->addLayout(treeCompRow);

        // Split LOD Atlas button
        m_btnSplitTreesLOD = new QPushButton("Split LOD Atlas", m_grpTreesOptions);
        m_btnSplitTreesLOD->setVisible(false); // shown only in certain contexts
        treesLayout->addWidget(m_btnSplitTreesLOD);
    }
    layout->addWidget(m_grpTreesOptions);

    // ==================================================================
    // Terrain LOD
    // ==================================================================
    m_chkTerrainLOD = new QCheckBox("Terrain LOD", panel);
    m_chkTerrainLOD->setChecked(false);
    m_chkTerrainLOD->setFont(boldFont);
    m_chkTerrainLOD->setToolTip("Create distant LOD for terrain and water");
    layout->addWidget(m_chkTerrainLOD);

    m_grpTerrainOptions = new QGroupBox(panel);
    {
        auto* terrainLayout = new QVBoxLayout(m_grpTerrainOptions);
        terrainLayout->setSpacing(4);

        // Settings for + LOD level selector
        auto* settingsRow = new QHBoxLayout();
        settingsRow->addWidget(new QLabel("Settings for:", m_grpTerrainOptions));
        m_cmbTerrainLevel = new QComboBox(m_grpTerrainOptions);
        m_cmbTerrainLevel->addItems({"LOD 4", "LOD 8", "LOD 16", "LOD 32"});
        m_cmbTerrainLevel->setCurrentIndex(0);
        settingsRow->addWidget(m_cmbTerrainLevel);
        settingsRow->addStretch();
        terrainLayout->addLayout(settingsRow);

        // Build checkboxes
        auto* buildRow = new QHBoxLayout();
        m_chkBuildMeshes = new QCheckBox("Build meshes", m_grpTerrainOptions);
        m_chkBuildMeshes->setToolTip("Build terrain LOD meshes");
        buildRow->addWidget(m_chkBuildMeshes);
        m_chkBuildDiffuseTextures = new QCheckBox("Build diffuse", m_grpTerrainOptions);
        m_chkBuildDiffuseTextures->setToolTip("Build terrain LOD diffuse and normal textures");
        buildRow->addWidget(m_chkBuildDiffuseTextures);
        m_chkBuildNormalTextures = new QCheckBox("Build normal", m_grpTerrainOptions);
        buildRow->addWidget(m_chkBuildNormalTextures);
        buildRow->addStretch();
        terrainLayout->addLayout(buildRow);

        // Per-LOD-level panels (stacked, only one visible at a time)
        terrainLayout->addWidget(createTerrainLevelPanel(4, m_terrainLOD4));
        terrainLayout->addWidget(createTerrainLevelPanel(8, m_terrainLOD8));
        terrainLayout->addWidget(createTerrainLevelPanel(16, m_terrainLOD16));
        terrainLayout->addWidget(createTerrainLevelPanel(32, m_terrainLOD32));

        // Show LOD4 by default
        m_terrainLOD4.panel->setVisible(true);
        m_terrainLOD8.panel->setVisible(false);
        m_terrainLOD16.panel->setVisible(false);
        m_terrainLOD32.panel->setVisible(false);

        // Shared terrain options
        auto* sharedRow = new QHBoxLayout();
        m_chkBakeNormalMaps = new QCheckBox("Bake normal-maps", m_grpTerrainOptions);
        m_chkBakeNormalMaps->setToolTip("Bake normal-maps of landscape textures onto terrain model-space normals. Automatically enables Raise steepness.");
        sharedRow->addWidget(m_chkBakeNormalMaps);
        m_chkBakeSpecular = new QCheckBox("Bake specular", m_grpTerrainOptions);
        m_chkBakeSpecular->setToolTip("Bake blended specular channel of landscape textures into the alpha of the model-space normal texture. Applies to all LOD levels.");
        sharedRow->addWidget(m_chkBakeSpecular);
        sharedRow->addStretch();
        terrainLayout->addLayout(sharedRow);

        auto* defaultsRow = new QHBoxLayout();
        m_lblTerrainDefaultDiffuseSize = new QLabel("Default size: Diffuse:", m_grpTerrainOptions);
        defaultsRow->addWidget(m_lblTerrainDefaultDiffuseSize);
        m_cmbTerrainDefaultDiffuseSize = new QComboBox(m_grpTerrainOptions);
        m_cmbTerrainDefaultDiffuseSize->addItems({"None", "256", "512", "1024", "2048"});
        m_cmbTerrainDefaultDiffuseSize->setCurrentText("None");
        m_cmbTerrainDefaultDiffuseSize->setToolTip("Default diffuse texture size in case source is missing. None = use diffuse texture size. Applies to all LOD levels.");
        defaultsRow->addWidget(m_cmbTerrainDefaultDiffuseSize);
        defaultsRow->addSpacing(10);
        m_lblTerrainDefaultNormalSize = new QLabel("Normal:", m_grpTerrainOptions);
        defaultsRow->addWidget(m_lblTerrainDefaultNormalSize);
        m_cmbTerrainDefaultNormalSize = new QComboBox(m_grpTerrainOptions);
        m_cmbTerrainDefaultNormalSize->addItems({"None", "256", "512", "1024", "2048"});
        m_cmbTerrainDefaultNormalSize->setCurrentText("None");
        m_cmbTerrainDefaultNormalSize->setToolTip("Default normal texture size in case source is missing. None = use normal texture size. Applies to all LOD levels.");
        defaultsRow->addWidget(m_cmbTerrainDefaultNormalSize);
        defaultsRow->addStretch();
        terrainLayout->addLayout(defaultsRow);

        auto* vcRow = new QHBoxLayout();
        m_lblTerrainVertexColorMultiplier = new QLabel("Vertex Color Intensity:", m_grpTerrainOptions);
        vcRow->addWidget(m_lblTerrainVertexColorMultiplier);
        m_cmbTerrainVertexColorMultiplier = new QComboBox(m_grpTerrainOptions);
        for (int i = 0; i <= 200; i += 5)
            m_cmbTerrainVertexColorMultiplier->addItem(QString::number(i / 100.0, 'f', 2));
        m_cmbTerrainVertexColorMultiplier->setCurrentText("1.00");
        m_cmbTerrainVertexColorMultiplier->setToolTip("Vertex color intensity multiplier. 1.00 = 100%. Applies to all LOD levels.");
        vcRow->addWidget(m_cmbTerrainVertexColorMultiplier);
        vcRow->addStretch();
        terrainLayout->addLayout(vcRow);
    }
    layout->addWidget(m_grpTerrainOptions);

    // ==================================================================
    // Occlusion
    // ==================================================================
    m_chkOcclusion = new QCheckBox("Occlusion", panel);
    m_chkOcclusion->setChecked(false);
    m_chkOcclusion->setFont(boldFont);
    m_chkOcclusion->setToolTip("Create/Update TVDT LOD occlusion data in Occlusion.esp. Copies CELL records.");
    layout->addWidget(m_chkOcclusion);

    m_grpOcclusionOptions = new QGroupBox(panel);
    {
        auto* occLayout = new QGridLayout(m_grpOcclusionOptions);
        occLayout->setSpacing(4);

        occLayout->addWidget(new QLabel("Height:", m_grpOcclusionOptions), 0, 0);
        m_cmbOcclusionHeight = new QComboBox(m_grpOcclusionOptions);
        for (int i = 50; i <= 500; i += 10)
            m_cmbOcclusionHeight->addItem(QString::number(i));
        m_cmbOcclusionHeight->setCurrentText("150");
        m_cmbOcclusionHeight->setToolTip("Increase occlusion plane height. Set higher values in case there are visible holes in occlusion, e.g. looking from a high vantage point.");
        occLayout->addWidget(m_cmbOcclusionHeight, 0, 1);

        occLayout->addWidget(new QLabel("Quality:", m_grpOcclusionOptions), 0, 2);
        m_cmbOcclusionQuality = new QComboBox(m_grpOcclusionOptions);
        m_cmbOcclusionQuality->addItems({"1", "2", "3"});
        m_cmbOcclusionQuality->setCurrentText("2");
        m_cmbOcclusionQuality->setToolTip("Sampling quality");
        occLayout->addWidget(m_cmbOcclusionQuality, 0, 3);

        occLayout->addWidget(new QLabel("Radius:", m_grpOcclusionOptions), 1, 0);
        m_cmbOcclusionRadius = new QComboBox(m_grpOcclusionOptions);
        for (int i = 1; i <= 20; i++)
            m_cmbOcclusionRadius->addItem(QString::number(i));
        m_cmbOcclusionRadius->setCurrentText("5");
        m_cmbOcclusionRadius->setToolTip("Cells beyond this distance are always occluded");
        occLayout->addWidget(m_cmbOcclusionRadius, 1, 1);

        occLayout->addWidget(new QLabel("Mode:", m_grpOcclusionOptions), 1, 2);
        m_cmbOcclusionMode = new QComboBox(m_grpOcclusionOptions);
        m_cmbOcclusionMode->addItems({
            "All", "+Border", "-Flat", "-Flat +Border",
            "+TVDT", "+TVDT +Border", "+TVDT -Flat", "+TVDT -Flat +Border",
            "-TVDT", "-TVDT +Border", "-TVDT -Flat", "-TVDT -Flat +Border"
        });
        m_cmbOcclusionMode->setCurrentText("-Flat +Border");
        m_cmbOcclusionMode->setToolTip(
            "All = update all CELLs that have terrain LOD meshes\n"
            "+Border = only cells at the border\n"
            "-Flat = terrain that is not flat\n"
            "+TVDT = cells with existing TVDT record\n"
            "-TVDT = cells without existing TVDT record");
        occLayout->addWidget(m_cmbOcclusionMode, 1, 3);
    }
    layout->addWidget(m_grpOcclusionOptions);

    // Info label
    auto* lblInfo = new QLabel(
        "Only worldspaces that have existing lodsettings file \"LODSettings\\<Worldspace>.lod\" "
        "(.dlodsettings for Fallout3 and New Vegas) are shown either in the Data folder, or in "
        "BSA/BA2 archives. Archives are loaded similar to the game itself - the ones specified in "
        "the game ini file and those that match plugin names.", panel);
    lblInfo->setWordWrap(true);
    auto infoFont = lblInfo->font();
    infoFont.setPointSize(infoFont.pointSize() - 1);
    lblInfo->setFont(infoFont);
    layout->addWidget(lblInfo);

    layout->addStretch();

    scrollArea->setWidget(panel);
    return scrollArea;
}

// ==========================================================================
// Terrain LOD level panel (one per LOD 4/8/16/32)
// ==========================================================================
QWidget* LODGenDialog::createTerrainLevelPanel(int lodLevel, TerrainLevelWidgets& w)
{
    w.panel = new QWidget(m_grpTerrainOptions);
    auto* grid = new QGridLayout(w.panel);
    grid->setSpacing(4);
    grid->setContentsMargins(0, 0, 0, 0);

    int row = 0;

    // Quality
    grid->addWidget(new QLabel("Quality:", w.panel), row, 0);
    w.quality = new QComboBox(w.panel);
    for (int i = 0; i <= 20; i++)
        w.quality->addItem(QString::number(i));
    w.quality->setCurrentText("10");
    w.quality->setToolTip("Lower = Better / Larger Files, Higher = Worse / Smaller Files, -1 = No Optimization");
    grid->addWidget(w.quality, row, 1);

    // Max Vertices
    grid->addWidget(new QLabel("Max Vertices:", w.panel), row, 2);
    w.maxVerts = new QLineEdit(w.panel);
    w.maxVerts->setText("10000");
    w.maxVerts->setMaximumWidth(80);
    w.maxVerts->setToolTip("Max allowed number of vertices in case quality setting produces too many");
    grid->addWidget(w.maxVerts, row, 3);

    // Optimize Unseen (label - controlled by quality)
    grid->addWidget(new QLabel("Optimize Unseen:", w.panel), row, 4);
    w.optimizeUnseen = new QComboBox(w.panel);
    w.optimizeUnseen->addItems({"None", "Remove", "Clear"});
    w.optimizeUnseen->setCurrentText("None");
    grid->addWidget(w.optimizeUnseen, row, 5);

    row++;

    // Water Delta
    grid->addWidget(new QLabel("Water Delta:", w.panel), row, 0);
    w.waterDelta = new QComboBox(w.panel);
    w.waterDelta->addItems({"0", "100", "200", "300", "400", "500", "600", "700", "800", "900", "1000", "1500", "2000", "3000", "5000"});
    w.waterDelta->setCurrentText("1000");
    w.waterDelta->setToolTip("0 = merge large chunks of triangles under water resulting in less vertices. "
        "x = also move shallow vertices under water to be at least this distance below the water level. "
        "Improves coast lines and lessens z-fighting between LOD terrain and water.");
    grid->addWidget(w.waterDelta, row, 1);

    row++;

    // Diffuse: Size / Format / MipMap
    grid->addWidget(new QLabel("Diffuse:", w.panel), row, 0);
    auto* diffRow = new QHBoxLayout();
    diffRow->addWidget(new QLabel("Size:", w.panel));
    w.diffuseSize = new QComboBox(w.panel);
    w.diffuseSize->addItems({"128", "256", "512", "1024", "2048", "4096"});
    if (lodLevel <= 4)
        w.diffuseSize->setCurrentText("256");
    else if (lodLevel <= 8)
        w.diffuseSize->setCurrentText("512");
    else if (lodLevel <= 16)
        w.diffuseSize->setCurrentText("512");
    else
        w.diffuseSize->setCurrentText("1024");
    w.diffuseSize->setToolTip("Diffuse texture size");
    diffRow->addWidget(w.diffuseSize);
    diffRow->addWidget(new QLabel("Format:", w.panel));
    w.diffuseComp = new QComboBox(w.panel);
    w.diffuseComp->addItems(kCompressionFormats);
    w.diffuseComp->setCurrentText("DXT1");
    w.diffuseComp->setToolTip("Compression diffuse texture");
    diffRow->addWidget(w.diffuseComp);
    w.diffuseMipMap = new QCheckBox("MipMap", w.panel);
    w.diffuseMipMap->setToolTip("Create MipMaps");
    diffRow->addWidget(w.diffuseMipMap);
    diffRow->addStretch();
    grid->addLayout(diffRow, row, 1, 1, 5);

    row++;

    // Brightness / Contrast / Gamma
    auto* bcgRow = new QHBoxLayout();
    bcgRow->addWidget(new QLabel("Brightness:", w.panel));
    w.brightness = new QComboBox(w.panel);
    for (int i = -30; i <= 30; ++i)
        w.brightness->addItem(QString::number(i));
    w.brightness->setCurrentText("0");
    w.brightness->setToolTip("Negative darker, positive brighter");
    bcgRow->addWidget(w.brightness);
    bcgRow->addWidget(new QLabel("Contrast:", w.panel));
    w.contrast = new QComboBox(w.panel);
    for (int i = -30; i <= 30; ++i)
        w.contrast->addItem(QString::number(i));
    w.contrast->setCurrentText("0");
    w.contrast->setToolTip("Negative less contrast, positive more contrast");
    bcgRow->addWidget(w.contrast);
    bcgRow->addWidget(new QLabel("Gamma:", w.panel));
    w.gamma = new QComboBox(w.panel);
    // Gamma values from 0.10 to 3.00 in steps of 0.05
    for (int i = 10; i <= 300; i += 5)
        w.gamma->addItem(QString::number(i / 100.0, 'f', 2));
    w.gamma->setCurrentText("1.00");
    w.gamma->setToolTip("1.0 no change, lower values darker, higher values brighter");
    bcgRow->addWidget(w.gamma);
    bcgRow->addStretch();
    grid->addLayout(bcgRow, row, 0, 1, 6);

    row++;

    // Normal: Size / Format / MipMap / Raise steepness
    grid->addWidget(new QLabel("Normal:", w.panel), row, 0);
    auto* normRow = new QHBoxLayout();
    normRow->addWidget(new QLabel("Size:", w.panel));
    w.normalSize = new QComboBox(w.panel);
    w.normalSize->addItems({"128", "256", "512", "1024", "2048", "4096"});
    if (lodLevel <= 4)
        w.normalSize->setCurrentText("256");
    else if (lodLevel <= 8)
        w.normalSize->setCurrentText("512");
    else if (lodLevel <= 16)
        w.normalSize->setCurrentText("1024");
    else
        w.normalSize->setCurrentText("2048");
    w.normalSize->setToolTip(QString("Max normal texture size - game data is %1x%1").arg(lodLevel <= 4 ? 256 : (lodLevel <= 8 ? 512 : (lodLevel <= 16 ? 1024 : 2048))));
    normRow->addWidget(w.normalSize);
    normRow->addWidget(new QLabel("Format:", w.panel));
    w.normalComp = new QComboBox(w.panel);
    w.normalComp->addItems(kCompressionFormats);
    w.normalComp->setCurrentText("DXT1");
    w.normalComp->setToolTip("Compression normal texture");
    normRow->addWidget(w.normalComp);
    w.normalMipMap = new QCheckBox("MipMap", w.panel);
    w.normalMipMap->setToolTip("Create MipMaps");
    normRow->addWidget(w.normalMipMap);
    w.normalRaiseSteepness = new QCheckBox("Raise steepness", w.panel);
    w.normalRaiseSteepness->setToolTip("Increase steepness of model-space normals");
    normRow->addWidget(w.normalRaiseSteepness);
    normRow->addStretch();
    grid->addLayout(normRow, row, 1, 1, 5);

    row++;

    // LOD4-specific: Protect Borders, Hide Quads
    if (lodLevel == 4) {
        auto* lod4Row = new QHBoxLayout();
        w.protectBorders = new QCheckBox("Protect Borders", w.panel);
        w.protectBorders->setToolTip("Keep cell borders intact to avoid weird terrain drops at border between loaded cells and LOD. Slightly larger files.");
        lod4Row->addWidget(w.protectBorders);
        w.hideQuads = new QCheckBox("Hide Quads", w.panel);
        w.hideQuads->setToolTip("Do not generate terrain for quads set to hide");
        lod4Row->addWidget(w.hideQuads);
        lod4Row->addStretch();
        grid->addLayout(lod4Row, row, 0, 1, 6);
    } else {
        w.protectBorders = nullptr;
        w.hideQuads = nullptr;
    }

    return w.panel;
}

// ==========================================================================
// Bottom panel (output, chunk, seasons, buttons)
// ==========================================================================
QWidget* LODGenDialog::createBottomPanel()
{
    auto* panel = new QWidget(this);
    auto* layout = new QVBoxLayout(panel);
    layout->setContentsMargins(0, 4, 0, 0);
    layout->setSpacing(4);

    // Output directory row
    auto* outputLayout = new QHBoxLayout();
    outputLayout->addWidget(new QLabel("Output Directory:", panel));
    m_edOutputDir = new QLineEdit(panel);
    m_edOutputDir->setPlaceholderText("Default output location...");
    outputLayout->addWidget(m_edOutputDir, 1);
    m_btnBrowseOutput = new QPushButton("Browse...", panel);
    outputLayout->addWidget(m_btnBrowseOutput);
    layout->addLayout(outputLayout);

    // Chunk + Seasons + LODSettings + HeightMaps row
    auto* bottomRow = new QHBoxLayout();

    // Specific chunk
    m_chkChunk = new QCheckBox("Specific chunk", panel);
    m_chkChunk->setToolTip("Create LOD for a specific area");
    bottomRow->addWidget(m_chkChunk);

    m_chunkCoordsWidget = new QWidget(panel);
    auto* chunkLayout = new QHBoxLayout(m_chunkCoordsWidget);
    chunkLayout->setContentsMargins(0, 0, 0, 0);

    chunkLayout->addWidget(new QLabel("LOD Level:", m_chunkCoordsWidget));
    m_cmbChunkLODLevel = new QComboBox(m_chunkCoordsWidget);
    m_cmbChunkLODLevel->addItems({"", "4", "8", "16"});
    m_cmbChunkLODLevel->setToolTip("Dimension - number of cells");
    chunkLayout->addWidget(m_cmbChunkLODLevel);

    m_lblLODX1 = new QLabel("W:", m_chunkCoordsWidget);
    chunkLayout->addWidget(m_lblLODX1);
    m_edLODX = new QLineEdit(m_chunkCoordsWidget);
    m_edLODX->setMaximumWidth(50);
    m_edLODX->setToolTip("Lower left cell X");
    chunkLayout->addWidget(m_edLODX);

    m_lblLODY1 = new QLabel("S:", m_chunkCoordsWidget);
    chunkLayout->addWidget(m_lblLODY1);
    m_edLODY = new QLineEdit(m_chunkCoordsWidget);
    m_edLODY->setMaximumWidth(50);
    m_edLODY->setToolTip("Lower left cell Y");
    chunkLayout->addWidget(m_edLODY);

    m_lblLODX2 = new QLabel("E:", m_chunkCoordsWidget);
    chunkLayout->addWidget(m_lblLODX2);
    m_edLODX2 = new QLineEdit(m_chunkCoordsWidget);
    m_edLODX2->setMaximumWidth(50);
    m_edLODX2->setToolTip("Upper right cell X, calculated using size of LOD Level");
    chunkLayout->addWidget(m_edLODX2);

    m_lblLODY2 = new QLabel("N:", m_chunkCoordsWidget);
    chunkLayout->addWidget(m_lblLODY2);
    m_edLODY2 = new QLineEdit(m_chunkCoordsWidget);
    m_edLODY2->setMaximumWidth(50);
    m_edLODY2->setToolTip("Upper right cell Y, calculated using size of LOD Level");
    chunkLayout->addWidget(m_edLODY2);

    bottomRow->addWidget(m_chunkCoordsWidget);
    m_chunkCoordsWidget->setVisible(false);

    bottomRow->addSpacing(10);

    // Seasons
    m_chkSeasons = new QCheckBox("Seasons", panel);
    m_chkSeasons->setToolTip("Generate seasonal LOD textures");
    bottomRow->addWidget(m_chkSeasons);

    m_cmbSeasons = new QComboBox(panel);
    // Make it a checkable combo box using QStandardItemModel
    auto* seasonsModel = new QStandardItemModel(m_cmbSeasons);
    const QStringList seasonNames = {"Default", "SPR", "SUM", "AUT", "WIN"};
    for (const auto& name : seasonNames) {
        auto* item = new QStandardItem(name);
        item->setFlags(Qt::ItemIsUserCheckable | Qt::ItemIsEnabled);
        item->setData(Qt::Checked, Qt::CheckStateRole);
        seasonsModel->appendRow(item);
    }
    m_cmbSeasons->setModel(seasonsModel);
    m_cmbSeasons->setToolTip("Select seasons to generate");
    m_cmbSeasons->setVisible(false);
    bottomRow->addWidget(m_cmbSeasons);

    bottomRow->addSpacing(10);

    // LODSettings File button
    m_btnCreateLODSettings = new QPushButton("LODSettings File", panel);
    bottomRow->addWidget(m_btnCreateLODSettings);

    // Height maps
    m_chkHeightMaps = new QCheckBox("Height maps", panel);
    m_chkHeightMaps->setToolTip("Export height maps");
    bottomRow->addWidget(m_chkHeightMaps);

    bottomRow->addStretch();
    layout->addLayout(bottomRow);

    // Separator
    auto* separator = new QFrame(panel);
    separator->setFrameShape(QFrame::HLine);
    separator->setFrameShadow(QFrame::Sunken);
    layout->addWidget(separator);

    // Generate / Cancel buttons
    auto* btnLayout = new QHBoxLayout();
    btnLayout->addStretch();
    m_btnGenerate = new QPushButton("Generate", panel);
    m_btnGenerate->setDefault(true);
    m_btnCancel = new QPushButton("Cancel", panel);
    btnLayout->addWidget(m_btnGenerate);
    btnLayout->addWidget(m_btnCancel);
    layout->addLayout(btnLayout);

    return panel;
}

// ==========================================================================
// Worldspace management
// ==========================================================================
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

QString LODGenDialog::outputDirectory() const { return m_edOutputDir->text(); }
void LODGenDialog::setOutputDirectory(const QString& dir) { m_edOutputDir->setText(dir); }

// ==========================================================================
// JSON serialization for FFI
// ==========================================================================
static QJsonObject serializeTerrainLevel(const LODGenDialog::TerrainLevelWidgets& w)
{
    QJsonObject level;
    level["quality"]              = w.quality->currentText();
    level["max_vertices"]         = w.maxVerts->text();
    level["optimize_unseen"]      = w.optimizeUnseen->currentText();
    level["water_delta"]          = w.waterDelta->currentText();
    level["diffuse_size"]         = w.diffuseSize->currentText();
    level["diffuse_compression"]  = w.diffuseComp->currentText();
    level["diffuse_mipmap"]       = w.diffuseMipMap->isChecked();
    level["brightness"]           = w.brightness->currentText();
    level["contrast"]             = w.contrast->currentText();
    level["gamma"]                = w.gamma->currentText();
    level["normal_size"]          = w.normalSize->currentText();
    level["normal_compression"]   = w.normalComp->currentText();
    level["normal_mipmap"]        = w.normalMipMap->isChecked();
    level["normal_raise_steepness"] = w.normalRaiseSteepness->isChecked();
    if (w.protectBorders)
        level["protect_borders"]  = w.protectBorders->isChecked();
    if (w.hideQuads)
        level["hide_quads"]       = w.hideQuads->isChecked();
    return level;
}

QByteArray LODGenDialog::toJson() const
{
    QJsonObject obj;
    obj["worldspaces"] = QJsonArray::fromStringList(selectedWorldspaces());
    obj["output_dir"]  = m_edOutputDir->text();
    obj["game_mode"]   = m_gameMode;

    // LOD type flags
    obj["objects_lod"]  = m_chkObjectsLOD->isChecked();
    obj["trees_lod"]    = m_chkTreesLOD->isChecked();
    obj["terrain_lod"]  = m_chkTerrainLOD->isChecked();

    // LOD levels are determined by LOD settings file; send all standard levels
    QJsonArray levels;
    levels.append(4); levels.append(8); levels.append(16); levels.append(32);
    obj["lod_levels"] = levels;

    // Objects LOD options
    obj["build_atlas"]             = m_chkBuildAtlas->isChecked();
    obj["atlas_width"]             = m_cmbAtlasWidth->currentText().toInt();
    obj["atlas_height"]            = m_cmbAtlasHeight->currentText().toInt();
    obj["atlas_texture_size"]      = m_cmbAtlasTextureSize->currentText().toInt();
    obj["atlas_uv_range"]          = m_cmbAtlasUVRange->currentText().toDouble();
    obj["compression_diffuse"]     = m_cmbCompDiffuse->currentText();
    obj["compression_normal"]      = m_cmbCompNormal->currentText();
    obj["compression_specular"]    = m_cmbCompSpecular->currentText();
    obj["default_alpha_threshold"] = m_cmbDefaultAlphaThreshold->currentText().toInt();
    obj["use_alpha_threshold"]     = m_chkUseAlphaThreshold->isChecked();
    obj["use_backlight_power"]     = m_chkUseBacklightPower->isChecked();
    obj["no_tangents"]             = m_chkNoTangents->isChecked();
    obj["no_vertex_colors"]        = m_chkNoVertexColors->isChecked();
    obj["snow"]                    = m_chkSnow->isChecked();
    obj["keep_specular"]           = m_chkKeepSpecular->isChecked();

    // Trees LOD options
    obj["trees_brightness"]            = m_cmbTreesBrightness->currentText().toInt();
    obj["trees_3d"]                    = m_chkTrees3D->isChecked();
    obj["tree_normal_map"]             = m_chkTreeNormalMap->isChecked();
    obj["tree_compression_diffuse"]    = m_cmbTreeCompDiffuse->currentText();
    obj["tree_compression_normal"]     = m_cmbTreeCompNormal->currentText();

    // Terrain LOD options
    obj["build_meshes"]                    = m_chkBuildMeshes->isChecked();
    obj["build_diffuse_textures"]          = m_chkBuildDiffuseTextures->isChecked();
    obj["build_normal_textures"]           = m_chkBuildNormalTextures->isChecked();
    obj["bake_normal_maps"]                = m_chkBakeNormalMaps->isChecked();
    obj["bake_specular"]                   = m_chkBakeSpecular->isChecked();
    obj["terrain_default_diffuse_size"]    = m_cmbTerrainDefaultDiffuseSize->currentText();
    obj["terrain_default_normal_size"]     = m_cmbTerrainDefaultNormalSize->currentText();
    obj["terrain_vertex_color_multiplier"] = m_cmbTerrainVertexColorMultiplier->currentText();

    // Per-level terrain settings (nested objects)
    obj["terrain_lod4"]  = serializeTerrainLevel(m_terrainLOD4);
    obj["terrain_lod8"]  = serializeTerrainLevel(m_terrainLOD8);
    obj["terrain_lod16"] = serializeTerrainLevel(m_terrainLOD16);
    obj["terrain_lod32"] = serializeTerrainLevel(m_terrainLOD32);

    // Occlusion
    obj["occlusion"]         = m_chkOcclusion->isChecked();
    obj["occlusion_height"]  = m_cmbOcclusionHeight->currentText();
    obj["occlusion_quality"] = m_cmbOcclusionQuality->currentText();
    obj["occlusion_radius"]  = m_cmbOcclusionRadius->currentText();
    obj["occlusion_mode"]    = m_cmbOcclusionMode->currentText();

    // Specific chunk options
    obj["chunk"] = m_chkChunk->isChecked();
    if (m_chkChunk->isChecked()) {
        obj["chunk_lod_level"] = m_cmbChunkLODLevel->currentText();
        obj["chunk_x"]  = m_edLODX->text();
        obj["chunk_y"]  = m_edLODY->text();
        obj["chunk_x2"] = m_edLODX2->text();
        obj["chunk_y2"] = m_edLODY2->text();
    }

    // Seasons
    obj["seasons"] = m_chkSeasons->isChecked();
    if (m_chkSeasons->isChecked()) {
        QJsonArray seasonsList;
        auto* model = qobject_cast<QStandardItemModel*>(m_cmbSeasons->model());
        if (model) {
            for (int i = 0; i < model->rowCount(); i++) {
                auto* item = model->item(i);
                if (item && item->checkState() == Qt::Checked)
                    seasonsList.append(item->text());
            }
        }
        obj["seasons_list"] = seasonsList;
    }

    // Height maps
    obj["height_maps"] = m_chkHeightMaps->isChecked();

    return QJsonDocument(obj).toJson(QJsonDocument::Compact);
}

// ==========================================================================
// Slots
// ==========================================================================
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
    updateTerrainOptionsEnabled();
}

void LODGenDialog::onOcclusionToggled(bool)
{
    updateOcclusionOptionsEnabled();
}

void LODGenDialog::onChunkToggled(bool checked)
{
    m_chunkCoordsWidget->setVisible(checked);
}

void LODGenDialog::onSeasonsToggled(bool checked)
{
    m_cmbSeasons->setVisible(checked);
}

void LODGenDialog::onTrees3DToggled(bool)
{
    // Could enable/disable tree compression when trees are 3D vs 2D
}

void LODGenDialog::onTreeNormalMapToggled(bool checked)
{
    m_cmbTreeCompNormal->setEnabled(checked);
    m_lblTreeCompNormal->setEnabled(checked);
}

void LODGenDialog::onBakeNormalMapsToggled(bool checked)
{
    // When bake normal maps is enabled, auto-enable Raise steepness on all levels
    if (checked) {
        m_terrainLOD4.normalRaiseSteepness->setChecked(true);
        m_terrainLOD8.normalRaiseSteepness->setChecked(true);
        m_terrainLOD16.normalRaiseSteepness->setChecked(true);
        m_terrainLOD32.normalRaiseSteepness->setChecked(true);
    }
}

void LODGenDialog::onTerrainLevelChanged()
{
    int idx = m_cmbTerrainLevel->currentIndex();
    showTerrainLevelPanel(idx);
}

void LODGenDialog::showTerrainLevelPanel(int idx)
{
    m_terrainLOD4.panel->setVisible(idx == 0);
    m_terrainLOD8.panel->setVisible(idx == 1);
    m_terrainLOD16.panel->setVisible(idx == 2);
    m_terrainLOD32.panel->setVisible(idx == 3);
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

void LODGenDialog::setGenerating(bool active)
{
    m_generating = active;

    // Show/hide log panel
    m_logPanel->setVisible(active);

    // Lock/unlock all controls
    m_lstWorldspaces->setEnabled(!active);
    m_chkObjectsLOD->setEnabled(!active);
    m_chkTreesLOD->setEnabled(!active);
    m_chkTerrainLOD->setEnabled(!active);
    m_chkOcclusion->setEnabled(!active);
    m_grpObjectsOptions->setEnabled(!active);
    m_grpTreesOptions->setEnabled(!active);
    m_grpTerrainOptions->setEnabled(!active);
    m_grpOcclusionOptions->setEnabled(!active);
    m_edOutputDir->setEnabled(!active);
    m_btnBrowseOutput->setEnabled(!active);
    m_chkChunk->setEnabled(!active);
    m_chkSeasons->setEnabled(!active);
    m_chkHeightMaps->setEnabled(!active);

    // Switch Generate button to Cancel during generation
    m_btnGenerate->setText(active ? "Cancel" : "Generate");
    m_btnCancel->setEnabled(!active);

    if (active) {
        m_logOutput->clear();
        m_progressBar->setValue(0);
        // Grow the dialog to show the log
        resize(width(), height() + 200);
    }
}

void LODGenDialog::appendLog(const QString& text)
{
    m_logOutput->appendPlainText(text);
    // Auto-scroll to bottom
    auto* sb = m_logOutput->verticalScrollBar();
    if (sb) sb->setValue(sb->maximum());
}

// Static callback bridge: LODGenDialog pointer is stored in a thread-local for the C callback.
// We use a global pointer since there's only one LODGen dialog at a time.
static LODGenDialog* s_activeDialog = nullptr;

static void lodLogCallback(const char* msg)
{
    if (!s_activeDialog || !msg) return;
    QString text = QString::fromUtf8(msg);
    // Post to the GUI thread
    QMetaObject::invokeMethod(s_activeDialog, [text]() {
        if (s_activeDialog)
            s_activeDialog->appendLog(text);
    }, Qt::QueuedConnection);
}

void LODGenDialog::onGenerateClicked()
{
    // If currently generating, act as Cancel button
    if (m_generating) {
        auto& ffi = XEditFFI::instance();
        if (ffi.xedit_lod_cancel)
            ffi.xedit_lod_cancel();
        appendLog("[Cancelling LOD generation...]");
        setGenerating(false);
        return;
    }

    if (selectedWorldspaces().isEmpty()) {
        QMessageBox::warning(this, "LOD Generation",
            "Select worldspace(s) for LOD generation.");
        return;
    }

    if (outputDirectory().isEmpty()) {
        QMessageBox::warning(this, "LOD Generation",
            "Please specify an output directory.");
        return;
    }

    auto& ffi = XEditFFI::instance();
    if (!ffi.isLoaded() || !ffi.xedit_generate_lod) {
        QMessageBox::warning(this, "LOD Generation",
            "Rust backend not loaded. Please load plugins first.");
        return;
    }

    setGenerating(true);
    appendLog("[LOD] Starting LOD generation...");

    // Serialize config to JSON
    QByteArray jsonConfig = toJson();
    appendLog("[LOD] Config: " + QString::number(jsonConfig.size()) + " bytes");

    // Set up the static callback bridge
    s_activeDialog = this;

    // Run LOD generation in a background thread
    auto* future = new QFutureWatcher<int32_t>(this);
    connect(future, &QFutureWatcher<int32_t>::finished, this, [this, future]() {
        int32_t result = future->result();
        s_activeDialog = nullptr;
        if (result == 0) {
            appendLog("[LOD] === Generation completed successfully ===");
        } else if (result == -100) {
            appendLog("[LOD] Generation cancelled by user.");
        } else {
            appendLog(QString("[LOD] Generation failed with error code: %1").arg(result));
        }
        setGenerating(false);
        future->deleteLater();
    });

    QByteArray configCopy = jsonConfig; // ensure lifetime
    future->setFuture(QtConcurrent::run([configCopy]() -> int32_t {
        auto& ffi = XEditFFI::instance();
        return ffi.xedit_generate_lod(
            configCopy.constData(),
            reinterpret_cast<void*>(lodLogCallback)
        );
    }));
}

void LODGenDialog::launchLodGenProcess()
{
    // Find LODGenx64 binary: check lodgen/ subdir first, then next to executable
    QString exeDir = QCoreApplication::applicationDirPath();
    m_lodGenPath = exeDir + "/lodgen/LODGenx64";
    if (!QFile::exists(m_lodGenPath)) {
        m_lodGenPath = exeDir + "/LODGenx64";
        if (!QFile::exists(m_lodGenPath)) {
            appendLog("[ERROR] LODGenx64 not found. Checked:");
            appendLog("  " + exeDir + "/lodgen/LODGenx64");
            appendLog("  " + exeDir + "/LODGenx64");
            m_lodGenPath.clear();
            setGenerating(false);
            return;
        }
    }

    // LODGen.txt should be in the output directory
    QString lodGenTxt = outputDirectory() + "/LODGen.txt";
    if (!QFile::exists(lodGenTxt)) {
        appendLog("[ERROR] LODGen.txt not found at: " + lodGenTxt);
        setGenerating(false);
        return;
    }

    m_isTerrainPass = false;
    m_lodProcess = new QProcess(this);

    connect(m_lodProcess, &QProcess::readyReadStandardOutput, this, [this]() {
        QString output = QString::fromUtf8(m_lodProcess->readAllStandardOutput());
        for (const QString& line : output.split('\n', Qt::SkipEmptyParts)) {
            appendLog(line.trimmed());
        }
    });

    connect(m_lodProcess, &QProcess::readyReadStandardError, this, [this]() {
        QString output = QString::fromUtf8(m_lodProcess->readAllStandardError());
        for (const QString& line : output.split('\n', Qt::SkipEmptyParts)) {
            appendLog("[stderr] " + line.trimmed());
        }
    });

    connect(m_lodProcess, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
        this, [this](int exitCode, QProcess::ExitStatus exitStatus) {
        appendLog("");
        appendLog(QString("[DEBUG] LODGenx64 finished: exitCode=%1 exitStatus=%2 isTerrainPass=%3")
            .arg(exitCode).arg(exitStatus == QProcess::NormalExit ? "Normal" : "Crashed").arg(m_isTerrainPass));
        if (exitStatus == QProcess::NormalExit && exitCode == 0) {
            if (!m_isTerrainPass) {
                // Objects pass finished — check for terrain LOD
                QString terrainTxt = outputDirectory() + "/LODGen_terrain.txt";
                appendLog("[DEBUG] Checking for terrain LODGen: " + terrainTxt + " exists=" + (QFile::exists(terrainTxt) ? "YES" : "NO"));
                if (QFile::exists(terrainTxt)) {
                    appendLog("[Objects LOD completed successfully, starting terrain LOD...]");
                    appendLog("");
                    m_isTerrainPass = true;

                    m_lodProcess->deleteLater();
                    m_lodProcess = new QProcess(this);

                    connect(m_lodProcess, &QProcess::readyReadStandardOutput, this, [this]() {
                        QString out = QString::fromUtf8(m_lodProcess->readAllStandardOutput());
                        for (const QString& line : out.split('\n', Qt::SkipEmptyParts))
                            appendLog(line.trimmed());
                    });
                    connect(m_lodProcess, &QProcess::readyReadStandardError, this, [this]() {
                        QString out = QString::fromUtf8(m_lodProcess->readAllStandardError());
                        for (const QString& line : out.split('\n', Qt::SkipEmptyParts))
                            appendLog("[stderr] " + line.trimmed());
                    });
                    connect(m_lodProcess, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
                        this, [this](int exitCode2, QProcess::ExitStatus exitStatus2) {
                        appendLog("");
                        if (exitStatus2 == QProcess::NormalExit && exitCode2 == 0) {
                            m_progressBar->setValue(100);
                            m_progressBar->setFormat("Complete - 100%");
                            appendLog("[Terrain LOD generation completed successfully]");
                        } else {
                            appendLog(QString("[ERROR] LODGenx64 (terrain) exited with code %1").arg(exitCode2));
                        }
                        m_lodProcess->deleteLater();
                        m_lodProcess = nullptr;
                        setGenerating(false);
                    });

                    QStringList terrainArgs;
                    terrainArgs << terrainTxt;
                    appendLog("Running terrain pass: " + m_lodGenPath + " " + terrainArgs.join(" "));
                    connect(m_lodProcess, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
                        appendLog(QString("[ERROR] Terrain LODGenx64 process error: %1").arg(error));
                    });
                    m_lodProcess->start(m_lodGenPath, terrainArgs);
                    if (!m_lodProcess->waitForStarted(5000)) {
                        appendLog("[ERROR] Failed to start terrain LODGenx64: " + m_lodProcess->errorString());
                    }
                    return;
                }
            }
            // No terrain pass needed, or terrain pass just finished
            m_progressBar->setValue(100);
            m_progressBar->setFormat("Complete - 100%");
            appendLog("[LOD generation completed successfully]");
        } else {
            QString passName = m_isTerrainPass ? "terrain" : "objects";
            appendLog(QString("[ERROR] LODGenx64 (%1) exited with code %2").arg(passName).arg(exitCode));
        }
        m_lodProcess->deleteLater();
        m_lodProcess = nullptr;
        setGenerating(false);
    });

    QStringList args;
    args << lodGenTxt;
    if (m_chkNoVertexColors->isChecked())
        args << "--dontGenerateVertexColors";
    if (m_chkNoTangents->isChecked())
        args << "--dontGenerateTangents";

    appendLog("Running: " + m_lodGenPath + " " + args.join(" "));
    m_lodProcess->start(m_lodGenPath, args);
}

// ==========================================================================
// Enable/disable helpers
// ==========================================================================
void LODGenDialog::updateObjectsOptionsEnabled()
{
    bool enabled = m_chkObjectsLOD->isChecked();
    m_grpObjectsOptions->setEnabled(enabled);
    m_grpObjectsOptions->setVisible(enabled);
}

void LODGenDialog::updateTreesOptionsEnabled()
{
    bool enabled = m_chkTreesLOD->isChecked();
    m_grpTreesOptions->setEnabled(enabled);
    m_grpTreesOptions->setVisible(enabled);
}

void LODGenDialog::updateTerrainOptionsEnabled()
{
    bool enabled = m_chkTerrainLOD->isChecked();
    m_grpTerrainOptions->setEnabled(enabled);
    m_grpTerrainOptions->setVisible(enabled);
}

void LODGenDialog::updateOcclusionOptionsEnabled()
{
    bool enabled = m_chkOcclusion->isChecked();
    m_grpOcclusionOptions->setEnabled(enabled);
    m_grpOcclusionOptions->setVisible(enabled);
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

    // Trees 3D only for Skyrim — force off for other games
    m_chkTrees3D->setVisible(isSkyrim);
    if (!isSkyrim) {
        m_chkTrees3D->setChecked(false);
    }

    // FO3/FNV: E/N coordinate fields visible
    m_lblLODX2->setVisible(isFO3FNV);
    m_edLODX2->setVisible(isFO3FNV);
    m_lblLODY2->setVisible(isFO3FNV);
    m_edLODY2->setVisible(isFO3FNV);

    // FO3/FNV: atlas is mandatory
    if (isFO3FNV) {
        m_chkBuildAtlas->setChecked(true);
        m_chkBuildAtlas->setEnabled(false);
    } else {
        m_chkBuildAtlas->setEnabled(true);
    }

    // FO3/FNV: hide texture size, UV range, no tangents, no vertex colors
    m_lblMaxTextureSize->setVisible(!isFO3FNV);
    m_cmbAtlasTextureSize->setVisible(!isFO3FNV);
    m_lblUVRange->setVisible(!isFO3FNV);
    m_cmbAtlasUVRange->setVisible(!isFO3FNV);
    m_chkNoTangents->setVisible(!isFO3FNV);
    m_chkNoVertexColors->setVisible(!isFO3FNV);

    // FO3/FNV: use 4096 atlas size (2048 is too small, produces many sheets)
    if (isFO3FNV) {
        m_cmbAtlasWidth->setCurrentText("4096");
        m_cmbAtlasHeight->setCurrentText("4096");
    }

    // FO3/FNV: disable alpha threshold
    m_cmbDefaultAlphaThreshold->setEnabled(!isFO3FNV);

    // Alpha threshold conditional enable
    updateAlphaThresholdEnabled();
}

void LODGenDialog::updateAlphaThresholdEnabled()
{
    bool isFO4 = (m_gameMode == "Fallout4");
    bool isFO3FNV = (m_gameMode == "Fallout3" || m_gameMode == "FalloutNV");
    QString diffuse = m_cmbCompDiffuse->currentText();

    if (isFO3FNV) {
        m_cmbDefaultAlphaThreshold->setEnabled(false);
    } else if (isFO4) {
        m_cmbDefaultAlphaThreshold->setEnabled(true);
    } else {
        // Skyrim: only enable when diffuse is DXT1
        m_cmbDefaultAlphaThreshold->setEnabled(diffuse == "DXT1");
    }

    // "Use source alpha threshold" (FO4 only): enable when diffuse is 8888/DXT3/DXT5
    if (isFO4) {
        bool alphaFormat = (diffuse == "8888" || diffuse == "DXT3" || diffuse == "DXT5");
        m_chkUseAlphaThreshold->setEnabled(alphaFormat);
    }
}
