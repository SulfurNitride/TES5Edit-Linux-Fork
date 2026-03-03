#pragma once

#include <QComboBox>
#include <QDialog>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QTableWidget>

#include <vector>

class LogAnalyzerDialog : public QDialog {
    Q_OBJECT
public:
    explicit LogAnalyzerDialog(QWidget* parent = nullptr);

    /// Pre-populate the file path (e.g. from main window context)
    void setLogFilePath(const QString& path);

    /// Log severity levels recognised by the analyser
    enum class LogLevel { All, Error, Warning, Info };

    /// A single parsed log entry
    struct LogEntry {
        QString timestamp;
        QString level;     // "Error", "Warning", "Info"
        QString source;
        QString message;
        int     lineNumber = 0;
    };

private slots:
    void onBrowse();
    void onAnalyze();
    void onFilterChanged();
    void onSearchTextChanged(const QString& text);
    void onTableSelectionChanged();

private:
    void buildFileSection(QLayout* layout);
    void buildFilterSection(QLayout* layout);
    void buildContentArea(QLayout* layout);
    void buildSummarySection(QLayout* layout);

    void loadLogFile(const QString& filePath);
    void parseLogContent(const QString& content);
    void applyFilters();
    void populateTable(const std::vector<const LogEntry*>& entries);
    void updateSummary();

    static QString normaliseLevel(const QString& raw);
    static QString extractPapyrusSource(const QString& message);

    // File selection
    QLineEdit*      m_editFilePath;
    QPushButton*    m_btnBrowse;
    QPushButton*    m_btnAnalyze;

    // Filter controls
    QComboBox*      m_comboLogLevel;
    QLineEdit*      m_editSearch;

    // Content display
    QPlainTextEdit* m_logView;
    QTableWidget*   m_tableEntries;

    // Summary labels
    QLabel*         m_lblTotalEntries;
    QLabel*         m_lblErrorCount;
    QLabel*         m_lblWarningCount;
    QLabel*         m_lblInfoCount;

    // Parsed data
    std::vector<LogEntry> m_allEntries;
    QString               m_rawContent;
};
