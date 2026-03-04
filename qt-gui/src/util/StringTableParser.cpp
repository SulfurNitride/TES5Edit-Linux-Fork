#include "StringTableParser.h"

#include <QFile>
#include <cstring>

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

quint32 StringTableParser::readU32(const char* data)
{
    // Little-endian read (Bethesda format is always LE)
    quint32 v = 0;
    std::memcpy(&v, data, 4);
    return v;
}

void StringTableParser::writeU32(QByteArray& buf, quint32 value)
{
    const char* p = reinterpret_cast<const char*>(&value);
    buf.append(p, 4);
}

// ---------------------------------------------------------------------------
// Parse
// ---------------------------------------------------------------------------

bool StringTableParser::parse(const QByteArray& data, StringType type,
                              QVector<Entry>& outEntries)
{
    outEntries.clear();

    // Need at least 8 bytes for header
    if (data.size() < 8)
        return false;

    const char* raw = data.constData();
    const quint32 count    = readU32(raw);
    const quint32 dataSize = readU32(raw + 4);

    // Validate sizes
    const qint64 headerSize    = 8;
    const qint64 directorySize = static_cast<qint64>(count) * 8;
    const qint64 expectedMin   = headerSize + directorySize;

    if (data.size() < expectedMin)
        return false;

    // The data section starts right after the directory
    const qint64 dataSectionOffset = expectedMin;

    // Sanity check: dataSectionOffset + dataSize should not exceed file size
    if (dataSectionOffset + dataSize > static_cast<quint64>(data.size()))
        return false;

    const char* dirStart  = raw + headerSize;
    const char* dataStart = raw + dataSectionOffset;

    outEntries.reserve(static_cast<int>(count));

    for (quint32 i = 0; i < count; ++i) {
        const char* dirEntry = dirStart + i * 8;
        quint32 id     = readU32(dirEntry);
        quint32 offset = readU32(dirEntry + 4);

        if (offset >= dataSize)
            continue; // skip invalid offset

        const char* strPtr = dataStart + offset;
        const qint64 remaining = dataSize - offset;
        QString value;

        if (type == StringType::Strings) {
            // Null-terminated string
            qint64 maxLen = remaining;
            qint64 len = 0;
            while (len < maxLen && strPtr[len] != '\0')
                ++len;
            value = QString::fromUtf8(strPtr, static_cast<int>(len));
        } else {
            // DLStrings / ILStrings: uint32 length prefix + data
            if (remaining < 4)
                continue;
            quint32 strLen = readU32(strPtr);
            if (strLen > static_cast<quint32>(remaining) - 4)
                strLen = static_cast<quint32>(remaining) - 4;
            // Strip trailing null if present
            const char* strData = strPtr + 4;
            int actualLen = static_cast<int>(strLen);
            if (actualLen > 0 && strData[actualLen - 1] == '\0')
                --actualLen;
            value = QString::fromUtf8(strData, actualLen);
        }

        outEntries.append({id, value});
    }

    return true;
}

bool StringTableParser::parseFile(const QString& filePath, StringType type,
                                  QVector<Entry>& outEntries)
{
    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly))
        return false;

    QByteArray data = file.readAll();
    return parse(data, type, outEntries);
}

// ---------------------------------------------------------------------------
// Serialize
// ---------------------------------------------------------------------------

QByteArray StringTableParser::serialize(const QVector<Entry>& entries, StringType type)
{
    QByteArray result;
    const quint32 count = static_cast<quint32>(entries.size());

    // Build data section first so we know offsets
    QByteArray dataSection;
    QVector<quint32> offsets;
    offsets.reserve(entries.size());

    for (const auto& entry : entries) {
        offsets.append(static_cast<quint32>(dataSection.size()));
        QByteArray utf8 = entry.value.toUtf8();

        if (type == StringType::Strings) {
            // Null-terminated
            dataSection.append(utf8);
            dataSection.append('\0');
        } else {
            // Length-prefixed (length includes the null terminator)
            quint32 len = static_cast<quint32>(utf8.size()) + 1;
            writeU32(dataSection, len);
            dataSection.append(utf8);
            dataSection.append('\0');
        }
    }

    // Write header
    quint32 dataSize = static_cast<quint32>(dataSection.size());
    writeU32(result, count);
    writeU32(result, dataSize);

    // Write directory
    for (int i = 0; i < entries.size(); ++i) {
        writeU32(result, entries[i].id);
        writeU32(result, offsets[i]);
    }

    // Write data section
    result.append(dataSection);

    return result;
}

bool StringTableParser::writeFile(const QString& filePath,
                                  const QVector<Entry>& entries, StringType type)
{
    QFile file(filePath);
    if (!file.open(QIODevice::WriteOnly))
        return false;

    QByteArray data = serialize(entries, type);
    return file.write(data) == data.size();
}

// ---------------------------------------------------------------------------
// Type helpers
// ---------------------------------------------------------------------------

StringTableParser::StringType StringTableParser::typeFromExtension(const QString& extension)
{
    QString ext = extension.toUpper();
    if (!ext.startsWith('.'))
        ext.prepend('.');

    if (ext == ".DLSTRINGS")
        return StringType::DLStrings;
    if (ext == ".ILSTRINGS")
        return StringType::ILStrings;
    return StringType::Strings; // default
}

QString StringTableParser::extensionForType(StringType type)
{
    switch (type) {
    case StringType::DLStrings: return QStringLiteral(".DLSTRINGS");
    case StringType::ILStrings: return QStringLiteral(".ILSTRINGS");
    default:                    return QStringLiteral(".STRINGS");
    }
}

QString StringTableParser::displayName(StringType type)
{
    switch (type) {
    case StringType::DLStrings: return QStringLiteral("DLSTRINGS");
    case StringType::ILStrings: return QStringLiteral("ILSTRINGS");
    default:                    return QStringLiteral("STRINGS");
    }
}
