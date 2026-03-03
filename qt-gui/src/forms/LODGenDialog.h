#pragma once
#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QGroupBox>
#include <QLineEdit>
#include <QListWidget>
#include <QPushButton>
#include <QStringList>

class LODGenDialog : public QDialog {
    Q_OBJECT
public:
    explicit LODGenDialog(QWidget* parent = nullptr);

    // Worldspace management
    void setWorldspaces(const QStringList& worldspaces);
    QStringList selectedWorldspaces() const;

    // LOD type getters
    bool objectsLOD() const;
    bool treesLOD() const;
    bool terrainLOD() const;

    // LOD level getters
    bool lodLevel4() const;
    bool lodLevel8() const;
    bool lodLevel16() const;
    bool lodLevel32() const;

    // Objects LOD options
    bool buildAtlas() const;
    bool noTangents() const;
    bool noVertexColors() const;

    // Atlas options
    int atlasWidth() const;
    int atlasHeight() const;
    int atlasTextureSize() const;
    QString compressionDiffuse() const;
    QString compressionNormal() const;

    // Trees LOD options
    int treesLODBrightness() const;
    bool trees3D() const;

    // Output directory
    QString outputDirectory() const;
    void setOutputDirectory(const QString& dir);

private slots:
    void onObjectsLODToggled(bool checked);
    void onTreesLODToggled(bool checked);
    void onTerrainLODToggled(bool checked);
    void onBrowseOutputDir();
    void onSelectAllWorldspaces();
    void onSelectNoneWorldspaces();
    void onGenerateClicked();

private:
    void setupUI();
    QWidget* createWorldspacePanel();
    QWidget* createOptionsPanel();
    void updateObjectsOptionsEnabled();
    void updateTreesOptionsEnabled();

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
    QComboBox* m_cmbCompDiffuse;
    QComboBox* m_cmbCompNormal;

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
