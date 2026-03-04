#include "ModGroupFile.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QTextStream>

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

bool ModGroupFile::load(const QString& filePath, QVector<PluginEntry>& outEntries,
                        QStringList* outComments)
{
    outEntries.clear();
    if (outComments)
        outComments->clear();

    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text))
        return false;

    QTextStream in(&file);
    while (!in.atEnd()) {
        QString line = in.readLine().trimmed();

        // Empty lines
        if (line.isEmpty())
            continue;

        // Comment lines
        if (line.startsWith(';')) {
            if (outComments)
                outComments->append(line);
            continue;
        }

        // Plugin entry
        PluginEntry entry;
        if (line.startsWith('-')) {
            entry.optional = true;
            entry.name = line.mid(1).trimmed();
        } else {
            entry.optional = false;
            entry.name = line;
        }

        if (!entry.name.isEmpty())
            outEntries.append(entry);
    }

    return true;
}

// ---------------------------------------------------------------------------
// Save
// ---------------------------------------------------------------------------

bool ModGroupFile::save(const QString& filePath, const QVector<PluginEntry>& entries,
                        const QStringList& headerComments)
{
    QFile file(filePath);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text | QIODevice::Truncate))
        return false;

    QTextStream out(&file);

    // Write header comments
    for (const QString& comment : headerComments) {
        if (!comment.startsWith(';'))
            out << "; ";
        out << comment << '\n';
    }

    // Write plugin entries
    for (const auto& entry : entries) {
        if (entry.optional)
            out << '-';
        out << entry.name << '\n';
    }

    return true;
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

QStringList ModGroupFile::toStringList(const QVector<PluginEntry>& entries)
{
    QStringList result;
    result.reserve(entries.size());
    for (const auto& e : entries) {
        if (e.optional)
            result.append(QStringLiteral("-%1").arg(e.name));
        else
            result.append(e.name);
    }
    return result;
}

QVector<ModGroupFile::PluginEntry> ModGroupFile::fromStringList(const QStringList& list)
{
    QVector<PluginEntry> result;
    result.reserve(list.size());
    for (const QString& s : list) {
        PluginEntry entry;
        if (s.startsWith('-')) {
            entry.optional = true;
            entry.name = s.mid(1).trimmed();
        } else {
            entry.optional = false;
            entry.name = s.trimmed();
        }
        if (!entry.name.isEmpty())
            result.append(entry);
    }
    return result;
}

// ---------------------------------------------------------------------------
// Directory scanning
// ---------------------------------------------------------------------------

QStringList ModGroupFile::scanDirectory(const QString& dirPath)
{
    QStringList result;
    QDir dir(dirPath);

    if (!dir.exists())
        return result;

    QStringList filters;
    filters << "*.modgroups";

    QFileInfoList files = dir.entryInfoList(filters, QDir::Files | QDir::Readable,
                                            QDir::Name | QDir::IgnoreCase);
    for (const QFileInfo& fi : files) {
        result.append(fi.completeBaseName());
    }

    return result;
}
