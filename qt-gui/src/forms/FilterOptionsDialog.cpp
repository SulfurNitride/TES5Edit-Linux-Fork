#include "FilterOptionsDialog.h"
#include <QDialogButtonBox>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QMessageBox>
#include <QScrollArea>
#include <QVBoxLayout>

FilterOptionsDialog::FilterOptionsDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Filter Options");
    resize(700, 600);

    auto* mainLayout = new QVBoxLayout(this);

    // Scroll area for all filter sections
    auto* scrollArea = new QScrollArea(this);
    scrollArea->setWidgetResizable(true);
    auto* scrollWidget = new QWidget;
    auto* scrollLayout = new QVBoxLayout(scrollWidget);

    // Build sections in order
    buildPresetSection(scrollWidget, scrollLayout);
    buildConflictSection(scrollWidget, scrollLayout);
    buildSignatureSection(scrollWidget, scrollLayout);
    buildTextFilterSection(scrollWidget, scrollLayout);
    buildPropertySection(scrollWidget, scrollLayout);

    scrollLayout->addStretch();
    scrollArea->setWidget(scrollWidget);
    mainLayout->addWidget(scrollArea);

    // Bottom buttons
    auto* buttonBox = new QDialogButtonBox(this);
    auto* btnApply  = buttonBox->addButton("Apply Filter", QDialogButtonBox::AcceptRole);
    auto* btnCancel = buttonBox->addButton("Cancel",       QDialogButtonBox::RejectRole);
    Q_UNUSED(btnApply)
    Q_UNUSED(btnCancel)
    connect(buttonBox, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttonBox, &QDialogButtonBox::rejected, this, &QDialog::reject);
    mainLayout->addWidget(buttonBox);
}

// ---------------------------------------------------------------------------
// Preset section
// ---------------------------------------------------------------------------
void FilterOptionsDialog::buildPresetSection(QWidget* parent, QLayout* layout)
{
    auto* grp = new QGroupBox("Preset", parent);
    auto* row = new QHBoxLayout(grp);

    m_presetCombo = new QComboBox(grp);
    m_presetCombo->setEditable(true);
    m_presetCombo->setMinimumWidth(200);
    row->addWidget(m_presetCombo);

    m_btnSavePreset = new QPushButton("Save", grp);
    m_btnLoadPreset = new QPushButton("Load", grp);
    m_btnDeletePreset = new QPushButton("Delete", grp);

    connect(m_btnSavePreset,   &QPushButton::clicked, this, &FilterOptionsDialog::onSavePreset);
    connect(m_btnLoadPreset,   &QPushButton::clicked, this, &FilterOptionsDialog::onLoadPreset);
    connect(m_btnDeletePreset, &QPushButton::clicked, this, &FilterOptionsDialog::onDeletePreset);

    row->addWidget(m_btnSavePreset);
    row->addWidget(m_btnLoadPreset);
    row->addWidget(m_btnDeletePreset);
    row->addStretch();

    layout->addWidget(grp);
}

// ---------------------------------------------------------------------------
// Conflict status section
// ---------------------------------------------------------------------------
void FilterOptionsDialog::buildConflictSection(QWidget* parent, QLayout* layout)
{
    auto* grp = new QGroupBox("Conflict Status", parent);
    auto* hbox = new QHBoxLayout(grp);

    // ConflictAll list
    auto* allBox = new QGroupBox("Conflict All", grp);
    auto* allLayout = new QVBoxLayout(allBox);
    m_conflictAllList = new QListWidget(allBox);

    const QStringList conflictAllItems = {
        "Not Defined",
        "Benign Conflict",
        "Override without Conflict",
        "Conflict",
        "Critical Conflict"
    };
    for (const auto& label : conflictAllItems) {
        auto* item = new QListWidgetItem(label, m_conflictAllList);
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(Qt::Checked);
    }
    allLayout->addWidget(m_conflictAllList);
    hbox->addWidget(allBox);

    // ConflictThis list
    auto* thisBox = new QGroupBox("Conflict This", grp);
    auto* thisLayout = new QVBoxLayout(thisBox);
    m_conflictThisList = new QListWidget(thisBox);

    const QStringList conflictThisItems = {
        "Not Defined",
        "Benign Conflict",
        "Single Override",
        "Multiple Override",
        "Conflict",
        "Critical Conflict"
    };
    for (const auto& label : conflictThisItems) {
        auto* item = new QListWidgetItem(label, m_conflictThisList);
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(Qt::Checked);
    }
    thisLayout->addWidget(m_conflictThisList);
    hbox->addWidget(thisBox);

    layout->addWidget(grp);
}

// ---------------------------------------------------------------------------
// Record Signatures section
// ---------------------------------------------------------------------------
void FilterOptionsDialog::buildSignatureSection(QWidget* parent, QLayout* layout)
{
    auto* grp = new QGroupBox("Record Signatures", parent);
    auto* vbox = new QVBoxLayout(grp);

    // Select All / None buttons
    auto* btnRow = new QHBoxLayout;
    auto* btnAll  = new QPushButton("Select All", grp);
    auto* btnNone = new QPushButton("Select None", grp);
    connect(btnAll,  &QPushButton::clicked, this, &FilterOptionsDialog::onSelectAllSignatures);
    connect(btnNone, &QPushButton::clicked, this, &FilterOptionsDialog::onSelectNoneSignatures);
    btnRow->addWidget(btnAll);
    btnRow->addWidget(btnNone);
    btnRow->addStretch();
    vbox->addLayout(btnRow);

    m_signatureList = new QListWidget(grp);
    m_signatureList->setMaximumHeight(200);

    const QStringList signatures = {
        "AACT", "ACHR", "ACTI", "ADDN", "ALCH", "AMMO", "ANIO", "APPA",
        "ARMA", "ARMO", "ARTO", "ASPC", "ASTP", "AVIF", "BOOK", "BPTD",
        "CAMS", "CELL", "CLAS", "CLFM", "CLMT", "COBJ", "COLL", "CONT",
        "CPTH", "CSTY", "DEBR", "DIAL", "DLBR", "DLVW", "DOBJ", "DOOR",
        "DUAL", "ECZN", "EFSH", "ENCH", "EQUP", "EXPL", "EYES", "FACT",
        "FLOR", "FLST", "FSTP", "FSTS", "FURN", "GLOB", "GMST", "GRAS",
        "HAZD", "HDPT", "IDLE", "IDLM", "IMAD", "IMGS", "INGR", "IPCT",
        "IPDS", "KEYM", "KYWD", "LAND", "LCRT", "LCTN", "LGTM", "LIGH",
        "LSCR", "LTEX", "LVLI", "LVLN", "LVSP", "MATO", "MATT", "MESG",
        "MGEF", "MISC", "MOVT", "MSTT", "MUSC", "MUST", "NAVI", "NAVM",
        "NPC_", "OTFT", "PACK", "PERK", "PGRE", "PHZD", "PROJ", "QUST",
        "RACE", "REFR", "REGN", "RELA", "REVB", "RFCT", "SCEN", "SCRL",
        "SHOU", "SLGM", "SMBN", "SMEN", "SMQN", "SNCT", "SNDR", "SOPM",
        "SOUN", "SPEL", "SPGD", "STAT", "TACT", "TREE", "TXST", "VTYP",
        "WATR", "WEAP", "WOOP", "WRLD", "WTHR"
    };

    for (const auto& sig : signatures) {
        auto* item = new QListWidgetItem(sig, m_signatureList);
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(Qt::Unchecked);
    }
    vbox->addWidget(m_signatureList);

    layout->addWidget(grp);
}

// ---------------------------------------------------------------------------
// Text Filters section
// ---------------------------------------------------------------------------
void FilterOptionsDialog::buildTextFilterSection(QWidget* parent, QLayout* layout)
{
    auto* grp = new QGroupBox("Text Filters", parent);
    auto* form = new QVBoxLayout(grp);

    auto addRow = [&](const QString& label, QLineEdit*& edit) {
        auto* row = new QHBoxLayout;
        auto* lbl = new QLabel(label, grp);
        lbl->setMinimumWidth(140);
        edit = new QLineEdit(grp);
        row->addWidget(lbl);
        row->addWidget(edit);
        form->addLayout(row);
    };

    addRow("EditorID contains:",      m_editEditorId);
    addRow("Name contains:",          m_editName);
    addRow("Base EditorID contains:", m_editBaseEditorId);
    addRow("Base Name contains:",     m_editBaseName);

    layout->addWidget(grp);
}

// ---------------------------------------------------------------------------
// Properties section
// ---------------------------------------------------------------------------
void FilterOptionsDialog::buildPropertySection(QWidget* parent, QLayout* layout)
{
    auto* grp = new QGroupBox("Properties", parent);
    auto* vbox = new QVBoxLayout(grp);

    m_chkPersistent        = new QCheckBox("Persistent", grp);
    m_chkDeleted           = new QCheckBox("Deleted", grp);
    m_chkVWD               = new QCheckBox("Visible When Distant (VWD)", grp);
    m_chkHasVWDMesh        = new QCheckBox("Has VWD Mesh", grp);
    m_chkInitiallyDisabled = new QCheckBox("Initially Disabled", grp);

    vbox->addWidget(m_chkPersistent);
    vbox->addWidget(m_chkDeleted);
    vbox->addWidget(m_chkVWD);
    vbox->addWidget(m_chkHasVWDMesh);
    vbox->addWidget(m_chkInitiallyDisabled);

    layout->addWidget(grp);
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------
void FilterOptionsDialog::onSelectAllSignatures()
{
    for (int i = 0; i < m_signatureList->count(); ++i)
        m_signatureList->item(i)->setCheckState(Qt::Checked);
}

void FilterOptionsDialog::onSelectNoneSignatures()
{
    for (int i = 0; i < m_signatureList->count(); ++i)
        m_signatureList->item(i)->setCheckState(Qt::Unchecked);
}

void FilterOptionsDialog::onSavePreset()
{
    QString name = m_presetCombo->currentText().trimmed();
    if (name.isEmpty()) {
        QMessageBox::warning(this, "Save Preset", "Please enter a preset name.");
        return;
    }
    // Add to combo if not already present
    if (m_presetCombo->findText(name) == -1)
        m_presetCombo->addItem(name);
    // TODO: persist preset settings to disk
}

void FilterOptionsDialog::onLoadPreset()
{
    QString name = m_presetCombo->currentText().trimmed();
    if (name.isEmpty()) {
        QMessageBox::warning(this, "Load Preset", "Please select a preset to load.");
        return;
    }
    // TODO: load preset settings from disk
}

void FilterOptionsDialog::onDeletePreset()
{
    int idx = m_presetCombo->currentIndex();
    if (idx >= 0) {
        m_presetCombo->removeItem(idx);
        // TODO: remove preset settings from disk
    }
}

// ---------------------------------------------------------------------------
// Getters
// ---------------------------------------------------------------------------
QStringList FilterOptionsDialog::checkedConflictAll() const
{
    QStringList result;
    for (int i = 0; i < m_conflictAllList->count(); ++i) {
        auto* item = m_conflictAllList->item(i);
        if (item->checkState() == Qt::Checked)
            result.append(item->text());
    }
    return result;
}

QStringList FilterOptionsDialog::checkedConflictThis() const
{
    QStringList result;
    for (int i = 0; i < m_conflictThisList->count(); ++i) {
        auto* item = m_conflictThisList->item(i);
        if (item->checkState() == Qt::Checked)
            result.append(item->text());
    }
    return result;
}

QStringList FilterOptionsDialog::checkedSignatures() const
{
    QStringList result;
    for (int i = 0; i < m_signatureList->count(); ++i) {
        auto* item = m_signatureList->item(i);
        if (item->checkState() == Qt::Checked)
            result.append(item->text());
    }
    return result;
}

QString FilterOptionsDialog::editorIdFilter() const     { return m_editEditorId->text().trimmed(); }
QString FilterOptionsDialog::nameFilter() const          { return m_editName->text().trimmed(); }
QString FilterOptionsDialog::baseEditorIdFilter() const  { return m_editBaseEditorId->text().trimmed(); }
QString FilterOptionsDialog::baseNameFilter() const      { return m_editBaseName->text().trimmed(); }

bool FilterOptionsDialog::filterPersistent() const       { return m_chkPersistent->isChecked(); }
bool FilterOptionsDialog::filterDeleted() const          { return m_chkDeleted->isChecked(); }
bool FilterOptionsDialog::filterVWD() const              { return m_chkVWD->isChecked(); }
bool FilterOptionsDialog::filterHasVWDMesh() const       { return m_chkHasVWDMesh->isChecked(); }
bool FilterOptionsDialog::filterInitiallyDisabled() const { return m_chkInitiallyDisabled->isChecked(); }

QString FilterOptionsDialog::currentPresetName() const   { return m_presetCombo->currentText().trimmed(); }
