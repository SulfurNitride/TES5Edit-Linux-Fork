#include "MainWindow.h"

#include <QAction>
#include <QApplication>
#include <QComboBox>
#include <QDir>
#include <QDateTime>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QHash>
#include <QHeaderView>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QMenu>
#include <QMenuBar>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QShortcut>
#include <QSortFilterProxyModel>
#include <QSplitter>
#include <QStatusBar>
#include <QTabWidget>
#include <QTimer>
#include <QTableView>
#include <QTextBrowser>
#include <QToolBar>
#include <QTreeView>
#include <QVBoxLayout>
#include <cstdio>
#include <thread>

#include "ffi/XEditFFI.h"
#include "ffi/XEditTypes.h"
#include "models/NavTreeModel.h"
#include "models/NavTreeItem.h"
#include "models/OverrideViewModel.h"
#include "models/RecordViewModel.h"
#include "models/RefByTableModel.h"
#include "delegates/ConflictColorDelegate.h"
#include "delegates/NavColorDelegate.h"
#include "forms/ModuleSelectDialog.h"
#include "forms/OptionsDialog.h"
#include "util/SettingsManager.h"
#include "forms/FilterOptionsDialog.h"
#include "forms/LegendDialog.h"
#include "forms/ScriptDialog.h"
#include "forms/LogAnalyzerDialog.h"
#include "forms/WaitDialog.h"
#include "forms/LocalizationDialog.h"
#include "forms/LocalizePluginDialog.h"
#include "forms/LODGenDialog.h"
#include "forms/ModGroupSelectDialog.h"
#include "forms/ModGroupEditDialog.h"
#include "forms/FileSelectDialog.h"
#include "forms/EditWarningDialog.h"
#include "forms/DeveloperMessageDialog.h"
#include "forms/TipDialog.h"
#include "forms/RichEditDialog.h"
#include "forms/ElementDetailDialog.h"
#include "forms/ViewElementsDialog.h"
#include "forms/WorldspaceCellDetailDialog.h"
#include "util/CommandLineArgs.h"
#include "util/ConflictColors.h"
#include "util/ModGroupFile.h"
#include "util/SignatureNames.h"
#include "util/StringBuffer.h"
#include "util/StringTableParser.h"

namespace {
QSet<int> conflictAllValuesFromLabels(const QStringList& labels)
{
    QSet<int> values;
    for (const QString& label : labels) {
        if (label == "Not Defined")
            values.insert(static_cast<int>(ConflictAll::Unknown));
        else if (label == "Benign Conflict")
            values.insert(static_cast<int>(ConflictAll::Benign));
        else if (label == "Override without Conflict")
            values.insert(static_cast<int>(ConflictAll::Override));
        else if (label == "Conflict")
            values.insert(static_cast<int>(ConflictAll::Conflict));
        else if (label == "Critical Conflict")
            values.insert(static_cast<int>(ConflictAll::Critical));
    }
    return values;
}

QSet<int> conflictThisValuesFromLabels(const QStringList& labels)
{
    QSet<int> values;
    for (const QString& label : labels) {
        if (label == "Not Defined") {
            values.insert(static_cast<int>(ConflictThis::NotDefined));
            values.insert(static_cast<int>(ConflictThis::Unknown));
        } else if (label == "Benign Conflict") {
            values.insert(static_cast<int>(ConflictThis::ConflictBenign));
        } else if (label == "Single Override") {
            values.insert(static_cast<int>(ConflictThis::OnlyOne));
            values.insert(static_cast<int>(ConflictThis::Override));
        } else if (label == "Multiple Override") {
            values.insert(static_cast<int>(ConflictThis::Override));
        } else if (label == "Conflict") {
            values.insert(static_cast<int>(ConflictThis::IdenticalToMasterWinsConflict));
            values.insert(static_cast<int>(ConflictThis::ConflictWins));
            values.insert(static_cast<int>(ConflictThis::ConflictLoses));
        } else if (label == "Critical Conflict") {
            values.insert(static_cast<int>(ConflictThis::ConflictWins));
            values.insert(static_cast<int>(ConflictThis::ConflictLoses));
        }
    }
    return values;
}
} // namespace

// ---------------------------------------------------------------------------
// Construction / destruction
// ---------------------------------------------------------------------------

MainWindow::MainWindow(QWidget* parent)
    : QMainWindow(parent)
{
    setWindowTitle("xEdit 4.1.6");
    setMinimumSize(1200, 700);

    m_navModel = new NavTreeModel(this);
    m_overrideModel = new OverrideViewModel(this);
    m_recordViewModel = new RecordViewModel(this);
    m_refByModel = new RefByTableModel(this);

    createMenuBar();
    createToolBar();
    createCentralWidget();
    createStatusBar();
    setupShortcuts();

    // Restore persisted settings (theme + font) on startup
    SettingsManager::applyThemeFromSettings();
    SettingsManager::applyFontFromSettings();

    logMessage("xEdit 4.1.6 started.");
}

MainWindow::~MainWindow() = default;

// ---------------------------------------------------------------------------
// Menu bar
// ---------------------------------------------------------------------------

void MainWindow::createMenuBar()
{
    // --- File ---
    QMenu* fileMenu = menuBar()->addMenu(tr("&File"));
    fileMenu->addAction(tr("&Open..."),        QKeySequence::Open, this, &MainWindow::onFileOpen);
    fileMenu->addAction(tr("Open MO2 Folder..."), this, &MainWindow::onFileMO2Open);
    fileMenu->addAction(tr("Select Modules..."), this, &MainWindow::onModuleSelect);
    fileMenu->addSeparator();
    fileMenu->addAction(tr("Select Files..."), this, &MainWindow::onFileSelect);
    fileMenu->addSeparator();
    fileMenu->addAction(tr("&Save"),           QKeySequence::Save, this, &MainWindow::onFileSave);
    fileMenu->addSeparator();
    fileMenu->addAction(tr("E&xit"),           QKeySequence::Quit, this, &MainWindow::onFileExit);

    // --- Navigation ---
    QMenu* navMenu = menuBar()->addMenu(tr("&Navigation"));
    navMenu->addAction(tr("Search FormID"),    this, &MainWindow::onFormIDSearch);
    navMenu->addAction(tr("Search EditorID"),  this, &MainWindow::onEditorIDSearch);
    navMenu->addSeparator();
    navMenu->addAction(tr("Find..."), QKeySequence::Find, this, &MainWindow::onToggleFindBar);

    // --- View ---
    QMenu* viewMenu = menuBar()->addMenu(tr("&View"));
    viewMenu->addAction(tr("Expand All"),      [this]() { m_navTree->expandAll(); });
    viewMenu->addAction(tr("Collapse All"),    [this]() { m_navTree->collapseAll(); });
    viewMenu->addSeparator();
    viewMenu->addAction(tr("View Elements..."), this, &MainWindow::onViewElements);
    viewMenu->addAction(tr("Worldspace Cell Detail..."), this, &MainWindow::onWorldspaceCellDetail);
    viewMenu->addSeparator();
    viewMenu->addAction(tr("Filter Options..."), this, &MainWindow::onFilterOptions);
    viewMenu->addAction(tr("Remove Filter"), this, &MainWindow::onRemoveFilter);

    // --- Referenced By ---
    menuBar()->addMenu(tr("&Referenced By"));

    // --- Messages ---
    QMenu* msgMenu = menuBar()->addMenu(tr("&Messages"));
    msgMenu->addAction(tr("Clear Messages"),   [this]() { m_messagesEdit->clear(); });

    // --- Main ---
    QMenu* mainMenu = menuBar()->addMenu(tr("M&ain"));
    m_actQuickClean   = mainMenu->addAction(tr("Quick Clean"),   this, &MainWindow::onQuickClean);
    m_actCheckITM     = mainMenu->addAction(tr("Check ITM"),     this, &MainWindow::onCheckITM);
    m_actRemoveITM    = mainMenu->addAction(tr("Remove ITM"),    this, &MainWindow::onRemoveITM);
    m_actUndeleteRefs = mainMenu->addAction(tr("Undelete Refs"), this, &MainWindow::onUndeleteRefs);
    mainMenu->addSeparator();
    mainMenu->addAction(tr("Script Editor..."),    this, &MainWindow::onScriptEditor);
    mainMenu->addAction(tr("Log Analyzer..."),     this, &MainWindow::onLogAnalyzer);
    mainMenu->addAction(tr("LOD Generation..."),   this, &MainWindow::onLODGen);
    mainMenu->addAction(tr("Localization..."),     this, &MainWindow::onLocalization);
    mainMenu->addAction(tr("Localize Plugin..."),  this, &MainWindow::onLocalizePlugin);
    mainMenu->addAction(tr("Mod Groups..."),       this, &MainWindow::onModGroupSelect);
    mainMenu->addSeparator();
    mainMenu->addAction(tr("Options..."), this, &MainWindow::onOptions);

    // --- Help ---
    QMenu* helpMenu = menuBar()->addMenu(tr("&Help"));
    helpMenu->addAction(tr("About xEdit"), [this]() {
        QMessageBox::about(this, tr("About xEdit"),
            tr("xEdit 4.1.6\n\n"
               "An advanced graphical module viewer/editor and conflict detector.\n\n"
               "Linux port powered by a Rust core."));
    });
    helpMenu->addAction(tr("About Qt"), [this]() { QMessageBox::aboutQt(this); });
    helpMenu->addSeparator();
    helpMenu->addAction(tr("Legend..."), this, &MainWindow::onLegend);
    helpMenu->addSeparator();
    helpMenu->addAction(tr("Tip of the Day"), this, &MainWindow::onTipOfTheDay);
    helpMenu->addAction(tr("Developer Message"), this, &MainWindow::onDeveloperMessage);
}

// ---------------------------------------------------------------------------
// Tool bar
// ---------------------------------------------------------------------------

void MainWindow::createToolBar()
{
    QToolBar* toolbar = addToolBar(tr("Main"));
    toolbar->setMovable(false);
    toolbar->setIconSize(QSize(16, 16));

    m_backAction = toolbar->addAction(tr("<"), this, &MainWindow::onNavBack);
    m_forwardAction = toolbar->addAction(tr(">"), this, &MainWindow::onNavForward);
    m_backAction->setEnabled(false);
    m_forwardAction->setEnabled(false);

    toolbar->addSeparator();
    toolbar->addAction(tr("Save"), this, &MainWindow::onFileSave);
    toolbar->addSeparator();

    m_gameCombo = new QComboBox;
    m_gameCombo->addItems({
        "Skyrim SE",
        "Fallout 4",
        "Starfield",
        "Fallout 76",
        "Fallout NV",
        "Fallout 3",
        "Oblivion",
        "Morrowind"
    });
    m_gameCombo->setCurrentIndex(0);

    static const QHash<QString, QString> modeToLabel = {
        {"sse", "Skyrim SE"},
        {"tes5", "Skyrim SE"},
        {"tes5vr", "Skyrim SE"},
        {"enderal", "Skyrim SE"},
        {"enderalse", "Skyrim SE"},
        {"fo4", "Fallout 4"},
        {"fo4vr", "Fallout 4"},
        {"sf1", "Starfield"},
        {"fo76", "Fallout 76"},
        {"fnv", "Fallout NV"},
        {"fo3", "Fallout 3"},
        {"tes4", "Oblivion"},
        {"tes3", "Morrowind"}
    };

    const auto& cli = CommandLineArgs::instance();
    QString detectedMode;
    QString detectionSource;

    // Priority 1: Explicit CLI game mode flag (e.g. -sse, -fo4)
    if (!cli.gameMode.isEmpty()) {
        detectedMode = cli.gameMode.toLower();
        detectionSource = "command line";
    }

    // Priority 2: Detect from -D: data path contents
    if (detectedMode.isEmpty() && !cli.dataPath.isEmpty()) {
        detectedMode = CommandLineArgs::detectGameFromDataContents(cli.dataPath);
        if (!detectedMode.isEmpty())
            detectionSource = "data path contents";
    }

    // Priority 3: Detect from MO2 - check CWD and parent for ModOrganizer.ini
    if (detectedMode.isEmpty()) {
        const QString cwd = QDir::currentPath();

        // Check if CWD contains ModOrganizer.ini
        detectedMode = CommandLineArgs::detectGameFromMO2Ini(cwd);
        if (!detectedMode.isEmpty()) {
            detectionSource = "MO2 ini (CWD)";
        }

        // Check parent of CWD (MO2 sometimes sets CWD to a subfolder)
        if (detectedMode.isEmpty()) {
            QDir parent(cwd);
            if (parent.cdUp()) {
                detectedMode = CommandLineArgs::detectGameFromMO2Ini(parent.path());
                if (!detectedMode.isEmpty())
                    detectionSource = "MO2 ini (parent dir)";
            }
        }

        // Check if CWD looks like a Data folder (contains master ESMs)
        if (detectedMode.isEmpty()) {
            detectedMode = CommandLineArgs::detectGameFromDataContents(cwd);
            if (!detectedMode.isEmpty()) {
                detectionSource = "CWD data contents";
                // Pre-set the data path so we don't prompt later
                if (cli.dataPath.isEmpty())
                    m_dataPath = cwd;
            }
        }
    }

    // Apply detected game to the combo box
    if (!detectedMode.isEmpty()) {
        const QString label = modeToLabel.value(detectedMode);
        if (!label.isEmpty()) {
            const int idx = m_gameCombo->findText(label);
            if (idx >= 0) {
                m_gameCombo->setCurrentIndex(idx);
                m_gameAutoDetected = true;
                m_autoDetectedGameMode = detectedMode;
            }
        }
    }

    toolbar->addWidget(m_gameCombo);

    // Show detection source as a tooltip when auto-detected
    if (m_gameAutoDetected) {
        m_gameCombo->setToolTip(
            tr("Game auto-detected from %1").arg(detectionSource));
    }
}

// ---------------------------------------------------------------------------
// Central widget (splitter with nav panel + tab widget)
// ---------------------------------------------------------------------------

void MainWindow::createCentralWidget()
{
    m_splitter = new QSplitter(Qt::Horizontal, this);

    // --- Left panel: search fields + nav tree + file filter ---
    QWidget* leftPanel = new QWidget;
    QVBoxLayout* leftLayout = new QVBoxLayout(leftPanel);
    leftLayout->setContentsMargins(0, 0, 0, 0);
    leftLayout->setSpacing(2);

    m_formIdSearch = new QLineEdit;
    m_formIdSearch->setPlaceholderText(tr("FormID Search..."));
    connect(m_formIdSearch, &QLineEdit::returnPressed, this, &MainWindow::onFormIDSearch);
    leftLayout->addWidget(m_formIdSearch);

    m_editorIdSearch = new QLineEdit;
    m_editorIdSearch->setPlaceholderText(tr("Editor ID Search..."));
    connect(m_editorIdSearch, &QLineEdit::returnPressed, this, &MainWindow::onEditorIDSearch);
    leftLayout->addWidget(m_editorIdSearch);

    // --- Find bar (hidden by default, toggled with Ctrl+F) ---
    m_findBar = new QWidget;
    QHBoxLayout* findLayout = new QHBoxLayout(m_findBar);
    findLayout->setContentsMargins(0, 0, 0, 0);
    findLayout->setSpacing(2);

    m_findInput = new QLineEdit;
    m_findInput->setPlaceholderText(tr("Find..."));
    findLayout->addWidget(m_findInput, 1);

    m_findTypeCombo = new QComboBox;
    m_findTypeCombo->addItem(tr("Editor ID"));
    m_findTypeCombo->addItem(tr("Form ID"));
    m_findTypeCombo->addItem(tr("Display Name"));
    m_findTypeCombo->setFixedWidth(110);
    findLayout->addWidget(m_findTypeCombo);

    m_findNextBtn = new QPushButton(tr("Find Next"));
    findLayout->addWidget(m_findNextBtn);

    auto* findCloseBtn = new QPushButton(tr("X"));
    findCloseBtn->setFixedWidth(24);
    findCloseBtn->setToolTip(tr("Close find bar"));
    findLayout->addWidget(findCloseBtn);

    m_findBar->setVisible(false);
    leftLayout->addWidget(m_findBar);

    connect(m_findNextBtn, &QPushButton::clicked, this, &MainWindow::onFindNext);
    connect(m_findInput, &QLineEdit::returnPressed, this, &MainWindow::onFindNext);
    connect(findCloseBtn, &QPushButton::clicked, this, [this]() { m_findBar->setVisible(false); });

    m_navTree = new QTreeView;
    m_navTree->setModel(m_navModel);
    m_navTree->setHeaderHidden(true);
    m_navTree->setItemDelegate(new NavColorDelegate(m_navTree));
    m_navTree->setContextMenuPolicy(Qt::CustomContextMenu);
    m_navTree->setDragEnabled(true);
    m_navTree->setAcceptDrops(true);
    m_navTree->setDropIndicatorShown(true);
    m_navTree->setDragDropMode(QAbstractItemView::InternalMove);
    m_navTree->setDefaultDropAction(Qt::MoveAction);
    connect(m_navTree, &QTreeView::clicked, this, &MainWindow::onNavTreeClicked);
    connect(m_navTree, &QTreeView::customContextMenuRequested, this, &MainWindow::onNavTreeContextMenu);
    leftLayout->addWidget(m_navTree, 1);

    m_fileFilter = new QLineEdit;
    m_fileFilter->setPlaceholderText(tr("Filter files..."));
    connect(m_fileFilter, &QLineEdit::textChanged, this, [this](const QString& text) {
        m_pluginNameFilterText = text.trimmed();
        applyCurrentNavFilters();
    });
    leftLayout->addWidget(m_fileFilter);

    m_splitter->addWidget(leftPanel);

    // --- Right panel: tab widget (tabs at bottom) ---
    m_tabWidget = new QTabWidget;
    m_tabWidget->setTabPosition(QTabWidget::South);

    // View tab
    m_viewTree = new QTreeView;
    m_viewTree->setModel(m_recordViewModel);
    m_viewTree->setItemDelegate(new ConflictColorDelegate(m_viewTree));
    m_viewTree->setHeaderHidden(false);
    m_viewTree->setAlternatingRowColors(false);
    m_viewTree->setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    m_viewTree->header()->setMinimumSectionSize(80);
    m_tabWidget->addTab(m_viewTree, tr("View"));

    // Referenced By tab
    m_refByTable = new QTableView;
    m_refByTable->setModel(m_refByModel);
    m_refByTable->setAlternatingRowColors(true);
    connect(m_refByTable, &QTableView::doubleClicked, this, &MainWindow::onRefByDoubleClicked);
    m_tabWidget->addTab(m_refByTable, tr("Referenced By"));

    // Messages tab
    m_messagesEdit = new QPlainTextEdit;
    m_messagesEdit->setReadOnly(true);
    m_messagesEdit->setMaximumBlockCount(10000);
    m_tabWidget->addTab(m_messagesEdit, tr("Messages"));

    // Information tab
    m_infoEdit = new QPlainTextEdit;
    m_infoEdit->setReadOnly(true);
    m_tabWidget->addTab(m_infoEdit, tr("Information"));

    // What's New tab
    m_whatsNewBrowser = new QTextBrowser;
    m_whatsNewBrowser->setOpenExternalLinks(true);
    m_whatsNewBrowser->setHtml(
        "<h2>xEdit 4.1.6 &mdash; Linux Port</h2>"
        "<p>Native Linux port built with Rust + Qt6.</p>"
        "<h3>Features</h3>"
        "<ul>"
        "<li>Native plugin loading for all Bethesda games (TES3-Starfield)</li>"
        "<li>Lazy-loading navigation tree with load order indices</li>"
        "<li>Conflict detection with color-coded records (pastel HSL matching original xEdit)</li>"
        "<li>Override view with decoded subrecord values (definition-based)</li>"
        "<li>Record filtering by conflict status, signature, editor ID</li>"
        "<li>Quick Show Conflicts (-QSC/-VQSC) command-line modes</li>"
        "<li>ITM detection and cleaning tools</li>"
        "<li>MO2 virtual filesystem support</li>"
        "<li>Inline subrecord editing with write-back</li>"
        "<li>Record copy/paste/delete/duplicate</li>"
        "<li>Find across records (Ctrl+F) &mdash; Editor ID, Form ID, Display Name</li>"
        "<li>Navigation history (back/forward)</li>"
        "<li>ModGroup support with .modgroups file parsing</li>"
        "<li>Localization string table editor (.STRINGS/.DLSTRINGS/.ILSTRINGS)</li>"
        "<li>Settings persistence across sessions</li>"
        "<li>Dark/Light theme support</li>"
        "<li>Referenced By tracking</li>"
        "<li>Log analyzer with filtering</li>"
        "<li>All operations threaded for responsive UI</li>"
        "</ul>");
    m_tabWidget->addTab(m_whatsNewBrowser, tr("What's New"));

    m_splitter->addWidget(m_tabWidget);
    m_splitter->setSizes({400, 800});

    setCentralWidget(m_splitter);
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

void MainWindow::createStatusBar()
{
    m_breadcrumbLabel = new QLabel(tr("Ready"));
    statusBar()->addWidget(m_breadcrumbLabel, 1);
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

void MainWindow::setupShortcuts()
{
    // Ctrl+F: handled by Navigation > Find... menu action (QKeySequence::Find)

    // Ctrl+E: Focus EditorID search field
    auto* scFocusEditorID = new QShortcut(QKeySequence(tr("Ctrl+E")), this);
    connect(scFocusEditorID, &QShortcut::activated, this, &MainWindow::onFocusEditorIDSearch);

    // Ctrl+G: Focus file filter field
    auto* scFocusFilter = new QShortcut(QKeySequence(tr("Ctrl+G")), this);
    connect(scFocusFilter, &QShortcut::activated, this, &MainWindow::onFocusFileFilter);

    // F5: Refresh current view
    auto* scRefresh = new QShortcut(QKeySequence(tr("F5")), this);
    connect(scRefresh, &QShortcut::activated, this, &MainWindow::onRefreshView);

    // Delete: Delete selected record (with confirmation)
    auto* scDelete = new QShortcut(QKeySequence(QKeySequence::Delete), this);
    connect(scDelete, &QShortcut::activated, this, &MainWindow::onDeleteRecord);

    // Ctrl+D: Duplicate selected record (stub)
    auto* scDuplicate = new QShortcut(QKeySequence(tr("Ctrl+D")), this);
    connect(scDuplicate, &QShortcut::activated, this, &MainWindow::onDuplicateRecord);

    // Escape: Clear current selection
    auto* scEscape = new QShortcut(QKeySequence(Qt::Key_Escape), this);
    connect(scEscape, &QShortcut::activated, this, &MainWindow::onClearSelection);

    // Ctrl+1 through Ctrl+5: Switch tabs
    for (int i = 0; i < 5; ++i) {
        auto* sc = new QShortcut(QKeySequence(Qt::CTRL | static_cast<Qt::Key>(Qt::Key_1 + i)), this);
        connect(sc, &QShortcut::activated, this, [this, i]() { onSwitchTab(i); });
    }
}

void MainWindow::onFocusFormIDSearch()
{
    m_formIdSearch->setFocus();
    m_formIdSearch->selectAll();
}

void MainWindow::onFocusEditorIDSearch()
{
    m_editorIdSearch->setFocus();
    m_editorIdSearch->selectAll();
}

void MainWindow::onFocusFileFilter()
{
    m_fileFilter->setFocus();
    m_fileFilter->selectAll();
}

void MainWindow::onRefreshView()
{
    // Re-populate the current record view if a record is selected
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item)
        return;

    if (item->nodeType() == NodeType::Record) {
        populateRecordInfo(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        populateSubrecordView(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        m_refByModel->setRecord(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        logMessage("View refreshed.");
    } else {
        // Re-trigger the nav click handler to refresh the info panel
        onNavTreeClicked(index);
        logMessage("View refreshed.");
    }
}

bool MainWindow::confirmEditAction()
{
    if (m_editWarningShown)
        return true;

    if (CommandLineArgs::instance().iKnowWhatImDoing) {
        m_editWarningShown = true;
        return true;
    }

    const bool confirmed = EditWarningDialog::confirmEdit(this,
        tr("You are about to modify a plugin file.\n\n"
           "Editing plugin files can potentially break your game if done "
           "incorrectly. Make sure you know what you are doing and always "
           "keep backups of your files.\n\n"
           "Are you sure you want to continue?"));

    if (confirmed)
        m_editWarningShown = true;

    return confirmed;
}

void MainWindow::onDeleteRecord()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Record) {
        logMessage("Delete requires a record to be selected.");
        return;
    }

    if (!confirmEditAction())
        return;

    // Confirmation dialog
    int ret = QMessageBox::question(this, tr("Delete Record"),
        tr("Are you sure you want to delete this record?\n\n"
           "Plugin: %1, Group: %2, Record: %3")
            .arg(item->pluginIndex())
            .arg(item->groupIndex())
            .arg(item->recordIndex()),
        QMessageBox::Yes | QMessageBox::No, QMessageBox::No);

    if (ret != QMessageBox::Yes)
        return;

    int pi = item->pluginIndex();
    int gi = item->groupIndex();
    int ri = item->recordIndex();
    uint32_t formId = item->formId();

    auto& ffi = XEditFFI::instance();
    if (ffi.xedit_delete_record) {
        int result = ffi.xedit_delete_record(pi, gi, ri);
        if (result == 0) {
            QModelIndex parentIdx = m_navTree->currentIndex().parent();
            m_navModel->removeRow(m_navTree->currentIndex().row(), parentIdx);
            logMessage(QString("Deleted record [%1]").arg(formId, 8, 16, QChar('0')));
        } else {
            logMessage("ERROR: Failed to delete record.");
        }
    } else {
        logMessage("ERROR: xedit_delete_record not available.");
    }
}

void MainWindow::onDuplicateRecord()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Record) {
        logMessage("Duplicate requires a record to be selected.");
        return;
    }

    if (!confirmEditAction())
        return;

    int pi = item->pluginIndex();
    int gi = item->groupIndex();
    int ri = item->recordIndex();

    auto& ffi = XEditFFI::instance();
    if (ffi.xedit_copy_record) {
        int newRecIdx = ffi.xedit_copy_record(pi, gi, ri, pi); // same plugin
        if (newRecIdx >= 0) {
            // Refresh the group node to show the new record
            QModelIndex parentIdx = m_navTree->currentIndex().parent();
            NavTreeItem* groupItem = m_navModel->itemFromIndex(parentIdx);
            if (groupItem) {
                groupItem->setCachedRecordCount(groupItem->cachedRecordCount() + 1);
                // Trigger fetchMore to pick up the new record
                if (m_navModel->canFetchMore(parentIdx))
                    m_navModel->fetchMore(parentIdx);
            }
            logMessage(QString("Duplicated record to index %1").arg(newRecIdx));
        } else {
            logMessage("ERROR: Failed to duplicate record.");
        }
    } else {
        logMessage("ERROR: xedit_copy_record not available.");
    }
}

void MainWindow::onClearSelection()
{
    m_navTree->clearSelection();
    m_navTree->setCurrentIndex(QModelIndex());
    m_overrideModel->clear();
    m_recordViewModel->clear();
    m_refByModel->clear();
    m_infoEdit->clear();
}

void MainWindow::onSwitchTab(int tabIndex)
{
    if (tabIndex >= 0 && tabIndex < m_tabWidget->count())
        m_tabWidget->setCurrentIndex(tabIndex);
}

// ---------------------------------------------------------------------------
// Engine initialisation helper
// ---------------------------------------------------------------------------

void MainWindow::ensureEngineInitialized()
{
    if (m_engineInitialized)
        return;

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_init) {
        logMessage("ERROR: xedit_init not available.");
        return;
    }

    const auto& cli = CommandLineArgs::instance();
    if (!cli.valid) {
        logMessage(QString("ERROR: %1").arg(cli.error));
        QMessageBox::warning(this, tr("Invalid Command Line"), cli.error);
        return;
    }

    // Ask the user to select a data directory the first time
    if (m_dataPath.isEmpty() && !cli.dataPath.isEmpty())
        m_dataPath = cli.dataPath;

    if (m_dataPath.isEmpty()) {
        m_dataPath = QFileDialog::getExistingDirectory(
            this, tr("Select Game Data Directory"));
        if (m_dataPath.isEmpty())
            return;
    }

    // Map combo text to FFI game name
    static const QHash<QString, QString> gameMap = {
        {"Skyrim SE", "SSE"}, {"Fallout 4", "FO4"}, {"Starfield", "Starfield"},
        {"Fallout 76", "FO76"}, {"Fallout NV", "FNV"}, {"Fallout 3", "FO3"},
        {"Oblivion", "TES4"}, {"Morrowind", "TES3"}
    };

    QString gameName;

    // Try CLI game mode first
    if (!cli.gameMode.isEmpty())
        gameName = CommandLineArgs::toFFIGameName(cli.gameMode);

    // Try auto-detected game mode from createToolBar()
    if (gameName.isEmpty() && !m_autoDetectedGameMode.isEmpty())
        gameName = CommandLineArgs::toFFIGameName(m_autoDetectedGameMode);

    // Try detecting from the data path contents (covers MO2 scenarios
    // where the data path was set via onFileMO2Open or other means)
    if (gameName.isEmpty() && !m_dataPath.isEmpty()) {
        const QString detected = CommandLineArgs::detectGameFromDataContents(m_dataPath);
        if (!detected.isEmpty()) {
            gameName = CommandLineArgs::toFFIGameName(detected);
            if (!gameName.isEmpty()) {
                m_autoDetectedGameMode = detected;
                logMessage(QString("Auto-detected game '%1' from data path contents.")
                    .arg(detected));
            }
        }
    }

    if (gameName.isEmpty()) {
        m_currentGame = m_gameCombo->currentText();
        gameName = gameMap.value(m_currentGame, "SSE");
    } else {
        // Use the detected mode label if available, otherwise the raw mode
        if (!m_autoDetectedGameMode.isEmpty()) {
            static const QHash<QString, QString> modeToLabel = {
                {"sse", "Skyrim SE"}, {"tes5", "Skyrim SE"}, {"tes5vr", "Skyrim SE"},
                {"enderal", "Skyrim SE"}, {"enderalse", "Skyrim SE"},
                {"fo4", "Fallout 4"}, {"fo4vr", "Fallout 4"},
                {"sf1", "Starfield"}, {"fo76", "Fallout 76"},
                {"fnv", "Fallout NV"}, {"fo3", "Fallout 3"},
                {"tes4", "Oblivion"}, {"tes3", "Morrowind"}
            };
            const QString label = modeToLabel.value(m_autoDetectedGameMode);
            m_currentGame = label.isEmpty() ? m_autoDetectedGameMode.toUpper() : label;
            // Sync the combo box to match
            const int idx = m_gameCombo->findText(m_currentGame);
            if (idx >= 0)
                m_gameCombo->setCurrentIndex(idx);
        } else {
            m_currentGame = cli.gameMode.toUpper();
        }
    }

    QByteArray gameBa  = gameName.toUtf8();
    QByteArray pathBa  = m_dataPath.toUtf8();

    int32_t rc = ffi.xedit_init(gameBa.constData(), pathBa.constData(), nullptr);
    if (rc < 0) {
        logMessage(QString("ERROR: xedit_init returned %1").arg(rc));
        QMessageBox::warning(this, tr("Initialization Failed"),
            tr("Failed to initialize the engine for %1.\nError code: %2")
                .arg(m_currentGame).arg(rc));
        return;
    }

    m_engineInitialized = true;
    logMessage(QString("Engine initialized for %1 (data: %2)")
        .arg(m_currentGame, m_dataPath));
}

bool MainWindow::hasActiveRecordFilter() const
{
    return !m_keepConflictAll.isEmpty()
        || !m_keepConflictThis.isEmpty()
        || !m_keepSignatures.isEmpty()
        || !m_editorIdFilterText.isEmpty()
        || !m_nameFilterText.isEmpty()
        || !m_baseEditorIdFilterText.isEmpty()
        || !m_baseNameFilterText.isEmpty()
        || m_filterPersistent
        || m_filterDeleted
        || m_filterVwd
        || m_filterHasVwdMesh
        || m_filterInitiallyDisabled
        || m_excludeMasterPlugin;
}

void MainWindow::ensureAllNavRecordsLoaded()
{
    const int pluginRows = m_navModel->rowCount();
    for (int p = 0; p < pluginRows; ++p) {
        const QModelIndex pluginIdx = m_navModel->index(p, 0);
        const int groupRows = m_navModel->rowCount(pluginIdx);
        for (int g = 0; g < groupRows; ++g) {
            const QModelIndex groupIdx = m_navModel->index(g, 0, pluginIdx);
            while (m_navModel->canFetchMore(groupIdx))
                m_navModel->fetchMore(groupIdx);
        }
    }
}

bool MainWindow::recordPassesCurrentFilter(int pluginIdx, int groupIdx, int recordIdx, NavTreeItem* item) const
{
    if (m_excludeMasterPlugin && pluginIdx == 0)
        return false;

    if (!m_keepConflictAll.isEmpty() && !m_keepConflictAll.contains(item->conflictAll()))
        return false;
    if (!m_keepConflictThis.isEmpty() && !m_keepConflictThis.contains(item->conflictThis()))
        return false;

    auto& ffi = XEditFFI::instance();

    if (!m_keepSignatures.isEmpty()) {
        const QString signature = ffiString(ffi.xedit_record_signature, pluginIdx, groupIdx, recordIdx).toUpper();
        if (!m_keepSignatures.contains(signature))
            return false;
    }

    if (!m_editorIdFilterText.isEmpty()) {
        const QString editorId = ffiString(ffi.xedit_record_editor_id, pluginIdx, groupIdx, recordIdx);
        if (!editorId.contains(m_editorIdFilterText, Qt::CaseInsensitive))
            return false;
    }

    return true;
}

void MainWindow::applyCurrentNavFilters()
{
    QApplication::setOverrideCursor(Qt::WaitCursor);

    const bool hasRecordFilter = m_filterApplied && hasActiveRecordFilter();

    auto& ffi = XEditFFI::instance();
    const auto& conflictCache = m_navModel->conflictCache();

    int groupsProcessed = 0;
    const int pluginRows = m_navModel->rowCount();
    for (int p = 0; p < pluginRows; ++p) {
        const QModelIndex pluginIdx = m_navModel->index(p, 0);
        NavTreeItem* pluginItem = m_navModel->itemFromIndex(pluginIdx);
        if (!pluginItem)
            continue;

        const bool pluginNamePass = m_pluginNameFilterText.isEmpty()
            || pluginItem->displayText().contains(m_pluginNameFilterText, Qt::CaseInsensitive);

        bool pluginHasVisibleRecords = !hasRecordFilter;
        const int groupRows = m_navModel->rowCount(pluginIdx);
        for (int g = 0; g < groupRows; ++g) {
            const QModelIndex groupIdx = m_navModel->index(g, 0, pluginIdx);
            bool groupVisible = !hasRecordFilter;

            if (hasRecordFilter) {
                NavTreeItem* groupItem = m_navModel->itemFromIndex(groupIdx);
                if (!groupItem) {
                    m_navTree->setRowHidden(g, pluginIdx, true);
                    continue;
                }

                // If records are already loaded, filter them directly
                if (groupItem->childCount() > 0) {
                    groupVisible = false;
                    const int recordRows = m_navModel->rowCount(groupIdx);
                    for (int r = 0; r < recordRows; ++r) {
                        const QModelIndex recordIdx = m_navModel->index(r, 0, groupIdx);
                        NavTreeItem* recordItem = m_navModel->itemFromIndex(recordIdx);
                        if (!recordItem) continue;
                        const bool visible = recordPassesCurrentFilter(
                            recordItem->pluginIndex(), recordItem->groupIndex(),
                            recordItem->recordIndex(), recordItem);
                        m_navTree->setRowHidden(r, groupIdx, !visible);
                        groupVisible = groupVisible || visible;
                    }
                } else if (ffi.xedit_group_form_ids) {
                    // Records NOT loaded yet -- use batch FormID check
                    int pi = groupItem->pluginIndex();
                    int gi = groupItem->groupIndex();
                    int recCount = groupItem->cachedRecordCount();
                    if (recCount <= 0) {
                        groupVisible = false;
                    } else {
                        // Get all FormIDs for this group in one FFI call
                        QVector<uint32_t> formIds(recCount);
                        int32_t got = ffi.xedit_group_form_ids(pi, gi, formIds.data(), recCount);
                        groupVisible = false;
                        for (int32_t i = 0; i < got && !groupVisible; ++i) {
                            // Check if this FormID passes the conflict filter
                            auto it = conflictCache.constFind(formIds[i]);
                            if (it != conflictCache.cend()) {
                                bool passes = true;
                                if (!m_keepConflictAll.isEmpty() && !m_keepConflictAll.contains(it.value().first))
                                    passes = false;
                                if (!m_keepConflictThis.isEmpty() && !m_keepConflictThis.contains(it.value().second))
                                    passes = false;
                                if (passes)
                                    groupVisible = true;
                            }
                        }
                    }
                } else {
                    // Fallback: show group (can't determine)
                    groupVisible = true;
                }
            }

            m_navTree->setRowHidden(g, pluginIdx, !groupVisible);
            pluginHasVisibleRecords = pluginHasVisibleRecords || groupVisible;

            if (++groupsProcessed % 50 == 0)
                QApplication::processEvents();
        }

        const bool pluginVisible = pluginNamePass && pluginHasVisibleRecords;
        m_navTree->setRowHidden(p, QModelIndex(), !pluginVisible);
    }

    QApplication::restoreOverrideCursor();
}

void MainWindow::rebuildNavTreeFromLoadedPlugins()
{
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_plugin_count)
        return;

    const int32_t pluginCount = ffi.xedit_plugin_count();
    m_navModel->clear();
    for (int32_t i = 0; i < pluginCount; ++i)
        m_navModel->addPlugin(i);
}

void MainWindow::removeFilter()
{
    m_filterApplied = false;
    m_keepConflictAll.clear();
    m_keepConflictThis.clear();
    m_keepSignatures.clear();
    m_editorIdFilterText.clear();
    m_nameFilterText.clear();
    m_baseEditorIdFilterText.clear();
    m_baseNameFilterText.clear();
    m_filterPersistent = false;
    m_filterDeleted = false;
    m_filterVwd = false;
    m_filterHasVwdMesh = false;
    m_filterInitiallyDisabled = false;
    m_excludeMasterPlugin = false;

    rebuildNavTreeFromLoadedPlugins();
    applyCurrentNavFilters();
}

void MainWindow::runConflictDetectionAsync(std::function<void()> onComplete)
{
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_detect_conflicts || !ffi.xedit_conflict_form_id
        || !ffi.xedit_conflict_severity || !ffi.xedit_conflict_plugin_count) {
        if (onComplete) onComplete();
        return;
    }

    logMessage("Detecting conflicts across all plugins...");
    statusBar()->showMessage(tr("Detecting conflicts..."));

    // Show indeterminate progress dialog during conflict detection
    auto* waitDlg = new WaitDialog(this);
    waitDlg->setMessage("Detecting conflicts across all plugins...");
    waitDlg->setProgress(-1.0); // indeterminate
    waitDlg->show();

    // Result struct: background thread fills both status and conflict map
    struct ConflictResult {
        std::atomic<int> status{-2}; // -2 = running, -1 = error, >= 0 = count
        QHash<uint32_t, QPair<int, int>> conflictMap; // filled by background thread
    };
    auto* result = new ConflictResult;

    // Run conflict detection AND hash map building in background thread
    std::thread([&ffi, result]() {
        int count = ffi.xedit_detect_conflicts(nullptr);
        if (count > 0) {
            // Build conflict map in background thread (avoids GUI freeze)
            auto mapSeverity = [](int severity) -> int {
                switch (severity) {
                case 0: return static_cast<int>(ConflictAll::Benign);
                case 1: return static_cast<int>(ConflictAll::Override);
                case 2: return static_cast<int>(ConflictAll::Critical);
                case 3: return static_cast<int>(ConflictAll::Conflict);
                default: return static_cast<int>(ConflictAll::Unknown);
                }
            };
            result->conflictMap.reserve(count);
            for (int i = 0; i < count; ++i) {
                uint32_t formId = ffi.xedit_conflict_form_id(nullptr, i);
                int severity = ffi.xedit_conflict_severity(nullptr, i);
                int conflictAll = mapSeverity(severity);
                int conflictThis = (severity == 1)
                    ? static_cast<int>(ConflictThis::ConflictWins)
                    : static_cast<int>(ConflictThis::ConflictLoses);
                auto it = result->conflictMap.find(formId);
                if (it == result->conflictMap.end()) {
                    result->conflictMap.insert(formId, qMakePair(conflictAll, conflictThis));
                } else {
                    if (conflictAll > it.value().first)
                        it.value().first = conflictAll;
                    if (conflictThis == static_cast<int>(ConflictThis::ConflictWins))
                        it.value().second = conflictThis;
                }
            }
        }
        result->status.store(count);
    }).detach();

    // Poll with QTimer until done
    auto* pollTimer = new QTimer(this);
    auto callback = onComplete; // capture by value
    connect(pollTimer, &QTimer::timeout, this, [this, pollTimer, result, callback, waitDlg]() {
        int count = result->status.load();
        if (count == -2) return; // still running

        pollTimer->stop();
        pollTimer->deleteLater();

        // Dismiss the progress dialog
        waitDlg->close();
        waitDlg->deleteLater();

        if (count < 0) {
            logMessage("Conflict detection failed.");
            statusBar()->showMessage(tr("Conflict detection failed."), 5000);
            delete result;
            if (callback) callback();
            return;
        }

        logMessage(QString("Conflict detection complete: %1 conflicts found.").arg(count));

        // Apply the pre-built conflict map (only the fast tree walk runs on GUI thread)
        m_navModel->setConflictCache(std::move(result->conflictMap));
        statusBar()->showMessage(tr("Conflicts detected: %1").arg(count), 5000);

        delete result;
        if (callback) callback();
    });
    pollTimer->start(50);
}

void MainWindow::applyQuickShowConflictsFilterIfRequested()
{
    const auto& cli = CommandLineArgs::instance();
    if (!cli.quickShowConflicts)
        return;

    runConflictDetectionAsync([this]() {
        const auto& cli = CommandLineArgs::instance();
        m_filterApplied = true;
        m_keepConflictAll.clear();
        m_keepConflictThis = QSet<int>{
            static_cast<int>(ConflictThis::IdenticalToMasterWinsConflict),
            static_cast<int>(ConflictThis::ConflictWins),
            static_cast<int>(ConflictThis::ConflictLoses)
        };
        m_keepSignatures.clear();
        m_editorIdFilterText.clear();
        m_nameFilterText.clear();
        m_baseEditorIdFilterText.clear();
        m_baseNameFilterText.clear();
        m_filterPersistent = false;
        m_filterDeleted = false;
        m_filterVwd = false;
        m_filterHasVwdMesh = false;
        m_filterInitiallyDisabled = false;
        m_excludeMasterPlugin = cli.veryQuickShowConflicts;

        applyCurrentNavFilters();
        logMessage(cli.veryQuickShowConflicts
            ? "Very Quick Show Conflicts filter applied."
            : "Quick Show Conflicts filter applied.");
    });
}

// ---------------------------------------------------------------------------
// File menu slots
// ---------------------------------------------------------------------------

void MainWindow::onFileOpen()
{
    ensureEngineInitialized();
    if (!m_engineInitialized)
        return;

    QStringList files = QFileDialog::getOpenFileNames(
        this, tr("Open Plugin Files"), m_dataPath,
        tr("Plugin files (*.esp *.esm *.esl);;All files (*)"));

    if (files.isEmpty())
        return;

    loadPluginsAndBuildIndex(files);
}

// ---------------------------------------------------------------------------
// enumerateDataDirPlugins — scan m_dataPath for .esp/.esm/.esl files
// ---------------------------------------------------------------------------
QStringList MainWindow::enumerateDataDirPlugins() const
{
    QStringList plugins;
    if (m_dataPath.isEmpty())
        return plugins;

    QDir dataDir(m_dataPath);
    if (!dataDir.exists())
        return plugins;

    const QStringList filters = {"*.esp", "*.esm", "*.esl"};
    QStringList entries = dataDir.entryList(filters, QDir::Files | QDir::Readable, QDir::Name | QDir::IgnoreCase);
    return entries;
}

// ---------------------------------------------------------------------------
// loadPluginsAndBuildIndex — load a list of plugin files and build nav tree
// ---------------------------------------------------------------------------
void MainWindow::loadPluginsAndBuildIndex(const QStringList& filePaths)
{
    auto& ffi = XEditFFI::instance();

    m_pendingPlugins.clear();
    for (const QString& filePath : filePaths) {
        QByteArray pathBa = filePath.toUtf8();
        int32_t idx = ffi.xedit_load_plugin(pathBa.constData());
        if (idx < 0) {
            logMessage(QString("ERROR: Failed to load '%1' (code %2)")
                .arg(filePath).arg(idx));
            continue;
        }

        m_pendingPlugins.append(idx);
        logMessage(QString("Loaded plugin '%1' (index %2)")
            .arg(filePath).arg(idx));
    }

    // Build the referenced-by index after loading plugins
    if (!m_pendingPlugins.isEmpty()) {
        if (ffi.xedit_build_refby_index_async && ffi.xedit_refby_build_status) {
            logMessage("Building referenced-by index (async)...");
            ffi.xedit_build_refby_index_async();
            statusBar()->showMessage(tr("Building index..."));

            auto* buildTimer = new QTimer(this);
            connect(buildTimer, &QTimer::timeout, this, [this, buildTimer]() {
                auto& ffi = XEditFFI::instance();
                if (ffi.xedit_refby_build_status && ffi.xedit_refby_build_status() == 3) {
                    buildTimer->stop();
                    buildTimer->deleteLater();
                    for (int idx : m_pendingPlugins) {
                        m_navModel->addPlugin(idx);
                    }
                    logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
                    m_pendingPlugins.clear();
                    applyQuickShowConflictsFilterIfRequested();
                    applyCurrentNavFilters();
                    statusBar()->showMessage(tr("Ready"));
                }
            });
            buildTimer->start(500);
        } else if (ffi.xedit_build_refby_index) {
            logMessage("Building referenced-by index...");
            ffi.xedit_build_refby_index();
            logMessage("Referenced-by index built.");
            for (int idx : m_pendingPlugins) {
                m_navModel->addPlugin(idx);
            }
            logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
            m_pendingPlugins.clear();
            applyQuickShowConflictsFilterIfRequested();
            applyCurrentNavFilters();
            statusBar()->showMessage(tr("Ready"));
        } else {
            for (int idx : m_pendingPlugins) {
                m_navModel->addPlugin(idx);
            }
            logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
            m_pendingPlugins.clear();
            applyQuickShowConflictsFilterIfRequested();
            applyCurrentNavFilters();
        }
    }
}

// ---------------------------------------------------------------------------
// onModuleSelect — show ModuleSelectDialog with plugins from the data directory
// ---------------------------------------------------------------------------
void MainWindow::onModuleSelect()
{
    ensureEngineInitialized();
    if (!m_engineInitialized)
        return;

    QStringList available = enumerateDataDirPlugins();
    if (available.isEmpty()) {
        QMessageBox::information(this, tr("No Plugins Found"),
            tr("No plugin files (.esp, .esm, .esl) were found in:\n%1").arg(m_dataPath));
        return;
    }

    ModuleSelectDialog dlg(available, this);
    if (dlg.exec() != QDialog::Accepted)
        return;

    QStringList selected = dlg.selectedPlugins();
    if (selected.isEmpty())
        return;

    logMessage(QString("Loading %1 selected module(s)...").arg(selected.size()));

    // Build full paths from data directory
    QStringList fullPaths;
    for (const QString& name : selected) {
        fullPaths.append(m_dataPath + "/" + name);
    }

    loadPluginsAndBuildIndex(fullPaths);
}

void MainWindow::onFileMO2Open()
{
    QString mo2Dir = QFileDialog::getExistingDirectory(
        this, tr("Select MO2 Directory"));
    if (mo2Dir.isEmpty())
        return;

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_load_mo2) {
        logMessage("ERROR: xedit_load_mo2 not available.");
        return;
    }

    // Step 1: Load the MO2 instance
    QByteArray pathBa = mo2Dir.toUtf8();
    int32_t rc = ffi.xedit_load_mo2(nullptr, pathBa.constData());
    if (rc < 0) {
        logMessage(QString("ERROR: xedit_load_mo2 returned %1").arg(rc));
        QMessageBox::warning(this, tr("MO2 Error"),
            tr("Failed to load MO2 from '%1'.\nError code: %2").arg(mo2Dir).arg(rc));
        return;
    }
    logMessage(QString("MO2 loaded from '%1'").arg(mo2Dir));

    // Step 2: Get available profiles and let user pick one
    if (ffi.xedit_mo2_profile_count && ffi.xedit_mo2_profile_name) {
        int32_t profileCount = ffi.xedit_mo2_profile_count(nullptr);
        if (profileCount > 0) {
            QStringList profileNames;
            for (int32_t i = 0; i < profileCount; ++i) {
                QString name = ffiString([&](char* buf, int32_t len) {
                    return ffi.xedit_mo2_profile_name(nullptr, i, buf, len);
                });
                if (!name.isEmpty())
                    profileNames.append(name);
            }

            if (!profileNames.isEmpty()) {
                bool ok = false;
                QString chosen = QInputDialog::getItem(
                    this, tr("Select MO2 Profile"),
                    tr("Choose a profile to load:"),
                    profileNames, 0, false, &ok);

                if (!ok || chosen.isEmpty()) {
                    logMessage("MO2 profile selection cancelled.");
                    return;
                }

                // Select the profile
                if (ffi.xedit_mo2_select_profile) {
                    QByteArray profileBa = chosen.toUtf8();
                    rc = ffi.xedit_mo2_select_profile(nullptr, profileBa.constData());
                    if (rc < 0) {
                        logMessage(QString("ERROR: Failed to select profile '%1' (code %2)").arg(chosen).arg(rc));
                        return;
                    }
                    logMessage(QString("Selected MO2 profile: '%1'").arg(chosen));
                }
            } else {
                logMessage("WARNING: No MO2 profiles found.");
            }
        } else {
            logMessage("WARNING: No MO2 profiles available.");
        }
    }

    // Step 3: Initialize the engine — use the MO2 directory as the data path
    // so ensureEngineInitialized() doesn't prompt for a folder again
    m_dataPath = mo2Dir;
    ensureEngineInitialized();
    if (!m_engineInitialized)
        return;

    // Step 4: Load the MO2 load order (this loads all plugins)
    if (ffi.xedit_mo2_load_order) {
        rc = ffi.xedit_mo2_load_order(nullptr);
        if (rc < 0) {
            logMessage(QString("ERROR: xedit_mo2_load_order returned %1").arg(rc));
            return;
        }
        logMessage(QString("MO2 load order applied (%1 plugins).").arg(rc));
    }

    // Step 5: Collect all loaded plugin indices for deferred nav population
    m_pendingPlugins.clear();
    if (ffi.xedit_plugin_count) {
        int32_t pluginCount = ffi.xedit_plugin_count();
        logMessage(QString("Queued %1 plugin(s) for navigation tree population.").arg(pluginCount));
        for (int32_t i = 0; i < pluginCount; ++i) {
            m_pendingPlugins.append(i);
            QString name = ffiString(ffi.xedit_plugin_filename, i);
            logMessage(QString("  Queued plugin [%1] %2").arg(i).arg(name));
        }
    }

    // Step 6: Build the referenced-by index in the background
    if (!m_pendingPlugins.isEmpty() && ffi.xedit_build_refby_index_async && ffi.xedit_refby_build_status) {
        logMessage("Building referenced-by index (async)...");
        ffi.xedit_build_refby_index_async();
        statusBar()->showMessage(tr("Building index..."));

        // Poll until Rust reports both refby build and offload complete.
        auto* buildTimer = new QTimer(this);
        connect(buildTimer, &QTimer::timeout, this, [this, buildTimer]() {
            auto& ffi = XEditFFI::instance();
            if (ffi.xedit_refby_build_status && ffi.xedit_refby_build_status() == 3) {
                buildTimer->stop();
                buildTimer->deleteLater();
                for (int idx : m_pendingPlugins) {
                    m_navModel->addPlugin(idx);
                }
                logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
                m_pendingPlugins.clear();
                applyQuickShowConflictsFilterIfRequested();
                applyCurrentNavFilters();
                statusBar()->showMessage(tr("Ready"));
            }
        });
        buildTimer->start(500); // Poll every 500ms
    } else if (!m_pendingPlugins.isEmpty() && ffi.xedit_build_refby_index) {
        logMessage("Building referenced-by index...");
        ffi.xedit_build_refby_index();
        logMessage("Referenced-by index built.");
        for (int idx : m_pendingPlugins) {
            m_navModel->addPlugin(idx);
        }
        logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
        m_pendingPlugins.clear();
        applyQuickShowConflictsFilterIfRequested();
        applyCurrentNavFilters();
        statusBar()->showMessage(tr("Ready"));
    } else if (!m_pendingPlugins.isEmpty()) {
        for (int idx : m_pendingPlugins) {
            m_navModel->addPlugin(idx);
        }
        logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
        m_pendingPlugins.clear();
        applyQuickShowConflictsFilterIfRequested();
        applyCurrentNavFilters();
    }

}

void MainWindow::onFileSave()
{
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_save_plugin || !ffi.xedit_plugin_count) {
        logMessage("ERROR: Save functions not available.");
        return;
    }

    int32_t count = ffi.xedit_plugin_count();
    if (count <= 0) {
        logMessage("No plugins loaded to save.");
        return;
    }
    QString dataPath = m_dataPath;

    runAsyncTask("Saving plugins...", [count, dataPath]() -> QString {
        auto& ffi = XEditFFI::instance();
        QString log;
        for (int32_t i = 0; i < count; ++i) {
            char nameBuf[512];
            int nameLen = ffi.xedit_plugin_filename(i, nameBuf, sizeof(nameBuf));
            QString name = (nameLen > 0) ? QString::fromUtf8(nameBuf, nameLen) : QString();
            if (name.isEmpty()) continue;
            QString fullPath = dataPath + "/" + name;
            QByteArray pathBa = fullPath.toUtf8();
            int32_t rc = ffi.xedit_save_plugin(i, pathBa.constData());
            if (rc < 0) {
                log += QString("ERROR: Failed to save '%1' (code %2)\n").arg(name).arg(rc);
            } else {
                log += QString("Saved '%1'\n").arg(name);
            }
        }
        return log;
    }, [this](const QString& output) {
        logMessage(output.trimmed());
    });
}

void MainWindow::onFileExit()
{
    close();
}

// ---------------------------------------------------------------------------
// Navigation slots
// ---------------------------------------------------------------------------

void MainWindow::onNavBack()
{
    if (m_navHistoryPos > 0) {
        m_navigatingHistory = true;
        m_navHistoryPos--;
        QModelIndex idx = m_navHistory[m_navHistoryPos];
        if (idx.isValid()) {
            m_navTree->setCurrentIndex(idx);
            m_navTree->scrollTo(idx);
            onNavTreeClicked(idx);
        }
        m_navigatingHistory = false;
        updateNavButtons();
    }
}

void MainWindow::onNavForward()
{
    if (m_navHistoryPos < m_navHistory.size() - 1) {
        m_navigatingHistory = true;
        m_navHistoryPos++;
        QModelIndex idx = m_navHistory[m_navHistoryPos];
        if (idx.isValid()) {
            m_navTree->setCurrentIndex(idx);
            m_navTree->scrollTo(idx);
            onNavTreeClicked(idx);
        }
        m_navigatingHistory = false;
        updateNavButtons();
    }
}

void MainWindow::updateNavButtons()
{
    m_backAction->setEnabled(m_navHistoryPos > 0);
    m_forwardAction->setEnabled(m_navHistoryPos < m_navHistory.size() - 1);
}

void MainWindow::onNavTreeClicked(const QModelIndex& index)
{
    if (!index.isValid())
        return;

    // Push to navigation history (unless we are traversing history)
    if (!m_navigatingHistory) {
        // Trim forward history
        while (m_navHistory.size() > m_navHistoryPos + 1)
            m_navHistory.removeLast();
        m_navHistory.append(QPersistentModelIndex(index));
        m_navHistoryPos = m_navHistory.size() - 1;
        updateNavButtons();
    }

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item)
        return;

    updateBreadcrumb(item);

    if (item->nodeType() == NodeType::Record) {
        populateRecordInfo(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        populateSubrecordView(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        m_refByModel->setRecord(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        m_tabWidget->setCurrentWidget(m_viewTree);
    } else if (item->nodeType() == NodeType::Plugin) {
        m_overrideModel->clear();
        m_recordViewModel->clear();
        m_infoEdit->clear();
        auto& ffi = XEditFFI::instance();
        QString name = ffiString(ffi.xedit_plugin_filename, item->pluginIndex());
        int32_t recCount = ffi.xedit_plugin_record_count
            ? ffi.xedit_plugin_record_count(item->pluginIndex()) : 0;
        int32_t grpCount = ffi.xedit_plugin_group_count
            ? ffi.xedit_plugin_group_count(item->pluginIndex()) : 0;
        int32_t masterCount = ffi.xedit_plugin_master_count
            ? ffi.xedit_plugin_master_count(item->pluginIndex()) : 0;

        QString info;
        info += QString("Plugin: %1\n").arg(name);
        info += QString("Groups: %1\n").arg(grpCount);
        info += QString("Records: %1\n").arg(recCount);
        info += QString("Masters: %1\n").arg(masterCount);

        if (ffi.xedit_plugin_master_name) {
            for (int32_t m = 0; m < masterCount; ++m) {
                QString masterName = ffiString(ffi.xedit_plugin_master_name, item->pluginIndex(), m);
                info += QString("  [%1] %2\n").arg(m).arg(masterName);
            }
        }

        m_infoEdit->setPlainText(info);
        m_tabWidget->setCurrentWidget(m_infoEdit);
        m_refByModel->clear();
    } else if (item->nodeType() == NodeType::Group) {
        m_overrideModel->clear();
        m_recordViewModel->clear();
        m_infoEdit->clear();
        auto& ffi = XEditFFI::instance();
        QString sig  = ffiString(ffi.xedit_group_signature, item->pluginIndex(), item->groupIndex());
        QString name = ffiString(ffi.xedit_group_name, item->pluginIndex(), item->groupIndex());
        int32_t recCount = ffi.xedit_group_record_count
            ? ffi.xedit_group_record_count(item->pluginIndex(), item->groupIndex()) : 0;

        QString info;
        info += QString("Group: %1 (%2)\n").arg(sig, name);
        info += QString("Records: %1\n").arg(recCount);
        m_infoEdit->setPlainText(info);
        m_tabWidget->setCurrentWidget(m_infoEdit);
        m_refByModel->clear();
    }
}

void MainWindow::populateRecordInfo(int pluginIdx, int groupIdx, int recordIdx)
{
    auto& ffi = XEditFFI::instance();

    QString sig      = ffiString(ffi.xedit_record_signature, pluginIdx, groupIdx, recordIdx);
    uint32_t formId  = ffi.xedit_record_form_id
        ? ffi.xedit_record_form_id(pluginIdx, groupIdx, recordIdx) : 0;
    QString editorId = ffiString(ffi.xedit_record_editor_id, pluginIdx, groupIdx, recordIdx);
    int32_t subCount = ffi.xedit_record_subrecord_count
        ? ffi.xedit_record_subrecord_count(pluginIdx, groupIdx, recordIdx) : 0;

    QString friendly = SignatureNames::toFriendlyName(sig);

    QString info;
    info += QString("Signature:  %1").arg(sig);
    if (!friendly.isEmpty() && friendly != sig)
        info += QString(" (%1)").arg(friendly);
    info += "\n";
    info += QString("FormID:     %1\n").arg(formId, 8, 16, QLatin1Char('0'));
    info += QString("Editor ID:  %1\n").arg(editorId.isEmpty() ? "(none)" : editorId);
    info += QString("Subrecords: %1\n").arg(subCount);

    m_infoEdit->setPlainText(info);
    m_tabWidget->setCurrentWidget(m_infoEdit);
}

void MainWindow::populateSubrecordView(int pluginIdx, int groupIdx, int recordIdx)
{
    auto& ffi = XEditFFI::instance();
    const uint32_t formId = ffi.xedit_record_form_id
        ? ffi.xedit_record_form_id(pluginIdx, groupIdx, recordIdx)
        : 0;

    bool useOverrideView = false;
    if (formId != 0) {
        m_overrideModel->loadRecord(formId);
        useOverrideView = (m_overrideModel->columnCount() > 1) && (m_overrideModel->rowCount() > 0);
    } else {
        m_overrideModel->clear();
    }

    auto* conflictDelegate = qobject_cast<ConflictColorDelegate*>(m_viewTree->itemDelegate());
    if (!conflictDelegate)
        conflictDelegate = new ConflictColorDelegate(m_viewTree);

    if (useOverrideView) {
        if (m_viewTree->model() != m_overrideModel)
            m_viewTree->setModel(m_overrideModel);

        const int columnCount = m_overrideModel->columnCount();
        for (int c = 0; c < columnCount; ++c)
            m_viewTree->setItemDelegateForColumn(c, conflictDelegate);

        m_viewTree->setHeaderHidden(false);
        m_viewTree->setRootIsDecorated(true);
        m_viewTree->setItemsExpandable(true);
        m_viewTree->setExpandsOnDoubleClick(true);

        // Expand all nodes so struct subrecord children are visible by default
        // (mirrors original Delphi xEdit behavior)
        m_viewTree->expandAll();

        QHeaderView* header = m_viewTree->header();
        header->setStretchLastSection(true);
        header->setSectionResizeMode(0, QHeaderView::Interactive);
        header->resizeSection(0, 200);
        header->setMinimumSectionSize(80);
        for (int c = 1; c < columnCount; ++c) {
            header->setSectionResizeMode(c, QHeaderView::Interactive);
            header->resizeSection(c, 200);
        }
        return;
    }

    m_recordViewModel->setRecord(pluginIdx, groupIdx, recordIdx);
    if (m_viewTree->model() != m_recordViewModel)
        m_viewTree->setModel(m_recordViewModel);
    m_viewTree->setItemDelegate(conflictDelegate);
    m_viewTree->setHeaderHidden(false);
    QHeaderView* header = m_viewTree->header();
    header->setSectionResizeMode(0, QHeaderView::Interactive);
    header->resizeSection(0, 100);  // Signature
    header->setSectionResizeMode(1, QHeaderView::Interactive);
    header->resizeSection(1, 60);   // Size
    header->setSectionResizeMode(2, QHeaderView::Interactive);
    header->resizeSection(2, 400);  // Data (hex)
    header->setStretchLastSection(true);  // Text column stretches
}

// ---------------------------------------------------------------------------
// Search slots
// ---------------------------------------------------------------------------

void MainWindow::onFormIDSearch()
{
    QString text = m_formIdSearch->text().trimmed();
    if (text.isEmpty()) return;
    bool ok = false;
    uint32_t formId = text.toUInt(&ok, 16);
    if (!ok) {
        logMessage(QString("Invalid FormID hex value: '%1'").arg(text));
        return;
    }
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_search_form_id || !ffi.xedit_plugin_count) {
        logMessage("ERROR: Search functions not available.");
        return;
    }

    struct SearchResult {
        std::atomic<bool> done{false};
        int32_t pluginIdx = -1;
        int32_t groupIdx = -1;
        int32_t recordIdx = -1;
    };
    auto* result = new SearchResult;

    std::thread([result, formId]() {
        auto& ffi = XEditFFI::instance();
        int32_t pluginCount = ffi.xedit_plugin_count();
        for (int32_t p = 0; p < pluginCount; ++p) {
            int32_t gi = -1, ri = -1;
            int32_t rc = ffi.xedit_search_form_id(p, formId, &gi, &ri);
            if (rc >= 0 && gi >= 0 && ri >= 0) {
                result->pluginIdx = p;
                result->groupIdx = gi;
                result->recordIdx = ri;
                break;
            }
        }
        result->done.store(true);
    }).detach();

    statusBar()->showMessage(tr("Searching for FormID %1...").arg(text));
    auto* pollTimer = new QTimer(this);
    connect(pollTimer, &QTimer::timeout, this, [this, pollTimer, result, text]() {
        if (!result->done.load()) return;
        pollTimer->stop();
        pollTimer->deleteLater();
        if (result->pluginIdx >= 0) {
            QModelIndex idx = m_navModel->findRecord(result->pluginIdx, result->groupIdx, result->recordIdx);
            if (idx.isValid()) {
                m_navTree->setCurrentIndex(idx);
                m_navTree->scrollTo(idx);
                onNavTreeClicked(idx);
                logMessage(QString("Found FormID %1 in plugin %2").arg(text).arg(result->pluginIdx));
            }
        } else {
            logMessage(QString("FormID %1 not found.").arg(text));
        }
        statusBar()->showMessage(tr("Ready"));
        delete result;
    });
    pollTimer->start(50);
}

void MainWindow::onEditorIDSearch()
{
    QString query = m_editorIdSearch->text().trimmed();
    if (query.isEmpty()) return;
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_search_editor_id || !ffi.xedit_plugin_count) {
        logMessage("ERROR: Search functions not available.");
        return;
    }

    struct SearchResult {
        std::atomic<bool> done{false};
        int32_t pluginIdx = -1;
        int32_t groupIdx = -1;
        int32_t recordIdx = -1;
        int32_t totalCount = 0;
    };
    auto* result = new SearchResult;

    std::thread([result, query]() {
        auto& ffi = XEditFFI::instance();
        QByteArray queryBa = query.toUtf8();
        int32_t pluginCount = ffi.xedit_plugin_count();
        for (int32_t p = 0; p < pluginCount; ++p) {
            int32_t results[128];
            int32_t count = ffi.xedit_search_editor_id(p, queryBa.constData(), results, 64);
            if (count > 0) {
                result->pluginIdx = p;
                result->groupIdx = results[0];
                result->recordIdx = results[1];
                result->totalCount = count;
                break;
            }
        }
        result->done.store(true);
    }).detach();

    statusBar()->showMessage(tr("Searching for EditorID '%1'...").arg(query));
    auto* pollTimer = new QTimer(this);
    connect(pollTimer, &QTimer::timeout, this, [this, pollTimer, result, query]() {
        if (!result->done.load()) return;
        pollTimer->stop();
        pollTimer->deleteLater();
        if (result->pluginIdx >= 0) {
            QModelIndex idx = m_navModel->findRecord(result->pluginIdx, result->groupIdx, result->recordIdx);
            if (idx.isValid()) {
                m_navTree->setCurrentIndex(idx);
                m_navTree->scrollTo(idx);
                onNavTreeClicked(idx);
                logMessage(QString("Found EditorID '%1' in plugin %2 (%3 result(s))").arg(query).arg(result->pluginIdx).arg(result->totalCount));
            }
        } else {
            logMessage(QString("EditorID '%1' not found.").arg(query));
        }
        statusBar()->showMessage(tr("Ready"));
        delete result;
    });
    pollTimer->start(50);
}

// ---------------------------------------------------------------------------
// Breadcrumb
// ---------------------------------------------------------------------------

void MainWindow::updateBreadcrumb(NavTreeItem* item)
{
    if (!item)
        return;

    auto& ffi = XEditFFI::instance();
    const int pi = item->pluginIndex();
    const QString pluginName = ffiString(ffi.xedit_plugin_filename, pi);
    const QString pluginLabel = QString("[%1] %2")
        .arg(pi, 2, 10, QLatin1Char('0'))
        .arg(pluginName.isEmpty() ? QString("Plugin %1").arg(pi) : pluginName);

    if (item->nodeType() == NodeType::Plugin) {
        setStatusText(pluginLabel);
        return;
    }

    // Build group label
    const QString groupSig = ffiString(ffi.xedit_group_signature, pi, item->groupIndex());
    QString groupLabel = groupSig;
    const QString groupFriendly = SignatureNames::toFriendlyName(groupSig);
    if (!groupFriendly.isEmpty() && groupFriendly != groupSig)
        groupLabel = QString("%1 - %2").arg(groupSig, groupFriendly);
    if (groupLabel.isEmpty())
        groupLabel = QStringLiteral("(group)");

    if (item->nodeType() == NodeType::Group) {
        setStatusText(QString("%1 \\ %2").arg(pluginLabel, groupLabel));
        return;
    }

    // Record node — include FormID and EditorID
    const uint32_t formId = ffi.xedit_record_form_id
        ? ffi.xedit_record_form_id(pi, item->groupIndex(), item->recordIndex())
        : 0;
    const QString formIdHex = QString("%1").arg(formId, 8, 16, QLatin1Char('0')).toUpper();
    const QString editorId = ffiString(ffi.xedit_record_editor_id, pi, item->groupIndex(), item->recordIndex());

    QString recordLabel = QString("[%1]").arg(formIdHex);
    if (!editorId.isEmpty())
        recordLabel += QString(" %1").arg(editorId);

    setStatusText(QString("%1 \\ %2 \\ %3").arg(pluginLabel, groupLabel, recordLabel));
}

// ---------------------------------------------------------------------------
// Find across records
// ---------------------------------------------------------------------------

void MainWindow::onToggleFindBar()
{
    bool show = !m_findBar->isVisible();
    m_findBar->setVisible(show);
    if (show) {
        m_findInput->setFocus();
        m_findInput->selectAll();
    }
}

void MainWindow::onFindNext()
{
    const QString query = m_findInput->text().trimmed();
    if (query.isEmpty())
        return;

    const int searchType = m_findTypeCombo->currentIndex();

    if (searchType == 0) {
        // Editor ID search — reuse existing logic via the search field
        m_editorIdSearch->setText(query);
        onEditorIDSearch();
        return;
    }

    if (searchType == 1) {
        // Form ID search — reuse existing logic via the search field
        m_formIdSearch->setText(query);
        onFormIDSearch();
        return;
    }

    // searchType == 2: Display Name search — walk visible nav tree items
    // Start from the item after the current selection
    QModelIndex startIdx = m_navTree->currentIndex();
    QModelIndex searchFrom;
    if (startIdx.isValid()) {
        searchFrom = m_navTree->indexBelow(startIdx);
    }
    if (!searchFrom.isValid()) {
        searchFrom = m_navModel->index(0, 0);
    }

    if (findDisplayNameMatch(query, searchFrom, true)) {
        return;
    }
    logMessage(QString("Display name '%1' not found.").arg(query));
}

bool MainWindow::findDisplayNameMatch(const QString& query, const QModelIndex& start, bool wrapAround)
{
    // Walk all visible items starting from 'start'
    QModelIndex idx = start;
    QModelIndex firstIdx = m_navModel->index(0, 0);

    while (idx.isValid()) {
        QString text = m_navModel->data(idx, Qt::DisplayRole).toString();
        if (text.contains(query, Qt::CaseInsensitive)) {
            m_navTree->setCurrentIndex(idx);
            m_navTree->scrollTo(idx);
            onNavTreeClicked(idx);
            logMessage(QString("Found display name match: '%1'").arg(text));
            return true;
        }
        idx = m_navTree->indexBelow(idx);
    }

    // Wrap around: search from the top to the original start
    if (wrapAround && start != firstIdx) {
        idx = firstIdx;
        while (idx.isValid() && idx != start) {
            QString text = m_navModel->data(idx, Qt::DisplayRole).toString();
            if (text.contains(query, Qt::CaseInsensitive)) {
                m_navTree->setCurrentIndex(idx);
                m_navTree->scrollTo(idx);
                onNavTreeClicked(idx);
                logMessage(QString("Found display name match (wrapped): '%1'").arg(text));
                return true;
            }
            idx = m_navTree->indexBelow(idx);
        }
    }

    return false;
}

// ---------------------------------------------------------------------------
// Cleaning / context menu slots
// ---------------------------------------------------------------------------

void MainWindow::onNavTreeContextMenu(const QPoint& pos)
{
    QModelIndex index = m_navTree->indexAt(pos);
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item)
        return;

    // Select the right-clicked item so subsequent actions target it
    m_navTree->setCurrentIndex(index);

    QMenu menu(this);

    switch (item->nodeType()) {
    case NodeType::Plugin:
        menu.addAction(m_actQuickClean);
        menu.addAction(m_actCheckITM);
        menu.addAction(m_actRemoveITM);
        menu.addAction(m_actUndeleteRefs);
        if (m_clipboardRecord.valid()) {
            menu.addSeparator();
            menu.addAction(tr("Paste Record"), this, &MainWindow::onPasteRecord);
        }
        break;

    case NodeType::Group:
        if (m_clipboardRecord.valid()) {
            menu.addAction(tr("Paste Record"), this, &MainWindow::onPasteRecord);
        }
        break;

    case NodeType::Record:
        menu.addAction(tr("Copy Record"), this, &MainWindow::onCopyRecord);
        if (m_clipboardRecord.valid()) {
            menu.addAction(tr("Paste as Override"), this, &MainWindow::onPasteAsOverride);
        }
        menu.addSeparator();
        menu.addAction(tr("Delete Record"), this, &MainWindow::onDeleteRecord);
        menu.addAction(tr("Duplicate Record"), this, &MainWindow::onDuplicateRecord);
        break;
    }

    if (!menu.isEmpty())
        menu.exec(m_navTree->viewport()->mapToGlobal(pos));
}

void MainWindow::onCopyRecord()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Record)
        return;

    m_clipboardRecord.pluginIdx = item->pluginIndex();
    m_clipboardRecord.groupIdx = item->groupIndex();
    m_clipboardRecord.recordIdx = item->recordIndex();

    logMessage(QString("Copied record [%1] to clipboard (plugin %2, group %3, record %4)")
        .arg(item->formId(), 8, 16, QChar('0'))
        .arg(item->pluginIndex())
        .arg(item->groupIndex())
        .arg(item->recordIndex()));
}

void MainWindow::onPasteRecord()
{
    if (!m_clipboardRecord.valid()) {
        logMessage("No record in clipboard.");
        return;
    }

    if (!confirmEditAction())
        return;

    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item)
        return;

    int dstPluginIdx = item->pluginIndex();

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_copy_record) {
        logMessage("ERROR: xedit_copy_record not available.");
        return;
    }

    int newRecIdx = ffi.xedit_copy_record(
        m_clipboardRecord.pluginIdx, m_clipboardRecord.groupIdx,
        m_clipboardRecord.recordIdx, dstPluginIdx);

    if (newRecIdx >= 0) {
        logMessage(QString("Pasted record to plugin %1, new record index %2")
            .arg(dstPluginIdx).arg(newRecIdx));
    } else {
        logMessage("ERROR: Failed to paste record.");
    }
}

void MainWindow::onPasteAsOverride()
{
    if (!m_clipboardRecord.valid()) {
        logMessage("No record in clipboard.");
        return;
    }

    if (!confirmEditAction())
        return;

    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item)
        return;

    int dstPluginIdx = item->pluginIndex();

    if (dstPluginIdx == m_clipboardRecord.pluginIdx) {
        logMessage("Paste as Override requires a different destination plugin.");
        return;
    }

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_copy_record) {
        logMessage("ERROR: xedit_copy_record not available.");
        return;
    }

    int newRecIdx = ffi.xedit_copy_record(
        m_clipboardRecord.pluginIdx, m_clipboardRecord.groupIdx,
        m_clipboardRecord.recordIdx, dstPluginIdx);

    if (newRecIdx >= 0) {
        logMessage(QString("Pasted as override to plugin %1, new record index %2")
            .arg(dstPluginIdx).arg(newRecIdx));
    } else {
        logMessage("ERROR: Failed to paste as override.");
    }
}

void MainWindow::onQuickClean()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid()) return;
    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Plugin) {
        logMessage("Quick Clean requires a plugin to be selected.");
        return;
    }
    int pluginIdx = item->pluginIndex();
    logMessage(QString("Quick Clean: starting on plugin %1...").arg(pluginIdx));

    runAsyncTask("Running Quick Clean...", [pluginIdx]() -> QString {
        auto& ffi = XEditFFI::instance();
        QString log;
        if (ffi.xedit_detect_itm) {
            char buf[4096];
            int32_t rc = ffi.xedit_detect_itm(nullptr, pluginIdx, buf, sizeof(buf));
            log += QString("  ITM detection: %1 record(s) found\n").arg(rc >= 0 ? rc : 0);
        }
        if (ffi.xedit_clean_itm) {
            int32_t rc = ffi.xedit_clean_itm(nullptr, pluginIdx);
            log += QString("  ITM cleaned: %1\n").arg(rc >= 0 ? rc : 0);
        }
        if (ffi.xedit_clean_deleted) {
            int32_t rc = ffi.xedit_clean_deleted(nullptr, pluginIdx);
            log += QString("  Deleted refs undeleted: %1\n").arg(rc >= 0 ? rc : 0);
        }
        return log;
    }, [this](const QString& output) {
        logMessage(output.trimmed());
        logMessage("Quick Clean: done.");
    });
}

void MainWindow::onCheckITM()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid()) return;
    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Plugin) {
        logMessage("Check ITM requires a plugin to be selected.");
        return;
    }
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_detect_itm) {
        logMessage("ERROR: xedit_detect_itm not available.");
        return;
    }
    int pluginIdx = item->pluginIndex();

    runAsyncTask("Checking for ITM records...", [pluginIdx]() -> QString {
        auto& ffi = XEditFFI::instance();
        char buf[4096];
        int32_t rc = ffi.xedit_detect_itm(nullptr, pluginIdx, buf, sizeof(buf));
        if (rc < 0) return QString("ERROR: ITM detection failed (code %1)").arg(rc);
        QString result = QString("ITM check: %1 record(s) found.").arg(rc);
        if (rc > 0) result += "\n" + QString::fromUtf8(buf);
        return result;
    }, [this](const QString& output) {
        logMessage(output);
    });
}

void MainWindow::onRemoveITM()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid()) return;
    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Plugin) {
        logMessage("Remove ITM requires a plugin to be selected.");
        return;
    }
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_clean_itm) {
        logMessage("ERROR: xedit_clean_itm not available.");
        return;
    }
    int pluginIdx = item->pluginIndex();

    runAsyncTask("Removing ITM records...", [pluginIdx]() -> QString {
        auto& ffi = XEditFFI::instance();
        int32_t rc = ffi.xedit_clean_itm(nullptr, pluginIdx);
        if (rc < 0) return QString("ERROR: ITM removal failed (code %1)").arg(rc);
        return QString("Removed %1 ITM record(s).").arg(rc);
    }, [this](const QString& output) {
        logMessage(output);
    });
}

void MainWindow::onUndeleteRefs()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid()) return;
    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Plugin) {
        logMessage("Undelete Refs requires a plugin to be selected.");
        return;
    }
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_clean_deleted) {
        logMessage("ERROR: xedit_clean_deleted not available.");
        return;
    }
    int pluginIdx = item->pluginIndex();

    runAsyncTask("Undeleting references...", [pluginIdx]() -> QString {
        auto& ffi = XEditFFI::instance();
        int32_t rc = ffi.xedit_clean_deleted(nullptr, pluginIdx);
        if (rc < 0) return QString("ERROR: Undelete refs failed (code %1)").arg(rc);
        return QString("Undeleted %1 reference(s).").arg(rc);
    }, [this](const QString& output) {
        logMessage(output);
    });
}

// ---------------------------------------------------------------------------
// Async task helper
// ---------------------------------------------------------------------------

void MainWindow::runAsyncTask(const QString& message, std::function<QString()> task, std::function<void(const QString&)> onComplete)
{
    auto* waitDlg = new WaitDialog(this);
    waitDlg->setMessage(message);
    waitDlg->setProgress(-1.0);
    waitDlg->show();

    struct Result {
        std::atomic<bool> done{false};
        QString output;
    };
    auto* result = new Result;

    std::thread([result, task = std::move(task)]() {
        result->output = task();
        result->done.store(true);
    }).detach();

    auto* pollTimer = new QTimer(this);
    auto callback = std::move(onComplete);
    connect(pollTimer, &QTimer::timeout, this, [this, pollTimer, waitDlg, result, callback]() {
        if (!result->done.load()) return;
        pollTimer->stop();
        pollTimer->deleteLater();
        waitDlg->close();
        waitDlg->deleteLater();
        QString output = result->output;
        delete result;
        if (callback) callback(output);
    });
    pollTimer->start(50);
}

// ---------------------------------------------------------------------------
// Logging and status
// ---------------------------------------------------------------------------

void MainWindow::logMessage(const QString& msg)
{
    QString timestamp = QDateTime::currentDateTime().toString("hh:mm:ss");
    m_messagesEdit->appendPlainText(QString("[%1] %2").arg(timestamp, msg));
    std::fprintf(stderr, "[xEdit] %s\n", msg.toUtf8().constData());
    std::fflush(stderr);
}

void MainWindow::setStatusText(const QString& text)
{
    m_breadcrumbLabel->setText(text);
}

// ---------------------------------------------------------------------------
// Dialog slots
// ---------------------------------------------------------------------------

void MainWindow::onOptions()
{
    OptionsDialog dlg(this);
    SettingsManager::load(dlg);
    if (dlg.exec() == QDialog::Accepted) {
        SettingsManager::save(dlg);
        SettingsManager::applyThemeFromSettings();
        SettingsManager::applyFontFromSettings();
    }
}

void MainWindow::onFilterOptions()
{
    FilterOptionsDialog dlg(this);
    if (dlg.exec() == QDialog::Accepted) {
        const QStringList conflictAllLabels = dlg.checkedConflictAll();
        const QStringList conflictThisLabels = dlg.checkedConflictThis();
        m_keepConflictAll = conflictAllValuesFromLabels(conflictAllLabels);
        m_keepConflictThis = conflictThisValuesFromLabels(conflictThisLabels);
        if (conflictAllLabels.size() >= 5)
            m_keepConflictAll.clear();
        if (conflictThisLabels.size() >= 6)
            m_keepConflictThis.clear();
        m_keepSignatures.clear();
        for (const QString& sig : dlg.checkedSignatures())
            m_keepSignatures.insert(sig.toUpper());

        m_editorIdFilterText = dlg.editorIdFilter();
        m_nameFilterText = dlg.nameFilter();
        m_baseEditorIdFilterText = dlg.baseEditorIdFilter();
        m_baseNameFilterText = dlg.baseNameFilter();
        m_filterPersistent = dlg.filterPersistent();
        m_filterDeleted = dlg.filterDeleted();
        m_filterVwd = dlg.filterVWD();
        m_filterHasVwdMesh = dlg.filterHasVWDMesh();
        m_filterInitiallyDisabled = dlg.filterInitiallyDisabled();
        m_excludeMasterPlugin = false;
        m_filterApplied = true;

        const bool unsupportedCriteriaRequested = !m_nameFilterText.isEmpty()
            || !m_baseEditorIdFilterText.isEmpty()
            || !m_baseNameFilterText.isEmpty()
            || m_filterPersistent
            || m_filterDeleted
            || m_filterVwd
            || m_filterHasVwdMesh
            || m_filterInitiallyDisabled;
        if (unsupportedCriteriaRequested) {
            logMessage("Some filter options are not available yet (Name/Base/flag filters) and were ignored.");
        }

        if (!m_keepConflictAll.isEmpty() || !m_keepConflictThis.isEmpty()) {
            // Run conflict detection async, then apply filter when done
            runConflictDetectionAsync([this]() {
                if (!hasActiveRecordFilter()) {
                    removeFilter();
                    logMessage("Filter removed.");
                    return;
                }
                applyCurrentNavFilters();
                logMessage("Filter applied.");
            });
            return;
        }

        if (!hasActiveRecordFilter()) {
            removeFilter();
            logMessage("Filter removed.");
            return;
        }

        applyCurrentNavFilters();
        logMessage("Filter applied.");
    }
}

void MainWindow::onRemoveFilter()
{
    removeFilter();
    logMessage("Filter removed.");
}

void MainWindow::onLegend()
{
    LegendDialog dlg(this);
    dlg.exec();
}

void MainWindow::onScriptEditor()
{
    ScriptDialog dlg(this);
    dlg.exec();
}

void MainWindow::onLogAnalyzer()
{
    LogAnalyzerDialog dlg(this);

    // Pre-populate with current messages pane content if available
    QString messages = m_messagesEdit->toPlainText();
    if (!messages.isEmpty())
        dlg.setLogContent(messages);

    dlg.exec();
}

void MainWindow::onLocalization()
{
    if (m_dataPath.isEmpty()) {
        logMessage("No data directory set. Load plugins first.");
        return;
    }

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_plugin_count) {
        logMessage("FFI not loaded.");
        return;
    }

    // Collect string table entries from all loaded plugins.
    // String tables live at: Data/Strings/<PluginBaseName>_<Language>.<EXT>
    QVector<LocalizationDialog::StringEntry> allEntries;

    QDir stringsDir(m_dataPath + "/Strings");
    if (!stringsDir.exists())
        stringsDir.setPath(m_dataPath + "/strings");
    if (!stringsDir.exists()) {
        logMessage("No Strings directory found. Checking data directory directly.");
        stringsDir.setPath(m_dataPath);
    }

    // Scan for all string table files
    QStringList stringExts = {"*.STRINGS", "*.DLSTRINGS", "*.ILSTRINGS",
                               "*.strings", "*.dlstrings", "*.ilstrings"};
    QFileInfoList stringFiles = stringsDir.entryInfoList(stringExts, QDir::Files | QDir::Readable);

    if (stringFiles.isEmpty()) {
        logMessage("No string table files found.");
        QMessageBox::information(this, tr("Localization"),
            tr("No string table files (.STRINGS, .DLSTRINGS, .ILSTRINGS) found in:\n%1")
                .arg(stringsDir.absolutePath()));
        return;
    }

    int totalParsed = 0;
    for (const QFileInfo& fi : stringFiles) {
        QString ext = fi.suffix().toUpper();
        auto type = StringTableParser::typeFromExtension(ext);
        QString typeDisplay = StringTableParser::displayName(type);

        QVector<StringTableParser::Entry> entries;
        if (StringTableParser::parseFile(fi.absoluteFilePath(), type, entries)) {
            for (const auto& e : entries) {
                LocalizationDialog::StringEntry se;
                se.id    = e.id;
                se.type  = typeDisplay;
                se.value = e.value;
                allEntries.append(se);
            }
            totalParsed += entries.size();
            logMessage(QStringLiteral("Parsed %1: %2 strings")
                .arg(fi.fileName()).arg(entries.size()));
        } else {
            logMessage(QStringLiteral("Failed to parse string table: %1").arg(fi.fileName()));
        }
    }

    logMessage(QStringLiteral("Total localization strings loaded: %1").arg(totalParsed));

    LocalizationDialog dlg(this);
    dlg.setEntries(allEntries);
    dlg.exec();
}

void MainWindow::onLocalizePlugin()
{
    // Determine current plugin name from nav tree selection
    QString pluginName = tr("(no plugin)");
    QString pluginBaseName;
    QModelIndex index = m_navTree->currentIndex();
    if (index.isValid()) {
        NavTreeItem* item = m_navModel->itemFromIndex(index);
        if (item) {
            auto& ffi = XEditFFI::instance();
            pluginName = ffiString(ffi.xedit_plugin_filename, item->pluginIndex());
            if (pluginName.isEmpty())
                pluginName = tr("Plugin %1").arg(item->pluginIndex());
        }
    }

    pluginBaseName = QFileInfo(pluginName).completeBaseName();

    LocalizePluginDialog dlg(pluginName, this);

    // If we have a data path, try to load existing string tables for this plugin
    if (!m_dataPath.isEmpty() && !pluginBaseName.isEmpty()) {
        QDir stringsDir(m_dataPath + "/Strings");
        if (!stringsDir.exists())
            stringsDir.setPath(m_dataPath + "/strings");
        if (!stringsDir.exists())
            stringsDir.setPath(m_dataPath);

        QVector<LocalizePluginDialog::LocalizableString> allStrings;

        QStringList stringExts = {"*.STRINGS", "*.DLSTRINGS", "*.ILSTRINGS",
                                   "*.strings", "*.dlstrings", "*.ilstrings"};
        QFileInfoList files = stringsDir.entryInfoList(stringExts, QDir::Files | QDir::Readable);

        for (const QFileInfo& fi : files) {
            // Match files belonging to this plugin (e.g. Skyrim_English.STRINGS)
            if (!fi.completeBaseName().startsWith(pluginBaseName, Qt::CaseInsensitive))
                continue;

            QString ext = fi.suffix().toUpper();
            auto type = StringTableParser::typeFromExtension(ext);

            QVector<StringTableParser::Entry> entries;
            if (StringTableParser::parseFile(fi.absoluteFilePath(), type, entries)) {
                for (const auto& e : entries) {
                    LocalizePluginDialog::LocalizableString ls;
                    ls.stringId       = e.id;
                    ls.currentValue   = e.value;
                    ls.localizedValue = e.value;
                    allStrings.append(ls);
                }
            }
        }

        if (!allStrings.isEmpty()) {
            dlg.setStrings(allStrings);
            logMessage(QStringLiteral("Loaded %1 localizable strings for %2")
                .arg(allStrings.size()).arg(pluginName));
        }
    }

    if (dlg.exec() == QDialog::Accepted) {
        // Save localized strings back to string table files
        if (m_dataPath.isEmpty() || pluginBaseName.isEmpty())
            return;

        QDir stringsDir(m_dataPath + "/Strings");
        if (!stringsDir.exists()) {
            stringsDir.setPath(m_dataPath + "/strings");
            if (!stringsDir.exists()) {
                QDir(m_dataPath).mkdir("Strings");
                stringsDir.setPath(m_dataPath + "/Strings");
            }
        }

        QVector<LocalizePluginDialog::LocalizableString> result = dlg.strings();
        QVector<StringTableParser::Entry> entries;
        entries.reserve(result.size());

        for (const auto& ls : result) {
            StringTableParser::Entry e;
            e.id    = ls.stringId;
            e.value = ls.localizedValue;
            entries.append(e);
        }

        // Write .STRINGS file (null-terminated format)
        QString outPath = QStringLiteral("%1/%2_English.STRINGS")
            .arg(stringsDir.absolutePath(), pluginBaseName);

        if (StringTableParser::writeFile(outPath, entries, StringTableParser::StringType::Strings)) {
            logMessage(QStringLiteral("Localization saved: %1 (%2 strings)")
                .arg(outPath).arg(entries.size()));
        } else {
            logMessage(QStringLiteral("Failed to save localization file: %1").arg(outPath));
            QMessageBox::warning(this, tr("Error"),
                tr("Could not write localization file:\n%1").arg(outPath));
        }
    }
}

void MainWindow::onLODGen()
{
    LODGenDialog dlg(this);

    // Map UI game label to internal game ID for game-specific LOD options
    static const QHash<QString, QString> labelToGameId = {
        {"Skyrim SE",   "SkyrimSE"},
        {"Fallout 4",   "Fallout4"},
        {"Starfield",   "Starfield"},
        {"Fallout 76",  "Fallout76"},
        {"Fallout NV",  "FalloutNV"},
        {"Fallout 3",   "Fallout3"},
        {"Oblivion",    "Oblivion"},
        {"Morrowind",   "Morrowind"},
    };
    QString gameLabel = m_gameCombo->currentText();
    QString gameId = labelToGameId.value(gameLabel, "SkyrimSE");
    dlg.setGameMode(gameId);

    auto& ffi = XEditFFI::instance();
    QStringList worldspaces;

    if (ffi.xedit_lod_list_worldspaces) {
        QByteArray buf(8192, 0);
        int32_t len = ffi.xedit_lod_list_worldspaces(
            reinterpret_cast<uint8_t*>(buf.data()), buf.size());
        if (len > 0) {
            QString result = QString::fromUtf8(buf.constData(), len);
            worldspaces = result.split('\n', Qt::SkipEmptyParts);
        }
    }

    if (!m_dataPath.isEmpty())
        dlg.setOutputDirectory(m_dataPath);

    dlg.setWorldspaces(worldspaces);
    dlg.exec();
}

void MainWindow::onModGroupSelect()
{
    if (m_dataPath.isEmpty()) {
        logMessage("No data directory set. Load plugins first.");
        return;
    }

    // Scan data directory for *.modgroups files
    QStringList modGroups = ModGroupFile::scanDirectory(m_dataPath);
    if (modGroups.isEmpty()) {
        logMessage(QStringLiteral("No .modgroups files found in: %1").arg(m_dataPath));
        QMessageBox::information(this, tr("Mod Groups"),
            tr("No .modgroups files were found in the data directory:\n%1").arg(m_dataPath));
        return;
    }

    ModGroupSelectDialog dlg(modGroups, this);

    if (dlg.exec() == QDialog::Accepted) {
        QStringList selected = dlg.selectedModGroups();
        if (selected.isEmpty()) {
            logMessage("No mod groups selected.");
        } else {
            logMessage(QStringLiteral("Selected mod groups: %1").arg(selected.join(", ")));
        }
    }
}

void MainWindow::onModGroupEdit()
{
    if (m_dataPath.isEmpty()) {
        logMessage("No data directory set. Load plugins first.");
        return;
    }

    // Let the user pick a .modgroups file to edit (or create a new one)
    QStringList modGroups = ModGroupFile::scanDirectory(m_dataPath);

    // Add a "New..." option at the top
    QStringList choices;
    choices << tr("<Create New Mod Group...>");
    choices.append(modGroups);

    bool ok = false;
    QString selected = QInputDialog::getItem(
        this, tr("Edit Mod Group"), tr("Select a mod group to edit:"),
        choices, 0, false, &ok);
    if (!ok || selected.isEmpty())
        return;

    QString modGroupName;
    QStringList pluginList;
    QStringList headerComments;
    QString filePath;

    if (selected == choices.first()) {
        // Creating a new mod group
        modGroupName.clear();
    } else {
        // Loading an existing mod group file
        modGroupName = selected;
        filePath = QStringLiteral("%1/%2.modgroups").arg(m_dataPath, modGroupName);

        QVector<ModGroupFile::PluginEntry> entries;
        if (!ModGroupFile::load(filePath, entries, &headerComments)) {
            logMessage(QStringLiteral("Failed to load mod group file: %1").arg(filePath));
            QMessageBox::warning(this, tr("Error"),
                tr("Could not read mod group file:\n%1").arg(filePath));
            return;
        }
        pluginList = ModGroupFile::toStringList(entries);
    }

    ModGroupEditDialog dlg(modGroupName, pluginList, this);

    if (dlg.exec() == QDialog::Accepted) {
        QString newName = dlg.modGroupName();
        QStringList newPlugins = dlg.plugins();

        if (newName.isEmpty()) {
            logMessage("Mod group name cannot be empty.");
            return;
        }

        // Determine save path
        QString savePath = QStringLiteral("%1/%2.modgroups").arg(m_dataPath, newName);

        // Convert back to entries
        QVector<ModGroupFile::PluginEntry> entries = ModGroupFile::fromStringList(newPlugins);

        if (!ModGroupFile::save(savePath, entries, headerComments)) {
            logMessage(QStringLiteral("Failed to save mod group file: %1").arg(savePath));
            QMessageBox::warning(this, tr("Error"),
                tr("Could not write mod group file:\n%1").arg(savePath));
            return;
        }

        logMessage(QStringLiteral("Mod group saved: %1 (%2 plugins)")
            .arg(savePath).arg(entries.size()));
    }
}

void MainWindow::onViewElements()
{
    QModelIndex index = m_navTree->currentIndex();
    if (!index.isValid()) {
        logMessage("View Elements requires a record to be selected.");
        return;
    }

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item || item->nodeType() != NodeType::Record) {
        logMessage("View Elements requires a record to be selected.");
        return;
    }

    ViewElementsDialog dlg(item->pluginIndex(), item->groupIndex(),
                           item->recordIndex(), this);
    dlg.exec();
}

void MainWindow::onWorldspaceCellDetail()
{
    WorldspaceCellDetailDialog dlg(this);
    if (dlg.exec() != QDialog::Accepted)
        return;

    QString worldspace = dlg.selectedWorldspace();
    int cellX = dlg.cellX();
    int cellY = dlg.cellY();

    if (worldspace.isEmpty() || worldspace.startsWith('(')) {
        logMessage("No valid worldspace selected.");
        return;
    }

    // Search nav tree for a matching CELL record within the selected worldspace.
    // We look for WRLD groups, find the matching worldspace EditorID, then search
    // its CELL sub-records for the matching grid coordinates.
    auto& ffi = XEditFFI::instance();
    if (!ffi.isLoaded()) {
        logMessage("FFI not loaded.");
        return;
    }

    const int32_t pluginCount = ffi.xedit_plugin_count
        ? ffi.xedit_plugin_count() : 0;

    bool found = false;

    for (int32_t p = 0; p < pluginCount && !found; ++p) {
        const int32_t groupCount = ffi.xedit_plugin_group_count
            ? ffi.xedit_plugin_group_count(p) : 0;

        for (int32_t g = 0; g < groupCount && !found; ++g) {
            QString sig;
            if (ffi.xedit_group_signature) {
                sig = ffiString([&](char* buf, int32_t len) {
                    return ffi.xedit_group_signature(p, g, buf, len);
                });
            }
            if (sig != QStringLiteral("WRLD"))
                continue;

            // Check each record in this WRLD group for our worldspace
            const int32_t recCount = ffi.xedit_group_record_count
                ? ffi.xedit_group_record_count(p, g) : 0;

            for (int32_t r = 0; r < recCount; ++r) {
                QString edid;
                if (ffi.xedit_record_editor_id) {
                    edid = ffiString([&](char* buf, int32_t len) {
                        return ffi.xedit_record_editor_id(p, g, r, buf, len);
                    });
                }
                if (edid != worldspace)
                    continue;

                // Found the worldspace record. Navigate to it in the nav tree.
                QModelIndex navIdx = m_navModel->findRecord(p, g, r);
                if (navIdx.isValid()) {
                    m_navTree->setCurrentIndex(navIdx);
                    m_navTree->scrollTo(navIdx);
                    onNavTreeClicked(navIdx);
                    logMessage(QString("Navigated to worldspace '%1' cell (%2, %3)")
                        .arg(worldspace).arg(cellX).arg(cellY));
                    found = true;
                }
                break;
            }
        }
    }

    if (!found) {
        logMessage(QString("Could not find worldspace '%1' cell (%2, %3) in loaded plugins.")
            .arg(worldspace).arg(cellX).arg(cellY));
    }
}

void MainWindow::onFileSelect()
{
    ensureEngineInitialized();
    if (!m_engineInitialized)
        return;

    // Build plugin file list: prefer already-loaded plugins, fall back to data dir scan
    QStringList files;
    auto& ffi = XEditFFI::instance();
    bool hasLoadedPlugins = false;

    if (ffi.xedit_plugin_count) {
        int32_t count = ffi.xedit_plugin_count();
        for (int32_t i = 0; i < count; ++i) {
            QString name = ffiString(ffi.xedit_plugin_filename, i);
            if (!name.isEmpty())
                files << name;
        }
        hasLoadedPlugins = !files.isEmpty();
    }

    // If no plugins are loaded yet, enumerate available plugins from the data directory
    if (files.isEmpty()) {
        files = enumerateDataDirPlugins();
        if (files.isEmpty()) {
            QMessageBox::information(this, tr("No Files"),
                tr("No plugin files found in:\n%1").arg(m_dataPath));
            return;
        }
    }

    FileSelectDialog dlg(files, this);
    dlg.setWindowTitle(hasLoadedPlugins ? tr("Select Loaded Files") : tr("Select Files to Load"));
    if (dlg.exec() != QDialog::Accepted)
        return;

    QStringList selected = dlg.selectedFiles();
    if (selected.isEmpty())
        return;

    if (hasLoadedPlugins) {
        // Already loaded -- report what the user selected
        logMessage(QString("Selected %1 file(s): %2")
            .arg(selected.size()).arg(selected.join(", ")));
    } else {
        // Not loaded yet -- load the selected plugins from the data directory
        logMessage(QString("Loading %1 selected file(s)...").arg(selected.size()));
        QStringList fullPaths;
        for (const QString& name : selected) {
            fullPaths.append(m_dataPath + "/" + name);
        }
        loadPluginsAndBuildIndex(fullPaths);
    }
}

void MainWindow::onDeveloperMessage()
{
    QString html = "<h3>Developer Message</h3>"
                   "<p>xEdit 4.1.6 Linux Port is under active development.</p>"
                   "<p>Please report issues on the project tracker.</p>";
    DeveloperMessageDialog dlg(html, this);
    dlg.exec();
}

void MainWindow::onTipOfTheDay()
{
    QStringList tips;
    tips << "Use Ctrl+F to open the Find bar (search by Editor ID, Form ID, or Display Name)."
         << "Right-click a plugin node for cleaning options."
         << "Use Ctrl+1 through Ctrl+5 to switch between tabs."
         << "Drag and drop records in the nav tree to reorder them."
         << "Use the Filter Options dialog to narrow down visible records.";
    TipDialog dlg(tips, this);
    dlg.exec();
}

// ---------------------------------------------------------------------------
// Referenced By navigation
// ---------------------------------------------------------------------------

void MainWindow::onRefByDoubleClicked(const QModelIndex& index)
{
    if (!index.isValid())
        return;

    const RefByTableModel::RefEntry* entry = m_refByModel->entryAt(index.row());
    if (!entry)
        return;

    QModelIndex navIdx = m_navModel->findRecord(entry->pluginIdx, entry->groupIdx, entry->recordIdx);
    if (navIdx.isValid()) {
        m_navTree->setCurrentIndex(navIdx);
        m_navTree->scrollTo(navIdx);
        onNavTreeClicked(navIdx);
    }
}
