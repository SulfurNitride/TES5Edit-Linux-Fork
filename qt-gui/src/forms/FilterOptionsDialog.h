#pragma once
#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QLineEdit>
#include <QListWidget>
#include <QMap>
#include <QPushButton>
#include <QStringList>

class FilterOptionsDialog : public QDialog {
    Q_OBJECT
public:
    explicit FilterOptionsDialog(QWidget* parent = nullptr);

    // Conflict status getters
    QStringList checkedConflictAll() const;
    QStringList checkedConflictThis() const;

    // Record signatures
    QStringList checkedSignatures() const;

    // Text filter getters
    QString editorIdFilter() const;
    QString nameFilter() const;
    QString baseEditorIdFilter() const;
    QString baseNameFilter() const;

    // Property getters
    bool filterPersistent() const;
    bool filterDeleted() const;
    bool filterVWD() const;
    bool filterHasVWDMesh() const;
    bool filterInitiallyDisabled() const;

    // Preset name
    QString currentPresetName() const;

private slots:
    void onSelectAllSignatures();
    void onSelectNoneSignatures();
    void onSavePreset();
    void onLoadPreset();
    void onDeletePreset();

private:
    void buildConflictSection(QWidget* parent, QLayout* layout);
    void buildSignatureSection(QWidget* parent, QLayout* layout);
    void buildTextFilterSection(QWidget* parent, QLayout* layout);
    void buildPropertySection(QWidget* parent, QLayout* layout);
    void buildPresetSection(QWidget* parent, QLayout* layout);

    // Conflict status
    QListWidget* m_conflictAllList;
    QListWidget* m_conflictThisList;

    // Record signatures
    QListWidget* m_signatureList;

    // Text filters
    QLineEdit* m_editEditorId;
    QLineEdit* m_editName;
    QLineEdit* m_editBaseEditorId;
    QLineEdit* m_editBaseName;

    // Properties
    QCheckBox* m_chkPersistent;
    QCheckBox* m_chkDeleted;
    QCheckBox* m_chkVWD;
    QCheckBox* m_chkHasVWDMesh;
    QCheckBox* m_chkInitiallyDisabled;

    // Presets
    QComboBox*   m_presetCombo;
    QPushButton* m_btnSavePreset;
    QPushButton* m_btnLoadPreset;
    QPushButton* m_btnDeletePreset;
};
