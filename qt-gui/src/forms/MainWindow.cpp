#include "MainWindow.h"

#include <QAction>
#include <QComboBox>
#include <QDateTime>
#include <QFileDialog>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QHash>
#include <QHeaderView>
#include <QLineEdit>
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

#include "ffi/XEditFFI.h"
#include "ffi/XEditTypes.h"
#include "models/NavTreeModel.h"
#include "models/NavTreeItem.h"
#include "models/RecordViewModel.h"
#include "models/RefByTableModel.h"
#include "delegates/ConflictColorDelegate.h"
#include "delegates/NavColorDelegate.h"
#include "forms/ModuleSelectDialog.h"
#include "forms/OptionsDialog.h"
#include "forms/FilterOptionsDialog.h"
#include "forms/LegendDialog.h"
#include "forms/ScriptDialog.h"
#include "forms/LogAnalyzerDialog.h"
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
#include "util/SignatureNames.h"
#include "util/StringBuffer.h"

// ---------------------------------------------------------------------------
// Construction / destruction
// ---------------------------------------------------------------------------

MainWindow::MainWindow(QWidget* parent)
    : QMainWindow(parent)
{
    setWindowTitle("xEdit 4.1.6");
    setMinimumSize(1200, 700);

    m_navModel = new NavTreeModel(this);
    m_recordViewModel = new RecordViewModel(this);
    m_refByModel = new RefByTableModel(this);

    createMenuBar();
    createToolBar();
    createCentralWidget();
    createStatusBar();
    setupShortcuts();

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

    // --- View ---
    QMenu* viewMenu = menuBar()->addMenu(tr("&View"));
    viewMenu->addAction(tr("Expand All"),      [this]() { m_navTree->expandAll(); });
    viewMenu->addAction(tr("Collapse All"),    [this]() { m_navTree->collapseAll(); });
    viewMenu->addSeparator();
    viewMenu->addAction(tr("View Elements..."), this, &MainWindow::onViewElements);
    viewMenu->addAction(tr("Worldspace Cell Detail..."), this, &MainWindow::onWorldspaceCellDetail);
    viewMenu->addSeparator();
    viewMenu->addAction(tr("Filter Options..."), this, &MainWindow::onFilterOptions);

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

    toolbar->addAction(tr("<"), [this]() { /* TODO: back navigation */ });
    toolbar->addAction(tr(">"), [this]() { /* TODO: forward navigation */ });

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
    toolbar->addWidget(m_gameCombo);
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
    leftLayout->addWidget(m_fileFilter);

    m_splitter->addWidget(leftPanel);

    // --- Right panel: tab widget (tabs at bottom) ---
    m_tabWidget = new QTabWidget;
    m_tabWidget->setTabPosition(QTabWidget::South);

    // View tab
    m_viewTree = new QTreeView;
    m_viewTree->setModel(m_recordViewModel);
    m_viewTree->setItemDelegate(new ConflictColorDelegate(m_viewTree));
    m_viewTree->setAlternatingRowColors(true);
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
        "<p>This is an early preview of the xEdit Linux port, built with a "
        "Rust core and Qt 6 GUI.</p>"
        "<ul>"
        "<li>Native plugin loading via Rust FFI</li>"
        "<li>Lazy-loading navigation tree</li>"
        "<li>ITM detection and cleaning</li>"
        "<li>MO2 integration support</li>"
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
    statusBar()->showMessage(tr("Ready"));
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

void MainWindow::setupShortcuts()
{
    // Ctrl+F: Focus FormID search field
    auto* scFocusFormID = new QShortcut(QKeySequence(tr("Ctrl+F")), this);
    connect(scFocusFormID, &QShortcut::activated, this, &MainWindow::onFocusFormIDSearch);

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

    // TODO: Call FFI delete function once available, e.g.:
    //   ffi.xedit_delete_record(pluginIdx, groupIdx, recordIdx);
    logMessage(QString("Delete record stub: plugin %1, group %2, record %3")
        .arg(item->pluginIndex())
        .arg(item->groupIndex())
        .arg(item->recordIndex()));
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

    // TODO: Call FFI duplicate function once available, e.g.:
    //   ffi.xedit_duplicate_record(pluginIdx, groupIdx, recordIdx);
    logMessage(QString("Duplicate record stub: plugin %1, group %2, record %3")
        .arg(item->pluginIndex())
        .arg(item->groupIndex())
        .arg(item->recordIndex()));
}

void MainWindow::onClearSelection()
{
    m_navTree->clearSelection();
    m_navTree->setCurrentIndex(QModelIndex());
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

    // Ask the user to select a data directory the first time
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
    m_currentGame = m_gameCombo->currentText();
    QString gameName = gameMap.value(m_currentGame, "SSE");
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

// ---------------------------------------------------------------------------
// File menu slots
// ---------------------------------------------------------------------------

void MainWindow::onFileOpen()
{
    ensureEngineInitialized();
    if (!m_engineInitialized)
        return;

    auto& ffi = XEditFFI::instance();

    QStringList files = QFileDialog::getOpenFileNames(
        this, tr("Open Plugin Files"), m_dataPath,
        tr("Plugin files (*.esp *.esm *.esl);;All files (*)"));

    m_pendingPlugins.clear();
    for (const QString& filePath : files) {
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
                    statusBar()->showMessage(tr("Ready"));
                }
            });
            buildTimer->start(500); // Poll every 500ms
        } else if (ffi.xedit_build_refby_index) {
            logMessage("Building referenced-by index...");
            ffi.xedit_build_refby_index();
            logMessage("Referenced-by index built.");
            for (int idx : m_pendingPlugins) {
                m_navModel->addPlugin(idx);
            }
            logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
            m_pendingPlugins.clear();
            statusBar()->showMessage(tr("Ready"));
        } else {
            for (int idx : m_pendingPlugins) {
                m_navModel->addPlugin(idx);
            }
            logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
            m_pendingPlugins.clear();
        }
    }
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
        statusBar()->showMessage(tr("Ready"));
    } else if (!m_pendingPlugins.isEmpty()) {
        for (int idx : m_pendingPlugins) {
            m_navModel->addPlugin(idx);
        }
        logMessage(QString("Navigation tree populated with %1 plugin(s).").arg(m_pendingPlugins.size()));
        m_pendingPlugins.clear();
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

    // Save each loaded plugin back to its original path
    for (int32_t i = 0; i < count; ++i) {
        QString name = ffiString(ffi.xedit_plugin_filename, i);
        if (name.isEmpty())
            continue;

        QString fullPath = m_dataPath + "/" + name;
        QByteArray pathBa = fullPath.toUtf8();
        int32_t rc = ffi.xedit_save_plugin(i, pathBa.constData());
        if (rc < 0) {
            logMessage(QString("ERROR: Failed to save '%1' (code %2)")
                .arg(name).arg(rc));
        } else {
            logMessage(QString("Saved '%1'").arg(name));
        }
    }
}

void MainWindow::onFileExit()
{
    close();
}

// ---------------------------------------------------------------------------
// Navigation slots
// ---------------------------------------------------------------------------

void MainWindow::onNavTreeClicked(const QModelIndex& index)
{
    if (!index.isValid())
        return;

    NavTreeItem* item = m_navModel->itemFromIndex(index);
    if (!item)
        return;

    if (item->nodeType() == NodeType::Record) {
        populateRecordInfo(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        populateSubrecordView(item->pluginIndex(), item->groupIndex(), item->recordIndex());
        m_refByModel->setRecord(item->pluginIndex(), item->groupIndex(), item->recordIndex());
    } else if (item->nodeType() == NodeType::Plugin) {
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
    m_recordViewModel->setRecord(pluginIdx, groupIdx, recordIdx);

    m_viewTree->header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
    m_viewTree->header()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    m_viewTree->header()->setSectionResizeMode(2, QHeaderView::ResizeToContents);
    m_viewTree->header()->setStretchLastSection(true);
}

// ---------------------------------------------------------------------------
// Search slots
// ---------------------------------------------------------------------------

void MainWindow::onFormIDSearch()
{
    QString text = m_formIdSearch->text().trimmed();
    if (text.isEmpty())
        return;

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

    int32_t pluginCount = ffi.xedit_plugin_count();
    for (int32_t p = 0; p < pluginCount; ++p) {
        int32_t groupIdx  = -1;
        int32_t recordIdx = -1;
        int32_t rc = ffi.xedit_search_form_id(p, formId, &groupIdx, &recordIdx);
        if (rc >= 0 && groupIdx >= 0 && recordIdx >= 0) {
            QModelIndex idx = m_navModel->findRecord(p, groupIdx, recordIdx);
            if (idx.isValid()) {
                m_navTree->setCurrentIndex(idx);
                m_navTree->scrollTo(idx);
                onNavTreeClicked(idx);
                logMessage(QString("Found FormID %1 in plugin %2")
                    .arg(text).arg(p));
                return;
            }
        }
    }

    logMessage(QString("FormID %1 not found.").arg(text));
}

void MainWindow::onEditorIDSearch()
{
    QString query = m_editorIdSearch->text().trimmed();
    if (query.isEmpty())
        return;

    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_search_editor_id || !ffi.xedit_plugin_count) {
        logMessage("ERROR: Search functions not available.");
        return;
    }

    QByteArray queryBa = query.toUtf8();
    int32_t pluginCount = ffi.xedit_plugin_count();

    for (int32_t p = 0; p < pluginCount; ++p) {
        // Buffer for up to 64 result pairs (group_idx, record_idx interleaved)
        int32_t results[128];
        int32_t count = ffi.xedit_search_editor_id(
            p, queryBa.constData(), results, 64);

        if (count > 0) {
            // Results are pairs: [group0, record0, group1, record1, ...]
            int32_t groupIdx  = results[0];
            int32_t recordIdx = results[1];
            QModelIndex idx = m_navModel->findRecord(p, groupIdx, recordIdx);
            if (idx.isValid()) {
                m_navTree->setCurrentIndex(idx);
                m_navTree->scrollTo(idx);
                onNavTreeClicked(idx);
                logMessage(QString("Found EditorID '%1' in plugin %2 (%3 result(s))")
                    .arg(query).arg(p).arg(count));
                return;
            }
        }
    }

    logMessage(QString("EditorID '%1' not found.").arg(query));
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
    if (!item || item->nodeType() != NodeType::Plugin)
        return;

    QMenu menu(this);
    menu.addAction(m_actQuickClean);
    menu.addAction(m_actCheckITM);
    menu.addAction(m_actRemoveITM);
    menu.addAction(m_actUndeleteRefs);
    menu.exec(m_navTree->viewport()->mapToGlobal(pos));
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
    auto& ffi = XEditFFI::instance();

    logMessage(QString("Quick Clean: starting on plugin %1...").arg(pluginIdx));

    // Check ITM
    if (ffi.xedit_detect_itm) {
        char buf[4096];
        int32_t rc = ffi.xedit_detect_itm(nullptr, pluginIdx, buf, sizeof(buf));
        logMessage(QString("  ITM detection: %1 record(s) found").arg(rc >= 0 ? rc : 0));
    }

    // Clean ITM
    if (ffi.xedit_clean_itm) {
        int32_t rc = ffi.xedit_clean_itm(nullptr, pluginIdx);
        logMessage(QString("  ITM cleaned: %1").arg(rc >= 0 ? rc : 0));
    }

    // Undelete refs
    if (ffi.xedit_clean_deleted) {
        int32_t rc = ffi.xedit_clean_deleted(nullptr, pluginIdx);
        logMessage(QString("  Deleted refs undeleted: %1").arg(rc >= 0 ? rc : 0));
    }

    logMessage("Quick Clean: done.");
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

    char buf[4096];
    int32_t rc = ffi.xedit_detect_itm(nullptr, item->pluginIndex(), buf, sizeof(buf));
    if (rc < 0) {
        logMessage(QString("ERROR: ITM detection failed (code %1)").arg(rc));
    } else {
        logMessage(QString("ITM check: %1 record(s) found.").arg(rc));
        if (rc > 0)
            logMessage(QString::fromUtf8(buf));
    }
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

    int32_t rc = ffi.xedit_clean_itm(nullptr, item->pluginIndex());
    if (rc < 0) {
        logMessage(QString("ERROR: ITM removal failed (code %1)").arg(rc));
    } else {
        logMessage(QString("Removed %1 ITM record(s).").arg(rc));
    }
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

    int32_t rc = ffi.xedit_clean_deleted(nullptr, item->pluginIndex());
    if (rc < 0) {
        logMessage(QString("ERROR: Undelete refs failed (code %1)").arg(rc));
    } else {
        logMessage(QString("Undeleted %1 reference(s).").arg(rc));
    }
}

// ---------------------------------------------------------------------------
// Logging and status
// ---------------------------------------------------------------------------

void MainWindow::logMessage(const QString& msg)
{
    QString timestamp = QDateTime::currentDateTime().toString("hh:mm:ss");
    m_messagesEdit->appendPlainText(QString("[%1] %2").arg(timestamp, msg));
}

void MainWindow::setStatusText(const QString& text)
{
    statusBar()->showMessage(text);
}

// ---------------------------------------------------------------------------
// Dialog slots
// ---------------------------------------------------------------------------

void MainWindow::onOptions()
{
    OptionsDialog dlg(this);
    dlg.exec();
}

void MainWindow::onFilterOptions()
{
    FilterOptionsDialog dlg(this);
    if (dlg.exec() == QDialog::Accepted) {
        logMessage("Filter applied.");
    }
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
    dlg.exec();
}

void MainWindow::onLocalization()
{
    LocalizationDialog dlg(this);
    dlg.exec();
}

void MainWindow::onLocalizePlugin()
{
    // Determine current plugin name from nav tree selection
    QString pluginName = tr("(no plugin)");
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

    LocalizePluginDialog dlg(pluginName, this);
    dlg.exec();
}

void MainWindow::onLODGen()
{
    LODGenDialog dlg(this);
    dlg.exec();
}

void MainWindow::onModGroupSelect()
{
    // Build a simple list of mod groups (placeholder for now)
    QStringList modGroups;
    modGroups << "Default" << "Unofficial Patches" << "Texture Overhauls";
    ModGroupSelectDialog dlg(modGroups, this);
    dlg.exec();
}

void MainWindow::onModGroupEdit()
{
    ModGroupEditDialog dlg({}, {}, this);
    dlg.exec();
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
    dlg.exec();
}

void MainWindow::onFileSelect()
{
    // Build plugin file list from FFI
    QStringList files;
    auto& ffi = XEditFFI::instance();
    if (ffi.xedit_plugin_count) {
        int32_t count = ffi.xedit_plugin_count();
        for (int32_t i = 0; i < count; ++i) {
            QString name = ffiString(ffi.xedit_plugin_filename, i);
            if (!name.isEmpty())
                files << name;
        }
    }

    if (files.isEmpty()) {
        // Fallback: let user browse for files
        files << "(No plugins loaded)";
    }

    FileSelectDialog dlg(files, this);
    if (dlg.exec() == QDialog::Accepted) {
        QStringList selected = dlg.selectedFiles();
        logMessage(QString("Selected %1 file(s).").arg(selected.size()));
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
    tips << "Use Ctrl+F to quickly search by FormID."
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
