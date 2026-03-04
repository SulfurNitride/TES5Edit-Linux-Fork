#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QPersistentModelIndex>
#include <QSet>
#include <QStringList>
#include <QVector>
#include <functional>

class QAction;
class QComboBox;
class QLabel;
class QLineEdit;
class QMenu;
class QPlainTextEdit;
class QPushButton;
class QSplitter;
class QTabWidget;
class QTableView;
class QTextBrowser;
class QTreeView;
class NavTreeModel;
class NavTreeItem;
class OverrideViewModel;
class RecordViewModel;
class RefByTableModel;
class ConflictColorDelegate;
class NavColorDelegate;

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(QWidget* parent = nullptr);
    ~MainWindow() override;

public slots:
    void logMessage(const QString& msg);
    void setStatusText(const QString& text);

private slots:
    // File menu
    void onFileOpen();
    void onFileMO2Open();
    void onFileSave();
    void onFileExit();

    // Navigation
    void onNavTreeClicked(const QModelIndex& index);
    void onFormIDSearch();
    void onEditorIDSearch();

    // Context menu cleaning
    void onQuickClean();
    void onCheckITM();
    void onRemoveITM();
    void onUndeleteRefs();

    // Nav tree context menu
    void onNavTreeContextMenu(const QPoint& pos);

    // Dialogs
    void onModuleSelect();
    void onOptions();
    void onFilterOptions();
    void onRemoveFilter();
    void onLegend();
    void onScriptEditor();
    void onLogAnalyzer();
    void onLocalization();
    void onLocalizePlugin();
    void onLODGen();
    void onModGroupSelect();
    void onModGroupEdit();
    void onViewElements();
    void onWorldspaceCellDetail();
    void onFileSelect();
    void onDeveloperMessage();
    void onTipOfTheDay();

    // Navigation history
    void onNavBack();
    void onNavForward();
    void updateNavButtons();

    // Referenced By navigation
    void onRefByDoubleClicked(const QModelIndex& index);

    // Keyboard shortcut slots
    void onFocusFormIDSearch();
    void onFocusEditorIDSearch();
    void onFocusFileFilter();
    void onRefreshView();
    void onDeleteRecord();
    void onDuplicateRecord();
    void onClearSelection();
    void onSwitchTab(int tabIndex);

    // Copy/Paste record slots
    void onCopyRecord();
    void onPasteRecord();
    void onPasteAsOverride();

    // Find across records
    void onToggleFindBar();
    void onFindNext();

private:
    void createMenuBar();
    void createToolBar();
    void createCentralWidget();
    void createStatusBar();
    void setupShortcuts();
    bool confirmEditAction();
    void ensureEngineInitialized();
    void applyCurrentNavFilters();
    void ensureAllNavRecordsLoaded();
    bool recordPassesCurrentFilter(int pluginIdx, int groupIdx, int recordIdx, NavTreeItem* item) const;
    void removeFilter();
    bool hasActiveRecordFilter() const;
    void rebuildNavTreeFromLoadedPlugins();
    QStringList enumerateDataDirPlugins() const;
    void loadPluginsAndBuildIndex(const QStringList& filePaths);
    void applyQuickShowConflictsFilterIfRequested();
    void runConflictDetectionAsync(std::function<void()> onComplete = nullptr);
    void runAsyncTask(const QString& message, std::function<QString()> task, std::function<void(const QString&)> onComplete = nullptr);
    void populateRecordInfo(int pluginIdx, int groupIdx, int recordIdx);
    void populateSubrecordView(int pluginIdx, int groupIdx, int recordIdx);
    void updateBreadcrumb(NavTreeItem* item);
    bool findDisplayNameMatch(const QString& query, const QModelIndex& start, bool wrapAround);

    // Models
    NavTreeModel*      m_navModel          = nullptr;
    OverrideViewModel* m_overrideModel     = nullptr;
    RecordViewModel*   m_recordViewModel   = nullptr;
    RefByTableModel*   m_refByModel        = nullptr;

    // Widgets
    QTreeView*      m_navTree           = nullptr;
    QPlainTextEdit* m_messagesEdit      = nullptr;
    QPlainTextEdit* m_infoEdit          = nullptr;
    QTreeView*      m_viewTree          = nullptr;
    QTableView*     m_refByTable        = nullptr;
    QTextBrowser*   m_whatsNewBrowser   = nullptr;
    QTabWidget*     m_tabWidget         = nullptr;
    QComboBox*      m_gameCombo         = nullptr;
    QLineEdit*      m_formIdSearch      = nullptr;
    QLineEdit*      m_editorIdSearch    = nullptr;
    QLineEdit*      m_fileFilter        = nullptr;
    QSplitter*      m_splitter          = nullptr;

    // State
    bool            m_editWarningShown  = false;
    bool            m_engineInitialized = false;
    bool            m_filterApplied     = false;
    bool            m_gameAutoDetected  = false;
    QString         m_currentGame;
    QString         m_dataPath;
    QString         m_autoDetectedGameMode;
    QVector<int>    m_pendingPlugins;
    QString         m_pluginNameFilterText;

    // Current active record filter criteria
    QSet<int>       m_keepConflictAll;
    QSet<int>       m_keepConflictThis;
    QSet<QString>   m_keepSignatures;
    QString         m_editorIdFilterText;
    QString         m_nameFilterText;
    QString         m_baseEditorIdFilterText;
    QString         m_baseNameFilterText;
    bool            m_filterPersistent  = false;
    bool            m_filterDeleted     = false;
    bool            m_filterVwd         = false;
    bool            m_filterHasVwdMesh  = false;
    bool            m_filterInitiallyDisabled = false;
    bool            m_excludeMasterPlugin = false;

    // Find bar widgets
    QWidget*        m_findBar           = nullptr;
    QLineEdit*      m_findInput         = nullptr;
    QComboBox*      m_findTypeCombo     = nullptr;
    QPushButton*    m_findNextBtn       = nullptr;

    // Navigation history
    QList<QPersistentModelIndex> m_navHistory;
    int             m_navHistoryPos     = -1;
    bool            m_navigatingHistory = false;
    QAction*        m_backAction        = nullptr;
    QAction*        m_forwardAction     = nullptr;

    // Status bar breadcrumb
    QLabel*         m_breadcrumbLabel   = nullptr;

    // Record clipboard for copy/paste
    struct ClipboardRecord {
        int pluginIdx = -1;
        int groupIdx = -1;
        int recordIdx = -1;
        bool valid() const { return pluginIdx >= 0; }
    };
    ClipboardRecord m_clipboardRecord;

    // Actions (for context menu reuse)
    QAction*        m_actQuickClean     = nullptr;
    QAction*        m_actCheckITM       = nullptr;
    QAction*        m_actRemoveITM      = nullptr;
    QAction*        m_actUndeleteRefs   = nullptr;
};

#endif // MAINWINDOW_H
