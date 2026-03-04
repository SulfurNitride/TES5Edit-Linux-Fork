#ifndef STRINGTABLEPARSER_H
#define STRINGTABLEPARSER_H

#include <QByteArray>
#include <QMap>
#include <QString>
#include <QVector>

/// Bethesda string table parser (.STRINGS, .DLSTRINGS, .ILSTRINGS).
///
/// Binary format:
///   Header:
///     uint32 count      — number of entries
///     uint32 dataSize   — total size of string data section
///   Directory (count entries):
///     uint32 id         — string ID
///     uint32 offset     — offset into data section
///   Data section:
///     For STRINGS:     null-terminated UTF-8 at each offset
///     For DL/ILSTRINGS: uint32 length + UTF-8 data at each offset
///
class StringTableParser {
public:
    enum class StringType {
        Strings,     // .STRINGS  — null-terminated
        DLStrings,   // .DLSTRINGS — length-prefixed
        ILStrings    // .ILSTRINGS — length-prefixed
    };

    struct Entry {
        quint32 id;
        QString value;
    };

    /// Parse a string table file. Returns true on success.
    static bool parse(const QByteArray& data, StringType type, QVector<Entry>& outEntries);

    /// Parse a string table from a file path.
    static bool parseFile(const QString& filePath, StringType type, QVector<Entry>& outEntries);

    /// Serialize entries back to binary string table format.
    static QByteArray serialize(const QVector<Entry>& entries, StringType type);

    /// Write entries to a file.
    static bool writeFile(const QString& filePath, const QVector<Entry>& entries, StringType type);

    /// Determine StringType from file extension.
    static StringType typeFromExtension(const QString& extension);

    /// Get extension string for a type.
    static QString extensionForType(StringType type);

    /// Get display name for a type (e.g. "STRINGS", "DLSTRINGS").
    static QString displayName(StringType type);

private:
    static quint32 readU32(const char* data);
    static void writeU32(QByteArray& buf, quint32 value);
};

#endif // STRINGTABLEPARSER_H
