#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QVector>

class QAction;
class QComboBox;
class QLineEdit;
class QMenu;
class QPlainTextEdit;
class QSplitter;
class QTabWidget;
class QTableView;
class QTextBrowser;
class QTreeView;
class NavTreeModel;
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
    void onOptions();
    void onFilterOptions();
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

private:
    void createMenuBar();
    void createToolBar();
    void createCentralWidget();
    void createStatusBar();
    void setupShortcuts();
    void ensureEngineInitialized();
    void populateRecordInfo(int pluginIdx, int groupIdx, int recordIdx);
    void populateSubrecordView(int pluginIdx, int groupIdx, int recordIdx);

    // Models
    NavTreeModel*      m_navModel          = nullptr;
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
    bool            m_engineInitialized = false;
    QString         m_currentGame;
    QString         m_dataPath;
    QVector<int>    m_pendingPlugins;

    // Actions (for context menu reuse)
    QAction*        m_actQuickClean     = nullptr;
    QAction*        m_actCheckITM       = nullptr;
    QAction*        m_actRemoveITM      = nullptr;
    QAction*        m_actUndeleteRefs   = nullptr;
};

#endif // MAINWINDOW_H
