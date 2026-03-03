#include "OptionsDialog.h"
#include <QColorDialog>
#include <QDialogButtonBox>
#include <QFontDialog>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QVBoxLayout>

OptionsDialog::OptionsDialog(QWidget* parent)
    : QDialog(parent)
    , m_selectedFont(font())
{
    setWindowTitle("Options");
    resize(500, 450);

    auto* mainLayout = new QVBoxLayout(this);

    m_tabWidget = new QTabWidget(this);
    m_tabWidget->addTab(createGeneralTab(),    "General");
    m_tabWidget->addTab(createUISettingsTab(), "UI Settings");
    m_tabWidget->addTab(createUIThemeTab(),    "UI Theme");
    m_tabWidget->addTab(createCleaningTab(),   "Cleaning");
    mainLayout->addWidget(m_tabWidget);

    auto* buttonBox = new QDialogButtonBox(
        QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    connect(buttonBox, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttonBox, &QDialogButtonBox::rejected, this, &QDialog::reject);
    mainLayout->addWidget(buttonBox);
}

// ---------------------------------------------------------------------------
// General tab
// ---------------------------------------------------------------------------
QWidget* OptionsDialog::createGeneralTab()
{
    auto* page = new QWidget;
    auto* layout = new QVBoxLayout(page);

    m_chkHideUnused         = new QCheckBox("Hide Unused", page);
    m_chkHideIgnored        = new QCheckBox("Hide Ignored", page);
    m_chkLoadBSAs           = new QCheckBox("Load BSAs", page);
    m_chkSimpleRecords      = new QCheckBox("Simple Records", page);
    m_chkShowGroupRecordCount = new QCheckBox("Show Group Record Count", page);

    layout->addWidget(m_chkHideUnused);
    layout->addWidget(m_chkHideIgnored);
    layout->addWidget(m_chkLoadBSAs);
    layout->addWidget(m_chkSimpleRecords);
    layout->addWidget(m_chkShowGroupRecordCount);
    layout->addStretch();

    return page;
}

// ---------------------------------------------------------------------------
// UI Settings tab
// ---------------------------------------------------------------------------
static QPushButton* makeColorButton(const QColor& initial, QWidget* parent)
{
    auto* btn = new QPushButton(parent);
    btn->setFixedSize(60, 24);
    btn->setStyleSheet(
        QStringLiteral("background-color: %1; border: 1px solid #888;")
            .arg(initial.name()));
    return btn;
}

QWidget* OptionsDialog::createUISettingsTab()
{
    auto* page = new QWidget;
    auto* layout = new QVBoxLayout(page);

    // -- Conflict All Colors --
    auto* grpAll = new QGroupBox("Conflict All Colors", page);
    auto* allLayout = new QVBoxLayout(grpAll);

    const QStringList conflictAllLabels = {
        "Not Defined",
        "Benign Conflict",
        "Override",
        "Conflict",
        "Critical Conflict"
    };
    const QList<QColor> conflictAllDefaults = {
        QColor(Qt::white),
        QColor(Qt::green),
        QColor(Qt::yellow),
        QColor(255, 165, 0),  // orange
        QColor(Qt::red)
    };

    for (int i = 0; i < conflictAllLabels.size(); ++i) {
        auto* row = new QHBoxLayout;
        row->addWidget(new QLabel(conflictAllLabels[i], grpAll));
        row->addStretch();
        auto* btn = makeColorButton(conflictAllDefaults[i], grpAll);
        m_conflictAllButtons[conflictAllLabels[i]] = btn;
        m_conflictAllColors[conflictAllLabels[i]]  = conflictAllDefaults[i];
        connect(btn, &QPushButton::clicked, this, &OptionsDialog::onConflictAllColorClicked);
        row->addWidget(btn);
        allLayout->addLayout(row);
    }
    layout->addWidget(grpAll);

    // -- Conflict This Colors --
    auto* grpThis = new QGroupBox("Conflict This Colors", page);
    auto* thisLayout = new QVBoxLayout(grpThis);

    const QStringList conflictThisLabels = {
        "Not Defined",
        "Benign Conflict",
        "Override",
        "Conflict",
        "Critical Conflict"
    };
    const QList<QColor> conflictThisDefaults = {
        QColor(Qt::white),
        QColor(Qt::green),
        QColor(Qt::yellow),
        QColor(255, 165, 0),
        QColor(Qt::red)
    };

    for (int i = 0; i < conflictThisLabels.size(); ++i) {
        auto* row = new QHBoxLayout;
        row->addWidget(new QLabel(conflictThisLabels[i], grpThis));
        row->addStretch();
        auto* btn = makeColorButton(conflictThisDefaults[i], grpThis);
        m_conflictThisButtons[conflictThisLabels[i]] = btn;
        m_conflictThisColors[conflictThisLabels[i]]  = conflictThisDefaults[i];
        connect(btn, &QPushButton::clicked, this, &OptionsDialog::onConflictThisColorClicked);
        row->addWidget(btn);
        thisLayout->addLayout(row);
    }
    layout->addWidget(grpThis);

    // -- Font --
    auto* fontRow = new QHBoxLayout;
    fontRow->addWidget(new QLabel("Font:", page));
    m_btnSelectFont = new QPushButton("Select Font...", page);
    connect(m_btnSelectFont, &QPushButton::clicked, this, &OptionsDialog::onSelectFont);
    fontRow->addWidget(m_btnSelectFont);
    fontRow->addStretch();
    layout->addLayout(fontRow);

    layout->addStretch();
    return page;
}

// ---------------------------------------------------------------------------
// UI Theme tab
// ---------------------------------------------------------------------------
QWidget* OptionsDialog::createUIThemeTab()
{
    auto* page = new QWidget;
    auto* layout = new QVBoxLayout(page);

    m_radioSystem = new QRadioButton("System", page);
    m_radioLight  = new QRadioButton("Light", page);
    m_radioDark   = new QRadioButton("Dark", page);

    m_radioSystem->setChecked(true);

    layout->addWidget(m_radioSystem);
    layout->addWidget(m_radioLight);
    layout->addWidget(m_radioDark);
    layout->addStretch();

    return page;
}

// ---------------------------------------------------------------------------
// Cleaning tab
// ---------------------------------------------------------------------------
QWidget* OptionsDialog::createCleaningTab()
{
    auto* page = new QWidget;
    auto* layout = new QVBoxLayout(page);

    m_chkUdrMovedRef      = new QCheckBox("Detect Moved References (UDR)", page);
    m_chkUdrDeletedRef    = new QCheckBox("Detect Deleted References (UDR)", page);
    m_chkUdrDeletedNavmesh = new QCheckBox("Detect Deleted Navmeshes (UDR)", page);

    m_chkUdrMovedRef->setChecked(true);
    m_chkUdrDeletedRef->setChecked(true);
    m_chkUdrDeletedNavmesh->setChecked(true);

    layout->addWidget(m_chkUdrMovedRef);
    layout->addWidget(m_chkUdrDeletedRef);
    layout->addWidget(m_chkUdrDeletedNavmesh);
    layout->addStretch();

    return page;
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------
void OptionsDialog::updateColorButton(QPushButton* btn, const QColor& color)
{
    btn->setStyleSheet(
        QStringLiteral("background-color: %1; border: 1px solid #888;")
            .arg(color.name()));
}

void OptionsDialog::onConflictAllColorClicked()
{
    auto* btn = qobject_cast<QPushButton*>(sender());
    if (!btn)
        return;

    for (auto it = m_conflictAllButtons.constBegin();
         it != m_conflictAllButtons.constEnd(); ++it) {
        if (it.value() == btn) {
            QColor chosen = QColorDialog::getColor(
                m_conflictAllColors[it.key()], this,
                QStringLiteral("Select color for %1").arg(it.key()));
            if (chosen.isValid()) {
                m_conflictAllColors[it.key()] = chosen;
                updateColorButton(btn, chosen);
            }
            return;
        }
    }
}

void OptionsDialog::onConflictThisColorClicked()
{
    auto* btn = qobject_cast<QPushButton*>(sender());
    if (!btn)
        return;

    for (auto it = m_conflictThisButtons.constBegin();
         it != m_conflictThisButtons.constEnd(); ++it) {
        if (it.value() == btn) {
            QColor chosen = QColorDialog::getColor(
                m_conflictThisColors[it.key()], this,
                QStringLiteral("Select color for %1").arg(it.key()));
            if (chosen.isValid()) {
                m_conflictThisColors[it.key()] = chosen;
                updateColorButton(btn, chosen);
            }
            return;
        }
    }
}

void OptionsDialog::onSelectFont()
{
    bool ok = false;
    QFont chosen = QFontDialog::getFont(&ok, m_selectedFont, this, "Select Font");
    if (ok) {
        m_selectedFont = chosen;
        m_btnSelectFont->setText(
            QStringLiteral("%1, %2pt").arg(chosen.family()).arg(chosen.pointSize()));
    }
}

// ---------------------------------------------------------------------------
// Getters
// ---------------------------------------------------------------------------
bool OptionsDialog::hideUnused() const            { return m_chkHideUnused->isChecked(); }
bool OptionsDialog::hideIgnored() const           { return m_chkHideIgnored->isChecked(); }
bool OptionsDialog::loadBSAs() const              { return m_chkLoadBSAs->isChecked(); }
bool OptionsDialog::simpleRecords() const         { return m_chkSimpleRecords->isChecked(); }
bool OptionsDialog::showGroupRecordCount() const  { return m_chkShowGroupRecordCount->isChecked(); }

OptionsDialog::Theme OptionsDialog::selectedTheme() const
{
    if (m_radioLight->isChecked())  return Light;
    if (m_radioDark->isChecked())   return Dark;
    return System;
}

bool OptionsDialog::udrDetectMovedRef() const     { return m_chkUdrMovedRef->isChecked(); }
bool OptionsDialog::udrDetectDeletedRef() const   { return m_chkUdrDeletedRef->isChecked(); }
bool OptionsDialog::udrDetectDeletedNavmesh() const { return m_chkUdrDeletedNavmesh->isChecked(); }

QColor OptionsDialog::conflictAllColor(const QString& key) const
{
    return m_conflictAllColors.value(key, Qt::white);
}

QColor OptionsDialog::conflictThisColor(const QString& key) const
{
    return m_conflictThisColors.value(key, Qt::white);
}

QFont OptionsDialog::selectedFont() const { return m_selectedFont; }

// ---------------------------------------------------------------------------
// Setters
// ---------------------------------------------------------------------------
void OptionsDialog::setHideUnused(bool v)           { m_chkHideUnused->setChecked(v); }
void OptionsDialog::setHideIgnored(bool v)          { m_chkHideIgnored->setChecked(v); }
void OptionsDialog::setLoadBSAs(bool v)             { m_chkLoadBSAs->setChecked(v); }
void OptionsDialog::setSimpleRecords(bool v)        { m_chkSimpleRecords->setChecked(v); }
void OptionsDialog::setShowGroupRecordCount(bool v) { m_chkShowGroupRecordCount->setChecked(v); }

void OptionsDialog::setTheme(Theme t)
{
    switch (t) {
    case Light:  m_radioLight->setChecked(true);  break;
    case Dark:   m_radioDark->setChecked(true);   break;
    default:     m_radioSystem->setChecked(true);  break;
    }
}

void OptionsDialog::setUdrDetectMovedRef(bool v)      { m_chkUdrMovedRef->setChecked(v); }
void OptionsDialog::setUdrDetectDeletedRef(bool v)    { m_chkUdrDeletedRef->setChecked(v); }
void OptionsDialog::setUdrDetectDeletedNavmesh(bool v) { m_chkUdrDeletedNavmesh->setChecked(v); }

void OptionsDialog::setConflictAllColor(const QString& key, const QColor& color)
{
    m_conflictAllColors[key] = color;
    if (m_conflictAllButtons.contains(key))
        updateColorButton(m_conflictAllButtons[key], color);
}

void OptionsDialog::setConflictThisColor(const QString& key, const QColor& color)
{
    m_conflictThisColors[key] = color;
    if (m_conflictThisButtons.contains(key))
        updateColorButton(m_conflictThisButtons[key], color);
}

void OptionsDialog::setSelectedFont(const QFont& font)
{
    m_selectedFont = font;
    if (m_btnSelectFont)
        m_btnSelectFont->setText(
            QStringLiteral("%1, %2pt").arg(font.family()).arg(font.pointSize()));
}
