#pragma once
#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QGroupBox>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
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
    void onBrowseOutputDir();
    void onSelectAllWorldspaces();
    void onSelectNoneWorldspaces();
    void onGenerateClicked();
    void onChunkToggled(bool checked);

private:
    void setupUI();
    QWidget* createWorldspacePanel();
    QWidget* createOptionsPanel();
    void updateObjectsOptionsEnabled();
    void updateTreesOptionsEnabled();
    void applyGameModeVisibility();

    QString m_gameMode;

    // Worldspace list
    QListWidget* m_lstWorldspaces;

    // LOD type checkboxes
    QCheckBox* m_chkObjectsLOD;
    QCheckBox* m_chkTreesLOD;
    QCheckBox* m_chkTerrainLOD;

    // LOD level checkboxes
    QCheckBox* m_chkLOD4;
    QCheckBox* m_chkLOD8;
    QCheckBox* m_chkLOD16;
    QCheckBox* m_chkLOD32;

    // Objects LOD options group
    QGroupBox* m_grpObjectsOptions;
    QCheckBox* m_chkBuildAtlas;
    QCheckBox* m_chkNoTangents;
    QCheckBox* m_chkNoVertexColors;
    QComboBox* m_cmbAtlasWidth;
    QComboBox* m_cmbAtlasHeight;
    QComboBox* m_cmbAtlasTextureSize;
    QComboBox* m_cmbAtlasUVRange;
    QComboBox* m_cmbCompDiffuse;
    QComboBox* m_cmbCompNormal;
    QComboBox* m_cmbCompSpecular;
    QLabel* m_lblSpecular;
    QComboBox* m_cmbDefaultAlphaThreshold;

    // FO4-specific
    QCheckBox* m_chkUseAlphaThreshold;
    QCheckBox* m_chkUseBacklightPower;

    // Specific chunk
    QCheckBox* m_chkChunk;
    QComboBox* m_cmbChunkLODLevel;
    QLineEdit* m_edLODX;
    QLineEdit* m_edLODY;
    QLineEdit* m_edLODX2;
    QLineEdit* m_edLODY2;
    QLabel* m_lblE;
    QLabel* m_lblN;
    QWidget* m_chunkCoordsWidget;

    // Trees LOD options
    QComboBox* m_cmbTreesBrightness;
    QCheckBox* m_chkTrees3D;

    // Output directory
    QLineEdit* m_edOutputDir;
    QPushButton* m_btnBrowseOutput;

    // Bottom buttons
    QPushButton* m_btnGenerate;
    QPushButton* m_btnCancel;
};
