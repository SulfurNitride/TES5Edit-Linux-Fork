#include "LODGenDialog.h"
#include <QFileDialog>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QMessageBox>
#include <QSplitter>
#include <QVBoxLayout>

LODGenDialog::LODGenDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("LOD Generation");
    resize(600, 500);
    setupUI();
}

void LODGenDialog::setupUI()
{
    auto* mainLayout = new QVBoxLayout(this);

    // Top label
    auto* lblHeader = new QLabel("Select worldspace(s) to generate LOD for", this);
    auto headerFont = lblHeader->font();
    headerFont.setBold(true);
    lblHeader->setFont(headerFont);
    mainLayout->addWidget(lblHeader);

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

    // Info label (matching the Delphi form's description)
    auto* lblInfo = new QLabel(
        "Only worldspaces that have existing lodsettings file "
        "\"LODSettings\\<Worldspace>.lod\" are shown, either in the "
        "Data folder or in BSA/BA2 archives.", this);
    lblInfo->setWordWrap(true);
    mainLayout->addWidget(lblInfo);

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
}

QWidget* LODGenDialog::createWorldspacePanel()
{
    auto* panel = new QWidget(this);
    auto* layout = new QVBoxLayout(panel);
    layout->setContentsMargins(0, 0, 0, 0);

    m_lstWorldspaces = new QListWidget(panel);
    m_lstWorldspaces->setSelectionMode(QAbstractItemView::ExtendedSelection);
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
    auto* panel = new QWidget(this);
    auto* layout = new QVBoxLayout(panel);
    layout->setContentsMargins(0, 0, 0, 0);

    // LOD type checkboxes
    m_chkObjectsLOD = new QCheckBox("Objects LOD", panel);
    m_chkTreesLOD = new QCheckBox("Trees LOD", panel);
    m_chkTerrainLOD = new QCheckBox("Terrain LOD", panel);
    m_chkObjectsLOD->setChecked(true);
    m_chkTreesLOD->setChecked(true);
    m_chkTerrainLOD->setChecked(false);

    auto objFont = m_chkObjectsLOD->font();
    objFont.setBold(true);
    m_chkObjectsLOD->setFont(objFont);
    m_chkTreesLOD->setFont(objFont);
    m_chkTerrainLOD->setFont(objFont);

    layout->addWidget(m_chkObjectsLOD);

    // Objects LOD options group
    m_grpObjectsOptions = new QGroupBox(panel);
    auto* objLayout = new QVBoxLayout(m_grpObjectsOptions);

    m_chkBuildAtlas = new QCheckBox("Build atlas", m_grpObjectsOptions);
    m_chkBuildAtlas->setChecked(true);
    m_chkBuildAtlas->setToolTip("Put LOD textures on texture atlas(es) for better performance");
    objLayout->addWidget(m_chkBuildAtlas);

    // Atlas size row
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

    // Max texture size row
    auto* texRow = new QHBoxLayout();
    texRow->addWidget(new QLabel("Max texture size:", m_grpObjectsOptions));
    m_cmbAtlasTextureSize = new QComboBox(m_grpObjectsOptions);
    for (int sz = 256; sz <= 1024; sz *= 2)
        m_cmbAtlasTextureSize->addItem(QString::number(sz));
    m_cmbAtlasTextureSize->setCurrentIndex(1);  // 512
    texRow->addWidget(m_cmbAtlasTextureSize);
    texRow->addStretch();
    objLayout->addLayout(texRow);

    // Compression row
    auto* compRow = new QHBoxLayout();
    compRow->addWidget(new QLabel("Compression:", m_grpObjectsOptions));
    const QStringList compressionFormats = {"888", "8888", "565", "DXT1", "DXT3", "DXT5", "BC4", "BC5"};
    m_cmbCompDiffuse = new QComboBox(m_grpObjectsOptions);
    m_cmbCompDiffuse->addItems(compressionFormats);
    m_cmbCompDiffuse->setCurrentText("DXT1");
    compRow->addWidget(new QLabel("Diffuse:", m_grpObjectsOptions));
    compRow->addWidget(m_cmbCompDiffuse);
    m_cmbCompNormal = new QComboBox(m_grpObjectsOptions);
    m_cmbCompNormal->addItems(compressionFormats);
    m_cmbCompNormal->setCurrentText("DXT1");
    compRow->addWidget(new QLabel("Normal:", m_grpObjectsOptions));
    compRow->addWidget(m_cmbCompNormal);
    compRow->addStretch();
    objLayout->addLayout(compRow);

    // Additional options
    m_chkNoVertexColors = new QCheckBox("No vertex colors", m_grpObjectsOptions);
    m_chkNoVertexColors->setToolTip("Reduces size of generated LOD at the expense of quality");
    objLayout->addWidget(m_chkNoVertexColors);

    m_chkNoTangents = new QCheckBox("No tangents", m_grpObjectsOptions);
    m_chkNoTangents->setToolTip("Reduces size of generated LOD at the expense of quality");
    objLayout->addWidget(m_chkNoTangents);

    layout->addWidget(m_grpObjectsOptions);

    // LOD level checkboxes
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

    // Trees LOD
    layout->addWidget(m_chkTreesLOD);

    auto* treesRow = new QHBoxLayout();
    treesRow->addWidget(new QLabel("LOD brightness:", panel));
    m_cmbTreesBrightness = new QComboBox(panel);
    for (int i = -30; i <= 30; ++i)
        m_cmbTreesBrightness->addItem(QString::number(i));
    m_cmbTreesBrightness->setCurrentText("0");
    treesRow->addWidget(m_cmbTreesBrightness);
    m_chkTrees3D = new QCheckBox("Generate as Objects LOD", panel);
    m_chkTrees3D->setChecked(true);
    m_chkTrees3D->setToolTip(
        "Insert trees into Objects LOD by using provided LOD nifs or placing "
        "billboard image on a flat mesh.");
    treesRow->addWidget(m_chkTrees3D);
    treesRow->addStretch();
    layout->addLayout(treesRow);

    // Terrain LOD
    layout->addWidget(m_chkTerrainLOD);
    layout->addStretch();

    // Initial state
    updateTreesOptionsEnabled();

    return panel;
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

// ---------------------------------------------------------------------------
// Getters
// ---------------------------------------------------------------------------
bool LODGenDialog::objectsLOD() const       { return m_chkObjectsLOD->isChecked(); }
bool LODGenDialog::treesLOD() const         { return m_chkTreesLOD->isChecked(); }
bool LODGenDialog::terrainLOD() const       { return m_chkTerrainLOD->isChecked(); }

bool LODGenDialog::lodLevel4() const        { return m_chkLOD4->isChecked(); }
bool LODGenDialog::lodLevel8() const        { return m_chkLOD8->isChecked(); }
bool LODGenDialog::lodLevel16() const       { return m_chkLOD16->isChecked(); }
bool LODGenDialog::lodLevel32() const       { return m_chkLOD32->isChecked(); }

bool LODGenDialog::buildAtlas() const       { return m_chkBuildAtlas->isChecked(); }
bool LODGenDialog::noTangents() const       { return m_chkNoTangents->isChecked(); }
bool LODGenDialog::noVertexColors() const   { return m_chkNoVertexColors->isChecked(); }

int LODGenDialog::atlasWidth() const        { return m_cmbAtlasWidth->currentText().toInt(); }
int LODGenDialog::atlasHeight() const       { return m_cmbAtlasHeight->currentText().toInt(); }
int LODGenDialog::atlasTextureSize() const  { return m_cmbAtlasTextureSize->currentText().toInt(); }

QString LODGenDialog::compressionDiffuse() const { return m_cmbCompDiffuse->currentText(); }
QString LODGenDialog::compressionNormal() const  { return m_cmbCompNormal->currentText(); }

int LODGenDialog::treesLODBrightness() const { return m_cmbTreesBrightness->currentText().toInt(); }
bool LODGenDialog::trees3D() const           { return m_chkTrees3D->isChecked(); }

QString LODGenDialog::outputDirectory() const       { return m_edOutputDir->text(); }
void LODGenDialog::setOutputDirectory(const QString& dir) { m_edOutputDir->setText(dir); }

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------
void LODGenDialog::onObjectsLODToggled(bool checked)
{
    updateObjectsOptionsEnabled();
    Q_UNUSED(checked);
}

void LODGenDialog::onTreesLODToggled(bool checked)
{
    updateTreesOptionsEnabled();
    Q_UNUSED(checked);
}

void LODGenDialog::onTerrainLODToggled(bool /*checked*/)
{
    // Terrain LOD options could be expanded here in the future
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

void LODGenDialog::onGenerateClicked()
{
    if (selectedWorldspaces().isEmpty()) {
        QMessageBox::warning(this, "LOD Generation",
            "Select worldspace(s) for LOD generation.");
        return;
    }
    accept();
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
