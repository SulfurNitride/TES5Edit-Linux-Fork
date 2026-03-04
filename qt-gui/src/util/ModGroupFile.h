#ifndef MODGROUPFILE_H
#define MODGROUPFILE_H

#include <QString>
#include <QStringList>
#include <QVector>

/// Parser and writer for xEdit .modgroups files.
///
/// Format:
///   - Lines starting with ';' are comments (preserved on save)
///   - Empty lines are skipped
///   - Each non-comment line is a plugin name
///   - A '-' prefix marks the plugin as optional
///
class ModGroupFile {
public:
    struct PluginEntry {
        QString name;      // plugin filename without prefix
        bool    optional;  // true if prefixed with '-'
    };

    /// Parse a .modgroups file. Returns true on success.
    static bool load(const QString& filePath, QVector<PluginEntry>& outEntries,
                     QStringList* outComments = nullptr);

    /// Save a .modgroups file.
    static bool save(const QString& filePath, const QVector<PluginEntry>& entries,
                     const QStringList& headerComments = {});

    /// Convert entries to a flat QStringList (with '-' prefix for optional).
    /// Suitable for display in ModGroupEditDialog.
    static QStringList toStringList(const QVector<PluginEntry>& entries);

    /// Convert a flat QStringList back to entries (recognizes '-' prefix).
    static QVector<PluginEntry> fromStringList(const QStringList& list);

    /// Scan a directory for *.modgroups files, returning just the base names
    /// (without extension).
    static QStringList scanDirectory(const QString& dirPath);
};

#endif // MODGROUPFILE_H
