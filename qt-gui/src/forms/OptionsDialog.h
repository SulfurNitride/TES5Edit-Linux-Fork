#pragma once
#include <QDialog>
#include <QCheckBox>
#include <QColor>
#include <QFont>
#include <QMap>
#include <QPushButton>
#include <QRadioButton>
#include <QTabWidget>

class OptionsDialog : public QDialog {
    Q_OBJECT
public:
    explicit OptionsDialog(QWidget* parent = nullptr);

    // General tab getters
    bool hideUnused() const;
    bool hideIgnored() const;
    bool loadBSAs() const;
    bool simpleRecords() const;
    bool showGroupRecordCount() const;

    // UI Theme tab getters
    enum Theme { System, Light, Dark };
    Theme selectedTheme() const;

    // Cleaning tab getters
    bool udrDetectMovedRef() const;
    bool udrDetectDeletedRef() const;
    bool udrDetectDeletedNavmesh() const;

    // UI Settings tab getters
    QColor conflictAllColor(const QString& key) const;
    QColor conflictThisColor(const QString& key) const;
    QFont selectedFont() const;

    // Setters for loading saved settings
    void setHideUnused(bool v);
    void setHideIgnored(bool v);
    void setLoadBSAs(bool v);
    void setSimpleRecords(bool v);
    void setShowGroupRecordCount(bool v);
    void setTheme(Theme t);
    void setUdrDetectMovedRef(bool v);
    void setUdrDetectDeletedRef(bool v);
    void setUdrDetectDeletedNavmesh(bool v);
    void setConflictAllColor(const QString& key, const QColor& color);
    void setConflictThisColor(const QString& key, const QColor& color);
    void setSelectedFont(const QFont& font);

private slots:
    void onConflictAllColorClicked();
    void onConflictThisColorClicked();
    void onSelectFont();

private:
    QWidget* createGeneralTab();
    QWidget* createUISettingsTab();
    QWidget* createUIThemeTab();
    QWidget* createCleaningTab();
    void updateColorButton(QPushButton* btn, const QColor& color);

    QTabWidget* m_tabWidget;

    // General tab
    QCheckBox* m_chkHideUnused;
    QCheckBox* m_chkHideIgnored;
    QCheckBox* m_chkLoadBSAs;
    QCheckBox* m_chkSimpleRecords;
    QCheckBox* m_chkShowGroupRecordCount;

    // UI Theme tab
    QRadioButton* m_radioSystem;
    QRadioButton* m_radioLight;
    QRadioButton* m_radioDark;

    // Cleaning tab
    QCheckBox* m_chkUdrMovedRef;
    QCheckBox* m_chkUdrDeletedRef;
    QCheckBox* m_chkUdrDeletedNavmesh;

    // UI Settings tab
    QMap<QString, QPushButton*> m_conflictAllButtons;
    QMap<QString, QColor>       m_conflictAllColors;
    QMap<QString, QPushButton*> m_conflictThisButtons;
    QMap<QString, QColor>       m_conflictThisColors;
    QPushButton* m_btnSelectFont;
    QFont m_selectedFont;
};
