#include "LogAnalyzerDialog.h"

#include <QFileDialog>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QMessageBox>
#include <QSplitter>
#include <QVBoxLayout>

#include <QFile>
#include <QRegularExpression>
#include <QTextStream>

#include <algorithm>

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------
LogAnalyzerDialog::LogAnalyzerDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Log Analyzer");
    resize(900, 600);

    auto* mainLayout = new QVBoxLayout(this);

    buildFileSection(mainLayout);
    buildFilterSection(mainLayout);
    buildContentArea(mainLayout);
    buildSummarySection(mainLayout);
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------
void LogAnalyzerDialog::setLogFilePath(const QString& path)
{
    m_editFilePath->setText(path);
}

void LogAnalyzerDialog::setLogContent(const QString& content)
{
    m_editFilePath->setText(tr("(messages pane)"));
    m_rawContent = content;
    m_logView->setPlainText(m_rawContent);
    parseLogContent(m_rawContent);
    applyFilters();
    updateSummary();
}

// ---------------------------------------------------------------------------
// UI construction
// ---------------------------------------------------------------------------
void LogAnalyzerDialog::buildFileSection(QLayout* layout)
{
    auto* grp = new QGroupBox("Log File", this);
    auto* row = new QHBoxLayout(grp);

    auto* lblFile = new QLabel("File:", grp);
    m_editFilePath = new QLineEdit(grp);
    m_editFilePath->setPlaceholderText("Select a log file...");

    m_btnBrowse = new QPushButton("Browse...", grp);
    m_btnAnalyze = new QPushButton("Analyze", grp);

    connect(m_btnBrowse,  &QPushButton::clicked, this, &LogAnalyzerDialog::onBrowse);
    connect(m_btnAnalyze, &QPushButton::clicked, this, &LogAnalyzerDialog::onAnalyze);

    row->addWidget(lblFile);
    row->addWidget(m_editFilePath, 1);
    row->addWidget(m_btnBrowse);
    row->addWidget(m_btnAnalyze);

    layout->addWidget(grp);
}

void LogAnalyzerDialog::buildFilterSection(QLayout* layout)
{
    auto* grp = new QGroupBox("Filters", this);
    auto* row = new QHBoxLayout(grp);

    auto* lblLevel = new QLabel("Log Level:", grp);
    m_comboLogLevel = new QComboBox(grp);
    m_comboLogLevel->addItems({"All", "Error", "Warning", "Info"});

    auto* lblSearch = new QLabel("Search:", grp);
    m_editSearch = new QLineEdit(grp);
    m_editSearch->setPlaceholderText("Filter by text...");

    connect(m_comboLogLevel, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &LogAnalyzerDialog::onFilterChanged);
    connect(m_editSearch, &QLineEdit::textChanged,
            this, &LogAnalyzerDialog::onSearchTextChanged);

    row->addWidget(lblLevel);
    row->addWidget(m_comboLogLevel);
    row->addSpacing(16);
    row->addWidget(lblSearch);
    row->addWidget(m_editSearch, 1);

    layout->addWidget(grp);
}

void LogAnalyzerDialog::buildContentArea(QLayout* layout)
{
    auto* splitter = new QSplitter(Qt::Vertical, this);

    // Parsed entries table
    m_tableEntries = new QTableWidget(0, 4, this);
    m_tableEntries->setHorizontalHeaderLabels({"Timestamp", "Level", "Source", "Message"});
    m_tableEntries->horizontalHeader()->setStretchLastSection(true);
    m_tableEntries->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_tableEntries->setSelectionMode(QAbstractItemView::SingleSelection);
    m_tableEntries->setEditTriggers(QAbstractItemView::NoEditTriggers);
    m_tableEntries->setSortingEnabled(true);
    m_tableEntries->verticalHeader()->setVisible(false);
    m_tableEntries->setColumnWidth(0, 160);
    m_tableEntries->setColumnWidth(1, 70);
    m_tableEntries->setColumnWidth(2, 160);

    connect(m_tableEntries, &QTableWidget::itemSelectionChanged,
            this, &LogAnalyzerDialog::onTableSelectionChanged);

    // Raw log view
    m_logView = new QPlainTextEdit(this);
    m_logView->setReadOnly(true);
    m_logView->setLineWrapMode(QPlainTextEdit::NoWrap);
    m_logView->setFont(QFont("Monospace", 9));

    splitter->addWidget(m_tableEntries);
    splitter->addWidget(m_logView);
    splitter->setStretchFactor(0, 3);
    splitter->setStretchFactor(1, 2);

    layout->addWidget(splitter);
}

void LogAnalyzerDialog::buildSummarySection(QLayout* layout)
{
    auto* grp = new QGroupBox("Summary", this);
    auto* row = new QHBoxLayout(grp);

    m_lblTotalEntries = new QLabel("Total: 0", grp);
    m_lblErrorCount   = new QLabel("Errors: 0", grp);
    m_lblWarningCount = new QLabel("Warnings: 0", grp);
    m_lblInfoCount    = new QLabel("Info: 0", grp);

    m_lblErrorCount->setStyleSheet("color: red; font-weight: bold;");
    m_lblWarningCount->setStyleSheet("color: orange; font-weight: bold;");
    m_lblInfoCount->setStyleSheet("color: blue;");

    row->addWidget(m_lblTotalEntries);
    row->addSpacing(24);
    row->addWidget(m_lblErrorCount);
    row->addSpacing(24);
    row->addWidget(m_lblWarningCount);
    row->addSpacing(24);
    row->addWidget(m_lblInfoCount);
    row->addStretch();

    layout->addWidget(grp);
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------
void LogAnalyzerDialog::onBrowse()
{
    QString path = QFileDialog::getOpenFileName(
        this,
        "Select Log File",
        m_editFilePath->text(),
        "Log Files (*.log *.txt);;All Files (*)"
    );
    if (!path.isEmpty())
        m_editFilePath->setText(path);
}

void LogAnalyzerDialog::onAnalyze()
{
    QString filePath = m_editFilePath->text().trimmed();
    if (filePath.isEmpty()) {
        QMessageBox::warning(this, "Log Analyzer", "Please select a log file first.");
        return;
    }

    loadLogFile(filePath);
}

void LogAnalyzerDialog::onFilterChanged()
{
    applyFilters();
}

void LogAnalyzerDialog::onSearchTextChanged(const QString& /*text*/)
{
    applyFilters();
}

void LogAnalyzerDialog::onTableSelectionChanged()
{
    auto selected = m_tableEntries->selectedItems();
    if (selected.isEmpty())
        return;

    int row = selected.first()->row();
    auto* msgItem = m_tableEntries->item(row, 3);
    if (msgItem) {
        // Highlight the corresponding line in the raw log view
        QString msg = msgItem->text();
        auto doc = m_logView->document();
        auto cursor = doc->find(msg);
        if (!cursor.isNull()) {
            m_logView->setTextCursor(cursor);
            m_logView->centerCursor();
        }
    }
}

// ---------------------------------------------------------------------------
// Log loading and parsing
// ---------------------------------------------------------------------------
void LogAnalyzerDialog::loadLogFile(const QString& filePath)
{
    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        QMessageBox::critical(this, "Log Analyzer",
                              QString("Cannot open file:\n%1").arg(filePath));
        return;
    }

    QTextStream in(&file);
    m_rawContent = in.readAll();
    file.close();

    m_logView->setPlainText(m_rawContent);

    parseLogContent(m_rawContent);
    applyFilters();
    updateSummary();
}

void LogAnalyzerDialog::parseLogContent(const QString& content)
{
    m_allEntries.clear();

    // Pattern 1 -- Papyrus-style: [MM/DD/YYYY - HH:MM:SS.mmm] level: message
    static const QRegularExpression rxPapyrus(
        R"(^\[(\d{2}/\d{2}/\d{4}\s*-\s*\d{2}:\d{2}:\d{2}\.\d{3})\]\s*(error|warning|info):\s*(.*)$)",
        QRegularExpression::CaseInsensitiveOption);

    // Pattern 2 -- Generic timestamped: YYYY-MM-DD HH:MM:SS [LEVEL] source - message
    static const QRegularExpression rxGeneric(
        R"(^(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\s+\[(\w+)\]\s+(\S+)\s*[-:]\s*(.*)$)");

    // Pattern 3 -- Simple level prefix: [ERROR] message  or  ERROR: message
    static const QRegularExpression rxSimple(
        R"(^\[?(ERROR|WARNING|WARN|INFO)\]?:?\s*(.*)$)",
        QRegularExpression::CaseInsensitiveOption);

    const QStringList lines = content.split('\n');
    int lineNum = 0;

    for (const QString& rawLine : lines) {
        ++lineNum;
        QString line = rawLine.trimmed();
        if (line.isEmpty())
            continue;

        LogEntry entry;
        entry.lineNumber = lineNum;

        // Try Papyrus format first (matches the Delphi original's log type)
        auto m = rxPapyrus.match(line);
        if (m.hasMatch()) {
            entry.timestamp = m.captured(1);
            entry.level     = normaliseLevel(m.captured(2));
            entry.source    = extractPapyrusSource(m.captured(3));
            entry.message   = m.captured(3);
            m_allEntries.push_back(std::move(entry));
            continue;
        }

        // Try generic timestamped format
        m = rxGeneric.match(line);
        if (m.hasMatch()) {
            entry.timestamp = m.captured(1);
            entry.level     = normaliseLevel(m.captured(2));
            entry.source    = m.captured(3);
            entry.message   = m.captured(4);
            m_allEntries.push_back(std::move(entry));
            continue;
        }

        // Try simple level-prefix format
        m = rxSimple.match(line);
        if (m.hasMatch()) {
            entry.timestamp = QString();
            entry.level     = normaliseLevel(m.captured(1));
            entry.source    = QString();
            entry.message   = m.captured(2);
            m_allEntries.push_back(std::move(entry));
            continue;
        }

        // Unrecognised lines are stored as Info with the full text
        entry.level   = "Info";
        entry.message = line;
        m_allEntries.push_back(std::move(entry));
    }
}

// ---------------------------------------------------------------------------
// Static helpers
// ---------------------------------------------------------------------------
QString LogAnalyzerDialog::normaliseLevel(const QString& raw)
{
    QString lower = raw.toLower().trimmed();
    if (lower == "error")
        return QStringLiteral("Error");
    if (lower == "warning" || lower == "warn")
        return QStringLiteral("Warning");
    return QStringLiteral("Info");
}

QString LogAnalyzerDialog::extractPapyrusSource(const QString& message)
{
    // Papyrus messages often contain a script name in brackets, e.g.
    //   "error: (scriptname (0x12345678)).Method() ..."
    // or reference a script directly:  "scriptname.psc(42,7): ..."
    static const QRegularExpression rxScript(
        R"((\w+\.psc)\b|script\s+(\w+)|\((\w+)\s+\(0x)",
        QRegularExpression::CaseInsensitiveOption);

    auto m = rxScript.match(message);
    if (m.hasMatch()) {
        // Return whichever capturing group matched
        for (int i = 1; i <= 3; ++i) {
            if (!m.captured(i).isEmpty())
                return m.captured(i);
        }
    }
    return {};
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------
void LogAnalyzerDialog::applyFilters()
{
    QString levelFilter = m_comboLogLevel->currentText();
    QString searchText  = m_editSearch->text().trimmed();

    std::vector<const LogEntry*> filtered;
    filtered.reserve(m_allEntries.size());

    for (const auto& entry : m_allEntries) {
        // Level filter
        if (levelFilter != "All" && entry.level != levelFilter)
            continue;

        // Text search (case-insensitive)
        if (!searchText.isEmpty()) {
            bool found = entry.message.contains(searchText, Qt::CaseInsensitive)
                      || entry.source.contains(searchText, Qt::CaseInsensitive)
                      || entry.timestamp.contains(searchText, Qt::CaseInsensitive);
            if (!found)
                continue;
        }

        filtered.push_back(&entry);
    }

    populateTable(filtered);
}

void LogAnalyzerDialog::populateTable(const std::vector<const LogEntry*>& entries)
{
    m_tableEntries->setSortingEnabled(false);
    m_tableEntries->setRowCount(0);
    m_tableEntries->setRowCount(static_cast<int>(entries.size()));

    for (int i = 0; i < static_cast<int>(entries.size()); ++i) {
        const auto* e = entries[static_cast<size_t>(i)];

        auto* tsItem  = new QTableWidgetItem(e->timestamp);
        auto* lvlItem = new QTableWidgetItem(e->level);
        auto* srcItem = new QTableWidgetItem(e->source);
        auto* msgItem = new QTableWidgetItem(e->message);

        // Colour-code the level column
        if (e->level == "Error")
            lvlItem->setForeground(Qt::red);
        else if (e->level == "Warning")
            lvlItem->setForeground(QColor(0xCC, 0x88, 0x00)); // dark orange
        else
            lvlItem->setForeground(Qt::darkBlue);

        m_tableEntries->setItem(i, 0, tsItem);
        m_tableEntries->setItem(i, 1, lvlItem);
        m_tableEntries->setItem(i, 2, srcItem);
        m_tableEntries->setItem(i, 3, msgItem);
    }

    m_tableEntries->setSortingEnabled(true);
}

void LogAnalyzerDialog::updateSummary()
{
    int errors   = 0;
    int warnings = 0;
    int infos    = 0;

    for (const auto& e : m_allEntries) {
        if (e.level == "Error")        ++errors;
        else if (e.level == "Warning") ++warnings;
        else                           ++infos;
    }

    m_lblTotalEntries->setText(QString("Total: %1").arg(m_allEntries.size()));
    m_lblErrorCount->setText(QString("Errors: %1").arg(errors));
    m_lblWarningCount->setText(QString("Warnings: %1").arg(warnings));
    m_lblInfoCount->setText(QString("Info: %1").arg(infos));
}
