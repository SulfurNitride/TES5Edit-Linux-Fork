#pragma once
#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QGroupBox>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QPlainTextEdit>
#include <QProcess>
#include <QProgressBar>
#include <QPushButton>
#include <QSpinBox>
#include <QStringList>

class LODGenDialog : public QDialog {
    Q_OBJECT
public:
    explicit LODGenDialog(QWidget* parent = nullptr);

    // Worldspace management
    void setWorldspaces(const QStringList& worldspaces);
    QStringList selectedWorldspaces() const;

    // Set game mode to show/hide game-specific controls
    // "SkyrimSE", "FalloutNV", "Fallout3", "Fallout4", "Oblivion", etc.
    void setGameMode(const QString& gameMode);

    // Output directory
    QString outputDirectory() const;
    void setOutputDirectory(const QString& dir);

    // Serialize all options to JSON for FFI
    QByteArray toJson() const;

private slots:
    void onObjectsLODToggled(bool checked);
    void onTreesLODToggled(bool checked);
    void onTerrainLODToggled(bool checked);
    void onOcclusionToggled(bool checked);
    void onBrowseOutputDir();
    void onSelectAllWorldspaces();
    void onSelectNoneWorldspaces();
    void onGenerateClicked();
    void onChunkToggled(bool checked);
    void onTerrainLevelChanged();
    void onSeasonsToggled(bool checked);
    void onTrees3DToggled(bool checked);
    void onTreeNormalMapToggled(bool checked);
    void onBakeNormalMapsToggled(bool checked);

public:
    // Terrain LOD level widget struct (public for serialization helper)
    struct TerrainLevelWidgets {
        QWidget* panel;
        QComboBox* quality;
        QLineEdit* maxVerts;
        QComboBox* optimizeUnseen; // label only, controlled by quality
        QComboBox* waterDelta;
        // Diffuse
        QComboBox* diffuseSize;
        QComboBox* diffuseComp;
        QCheckBox* diffuseMipMap;
        // Brightness/Contrast/Gamma
        QComboBox* brightness;
        QComboBox* contrast;
        QComboBox* gamma;
        // Normal
        QComboBox* normalSize;
        QComboBox* normalComp;
        QCheckBox* normalMipMap;
        QCheckBox* normalRaiseSteepness;
        // LOD4-only
        QCheckBox* protectBorders;  // nullptr for LOD8/16/32
        QCheckBox* hideQuads;       // nullptr for LOD8/16/32
    };

private:
    void setupUI();
    QWidget* createWorldspacePanel();
    QWidget* createOptionsPanel();
    QWidget* createBottomPanel();
    void updateObjectsOptionsEnabled();
    void updateTreesOptionsEnabled();
    void updateTerrainOptionsEnabled();
    void updateOcclusionOptionsEnabled();
    void applyGameModeVisibility();
    void updateAlphaThresholdEnabled();
    void showTerrainLevelPanel(int level);
    QWidget* createTerrainLevelPanel(int lodLevel, TerrainLevelWidgets& widgets);

    QString m_gameMode;

    // Worldspace list
    QListWidget* m_lstWorldspaces;

    // ---- Objects LOD ----
    QCheckBox* m_chkObjectsLOD;
    QGroupBox* m_grpObjectsOptions;
    QCheckBox* m_chkBuildAtlas;
    QCheckBox* m_chkNoTangents;
    QCheckBox* m_chkNoVertexColors;
    QComboBox* m_cmbAtlasWidth;
    QComboBox* m_cmbAtlasHeight;
    QComboBox* m_cmbAtlasTextureSize;
    QLabel* m_lblMaxTextureSize;
    QComboBox* m_cmbAtlasUVRange;
    QLabel* m_lblUVRange;
    QComboBox* m_cmbCompDiffuse;
    QComboBox* m_cmbCompNormal;
    QComboBox* m_cmbCompSpecular;
    QLabel* m_lblSpecular;
    QComboBox* m_cmbDefaultAlphaThreshold;
    QCheckBox* m_chkUseAlphaThreshold;
    QCheckBox* m_chkUseBacklightPower;
    QCheckBox* m_chkSnow;
    QCheckBox* m_chkKeepSpecular;

    // ---- Trees LOD ----
    QCheckBox* m_chkTreesLOD;
    QGroupBox* m_grpTreesOptions;
    QComboBox* m_cmbTreesBrightness;
    QCheckBox* m_chkTrees3D;
    QCheckBox* m_chkTreeNormalMap;
    QComboBox* m_cmbTreeCompDiffuse;
    QComboBox* m_cmbTreeCompNormal;
    QLabel* m_lblTreeCompDiffuse;
    QLabel* m_lblTreeCompNormal;
    QPushButton* m_btnSplitTreesLOD;

    // ---- Terrain LOD ----
    QCheckBox* m_chkTerrainLOD;
    QGroupBox* m_grpTerrainOptions;
    QComboBox* m_cmbTerrainLevel;
    QCheckBox* m_chkBuildMeshes;
    QCheckBox* m_chkBuildDiffuseTextures;
    QCheckBox* m_chkBuildNormalTextures;
    // Per-LOD-level panels
    TerrainLevelWidgets m_terrainLOD4;
    TerrainLevelWidgets m_terrainLOD8;
    TerrainLevelWidgets m_terrainLOD16;
    TerrainLevelWidgets m_terrainLOD32;
    // Shared terrain options
    QCheckBox* m_chkBakeNormalMaps;
    QCheckBox* m_chkBakeSpecular;
    QComboBox* m_cmbTerrainDefaultDiffuseSize;
    QComboBox* m_cmbTerrainDefaultNormalSize;
    QComboBox* m_cmbTerrainVertexColorMultiplier;
    QLabel* m_lblTerrainDefaultDiffuseSize;
    QLabel* m_lblTerrainDefaultNormalSize;
    QLabel* m_lblTerrainVertexColorMultiplier;

    // ---- Occlusion ----
    QCheckBox* m_chkOcclusion;
    QGroupBox* m_grpOcclusionOptions;
    QComboBox* m_cmbOcclusionHeight;
    QComboBox* m_cmbOcclusionQuality;
    QComboBox* m_cmbOcclusionRadius;
    QComboBox* m_cmbOcclusionMode;

    // ---- Bottom panel ----
    // Specific chunk
    QCheckBox* m_chkChunk;
    QComboBox* m_cmbChunkLODLevel;
    QLineEdit* m_edLODX;
    QLineEdit* m_edLODY;
    QLineEdit* m_edLODX2;
    QLineEdit* m_edLODY2;
    QLabel* m_lblLODX1;
    QLabel* m_lblLODY1;
    QLabel* m_lblLODX2;
    QLabel* m_lblLODY2;
    QWidget* m_chunkCoordsWidget;

    // Seasons
    QCheckBox* m_chkSeasons;
    QComboBox* m_cmbSeasons;  // multi-select via checkable items

    // LODSettings File button
    QPushButton* m_btnCreateLODSettings;

    // Height maps
    QCheckBox* m_chkHeightMaps;

    // Output directory
    QLineEdit* m_edOutputDir;
    QPushButton* m_btnBrowseOutput;

    // Bottom buttons
    QPushButton* m_btnGenerate;
    QPushButton* m_btnCancel;

    // Log / progress area (shown during generation)
    QPlainTextEdit* m_logOutput;
    QProgressBar* m_progressBar;
    QWidget* m_logPanel;

    // State
    bool m_generating = false;
    bool m_isTerrainPass = false;
    QString m_lodGenPath;
    QProcess* m_lodProcess = nullptr;
    void setGenerating(bool active);
    void launchLodGenProcess();
public:
    void appendLog(const QString& text);
};
