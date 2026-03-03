#pragma once

#include <QObject>
#include <QSqlDatabase>
#include <QSqlQuery>
#include <QString>
#include <QByteArray>
#include <QVector>
#include <QThread>
#include <atomic>
#include <cstdint>

// ---------------------------------------------------------------------------
// Cached data structures
// ---------------------------------------------------------------------------

struct CachedRecord {
    int pluginIdx = -1;
    int groupIdx  = -1;
    int recordIdx = -1;
    uint32_t formId = 0;
    QString editorId;
    QString signature;
};

struct CachedSubrecord {
    QString signature;
    int32_t size = 0;
    QByteArray data;
};

struct CachedRefBy {
    int pluginIdx  = -1;
    int groupIdx   = -1;
    int recordIdx  = -1;
};

// ---------------------------------------------------------------------------
// RecordCache -- in-memory SQLite cache for FFI record data
// ---------------------------------------------------------------------------

class RecordCache : public QObject
{
    Q_OBJECT

public:
    static RecordCache& instance();

    /// Build the cache synchronously (call from a worker thread).
    void buildCache();

    /// Build the cache on a background QThread; emits cacheReady() when done.
    void buildCacheAsync();

    /// Fast queries -- all use prepared statements on the in-memory DB.
    CachedRecord               getRecord(int pluginIdx, int groupIdx, int recordIdx) const;
    QVector<CachedSubrecord>   getSubrecords(int pluginIdx, int groupIdx, int recordIdx) const;
    QVector<CachedRefBy>       getReferencedBy(int pluginIdx, int groupIdx, int recordIdx) const;

    /// Returns true once the cache has been fully built.
    bool isReady() const;

signals:
    /// Emitted periodically during buildCache() with a human-readable message
    /// and a fraction in [0.0, 1.0].
    void buildProgress(const QString& message, double fraction);

    /// Emitted (from the worker thread) when the cache is fully populated.
    void cacheReady();

private:
    RecordCache();
    ~RecordCache() override;

    RecordCache(const RecordCache&) = delete;
    RecordCache& operator=(const RecordCache&) = delete;

    /// Create the schema (tables + indices).
    void createSchema();

    /// Populate records & subrecords tables.
    void populateRecords();

    /// Populate the refby table.
    void populateRefBy();

    /// Maximum bytes of subrecord data to cache per subrecord.
    static constexpr int kMaxSubrecordDataBytes = 256;

    /// Named connection used for all queries.
    static constexpr const char* kConnectionName = "xedit_cache";

    QSqlDatabase m_db;
    std::atomic<bool> m_ready{false};

    QThread* m_workerThread = nullptr;
};
