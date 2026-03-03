#include "PascalHighlighter.h"

PascalHighlighter::PascalHighlighter(QTextDocument* parent)
    : QSyntaxHighlighter(parent)
{
    // --- Keyword format: blue, bold ---
    m_keywordFormat.setForeground(Qt::blue);
    m_keywordFormat.setFontWeight(QFont::Bold);

    const QStringList keywords = {
        QStringLiteral("and"),
        QStringLiteral("array"),
        QStringLiteral("as"),
        QStringLiteral("begin"),
        QStringLiteral("case"),
        QStringLiteral("class"),
        QStringLiteral("const"),
        QStringLiteral("constructor"),
        QStringLiteral("destructor"),
        QStringLiteral("div"),
        QStringLiteral("do"),
        QStringLiteral("downto"),
        QStringLiteral("else"),
        QStringLiteral("end"),
        QStringLiteral("except"),
        QStringLiteral("false"),
        QStringLiteral("file"),
        QStringLiteral("finally"),
        QStringLiteral("for"),
        QStringLiteral("function"),
        QStringLiteral("goto"),
        QStringLiteral("if"),
        QStringLiteral("implementation"),
        QStringLiteral("in"),
        QStringLiteral("inherited"),
        QStringLiteral("interface"),
        QStringLiteral("is"),
        QStringLiteral("mod"),
        QStringLiteral("nil"),
        QStringLiteral("not"),
        QStringLiteral("object"),
        QStringLiteral("of"),
        QStringLiteral("on"),
        QStringLiteral("or"),
        QStringLiteral("packed"),
        QStringLiteral("procedure"),
        QStringLiteral("program"),
        QStringLiteral("property"),
        QStringLiteral("raise"),
        QStringLiteral("record"),
        QStringLiteral("repeat"),
        QStringLiteral("set"),
        QStringLiteral("shl"),
        QStringLiteral("shr"),
        QStringLiteral("string"),
        QStringLiteral("then"),
        QStringLiteral("to"),
        QStringLiteral("true"),
        QStringLiteral("try"),
        QStringLiteral("type"),
        QStringLiteral("unit"),
        QStringLiteral("until"),
        QStringLiteral("uses"),
        QStringLiteral("var"),
        QStringLiteral("while"),
        QStringLiteral("with"),
        QStringLiteral("xor"),
    };

    for (const QString& kw : keywords) {
        HighlightingRule rule;
        rule.pattern = QRegularExpression(
            QStringLiteral("\\b%1\\b").arg(kw),
            QRegularExpression::CaseInsensitiveOption);
        rule.format = m_keywordFormat;
        m_rules.push_back(rule);
    }

    // --- Number format: dark magenta ---
    m_numberFormat.setForeground(Qt::darkMagenta);
    {
        HighlightingRule rule;
        // Integer, hex ($xx), and floating-point literals
        rule.pattern = QRegularExpression(QStringLiteral("\\b\\d+(\\.\\d+)?([eE][+-]?\\d+)?\\b|\\$[0-9A-Fa-f]+\\b"));
        rule.format = m_numberFormat;
        m_rules.push_back(rule);
    }

    // --- String format: red (single-quoted) ---
    m_stringFormat.setForeground(Qt::red);
    {
        HighlightingRule rule;
        rule.pattern = QRegularExpression(QStringLiteral("'[^']*'"));
        rule.format = m_stringFormat;
        m_rules.push_back(rule);
    }

    // --- Line comment format: green, italic ---
    m_commentFormat.setForeground(Qt::darkGreen);
    m_commentFormat.setFontItalic(true);
    {
        HighlightingRule rule;
        rule.pattern = QRegularExpression(QStringLiteral("//[^\n]*"));
        rule.format = m_commentFormat;
        m_rules.push_back(rule);
    }
}

void PascalHighlighter::highlightBlock(const QString& text)
{
    // Apply single-line rules first
    for (const auto& rule : m_rules) {
        auto matchIter = rule.pattern.globalMatch(text);
        while (matchIter.hasNext()) {
            auto match = matchIter.next();
            setFormat(match.capturedStart(), match.capturedLength(), rule.format);
        }
    }

    // --- Multi-line block comments: { } and (* *) ---

    // Determine the starting state from the previous block
    int state = previousBlockState();

    int i = 0;
    while (i < text.length()) {
        if (state == InBraceComment) {
            // We are inside a { } comment, search for closing }
            int end = text.indexOf(QLatin1Char('}'), i);
            if (end == -1) {
                // Comment extends to end of block
                setFormat(i, text.length() - i, m_commentFormat);
                setCurrentBlockState(InBraceComment);
                return;
            }
            // Include the closing }
            setFormat(i, end - i + 1, m_commentFormat);
            i = end + 1;
            state = None;
        } else if (state == InParenStarComment) {
            // We are inside a (* *) comment, search for closing *)
            int end = text.indexOf(QStringLiteral("*)"), i);
            if (end == -1) {
                setFormat(i, text.length() - i, m_commentFormat);
                setCurrentBlockState(InParenStarComment);
                return;
            }
            setFormat(i, end - i + 2, m_commentFormat);
            i = end + 2;
            state = None;
        } else {
            // Not in a block comment -- look for the start of one
            int braceStart = text.indexOf(QLatin1Char('{'), i);
            int parenStarStart = text.indexOf(QStringLiteral("(*"), i);

            // Determine which comes first
            if (braceStart == -1 && parenStarStart == -1) {
                // No more block comments in this line
                break;
            }

            if (braceStart != -1 && (parenStarStart == -1 || braceStart < parenStarStart)) {
                // { } comment starts first
                int end = text.indexOf(QLatin1Char('}'), braceStart + 1);
                if (end == -1) {
                    setFormat(braceStart, text.length() - braceStart, m_commentFormat);
                    setCurrentBlockState(InBraceComment);
                    return;
                }
                setFormat(braceStart, end - braceStart + 1, m_commentFormat);
                i = end + 1;
            } else {
                // (* *) comment starts first
                int end = text.indexOf(QStringLiteral("*)"), parenStarStart + 2);
                if (end == -1) {
                    setFormat(parenStarStart, text.length() - parenStarStart, m_commentFormat);
                    setCurrentBlockState(InParenStarComment);
                    return;
                }
                setFormat(parenStarStart, end - parenStarStart + 2, m_commentFormat);
                i = end + 2;
            }
        }
    }

    // If we reach here without setting the state, we are not inside a block comment
    if (currentBlockState() != InBraceComment && currentBlockState() != InParenStarComment)
        setCurrentBlockState(None);
}
