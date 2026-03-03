#include "cache/RecordCache.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"

#include <QSqlError>
#include <QSqlQuery>
#include <QDebug>

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

RecordCache& RecordCache::instance()
{
    static RecordCache s;
    return s;
}

// ---------------------------------------------------------------------------
// Construction / destruction
// ---------------------------------------------------------------------------

RecordCache::RecordCache()
{
    m_db = QSqlDatabase::addDatabase(QStringLiteral("QSQLITE"), kConnectionName);
    m_db.setDatabaseName(QStringLiteral(":memory:"));
    if (!m_db.open()) {
        qWarning() << "RecordCache: failed to open in-memory SQLite database:"
                    << m_db.lastError().text();
    }
}

RecordCache::~RecordCache()
{
    if (m_workerThread) {
        m_workerThread->quit();
        m_workerThread->wait();
    }
    m_db.close();
    QSqlDatabase::removeDatabase(kConnectionName);
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

void RecordCache::createSchema()
{
    QSqlQuery q(m_db);

    q.exec(QStringLiteral(
        "CREATE TABLE IF NOT EXISTS records ("
        "  plugin_idx  INTEGER,"
        "  group_idx   INTEGER,"
        "  record_idx  INTEGER,"
        "  form_id     INTEGER,"
        "  editor_id   TEXT,"
        "  signature   TEXT,"
        "  PRIMARY KEY (plugin_idx, group_idx, record_idx)"
        ")"));

    q.exec(QStringLiteral(
        "CREATE TABLE IF NOT EXISTS subrecords ("
        "  plugin_idx  INTEGER,"
        "  group_idx   INTEGER,"
        "  record_idx  INTEGER,"
        "  sub_idx     INTEGER,"
        "  signature   TEXT,"
        "  size        INTEGER,"
        "  data        BLOB,"
        "  PRIMARY KEY (plugin_idx, group_idx, record_idx, sub_idx)"
        ")"));

    q.exec(QStringLiteral(
        "CREATE TABLE IF NOT EXISTS refby ("
        "  src_plugin  INTEGER,"
        "  src_group   INTEGER,"
        "  src_record  INTEGER,"
        "  ref_plugin  INTEGER,"
        "  ref_group   INTEGER,"
        "  ref_record  INTEGER"
        ")"));

    q.exec(QStringLiteral(
        "CREATE INDEX IF NOT EXISTS idx_refby_src "
        "ON refby(src_plugin, src_group, src_record)"));
}

// ---------------------------------------------------------------------------
// Populate records & subrecords
// ---------------------------------------------------------------------------

void RecordCache::populateRecords()
{
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_plugin_count)
        return;

    const int pluginCount = ffi.xedit_plugin_count();
    if (pluginCount <= 0)
        return;

    // --- Count total records for progress reporting ---
    int64_t totalRecords = 0;
    QVector<int> groupCounts(pluginCount, 0);
    QVector<QVector<int>> recordCounts(pluginCount);

    for (int p = 0; p < pluginCount; ++p) {
        int gc = ffi.xedit_plugin_group_count ? ffi.xedit_plugin_group_count(p) : 0;
        if (gc < 0) gc = 0;
        groupCounts[p] = gc;
        recordCounts[p].resize(gc);
        for (int g = 0; g < gc; ++g) {
            int rc = ffi.xedit_group_record_count ? ffi.xedit_group_record_count(p, g) : 0;
            if (rc < 0) rc = 0;
            recordCounts[p][g] = rc;
            totalRecords += rc;
        }
    }

    // --- Prepared insert statements ---
    QSqlQuery insRec(m_db);
    insRec.prepare(QStringLiteral(
        "INSERT INTO records (plugin_idx, group_idx, record_idx, form_id, editor_id, signature) "
        "VALUES (?, ?, ?, ?, ?, ?)"));

    QSqlQuery insSub(m_db);
    insSub.prepare(QStringLiteral(
        "INSERT INTO subrecords (plugin_idx, group_idx, record_idx, sub_idx, signature, size, data) "
        "VALUES (?, ?, ?, ?, ?, ?, ?)"));

    m_db.transaction();

    int64_t processed = 0;
    const int64_t progressInterval = qMax<int64_t>(1, totalRecords / 200);

    for (int p = 0; p < pluginCount; ++p) {
        const int gc = groupCounts[p];
        for (int g = 0; g < gc; ++g) {
            const int rc = recordCounts[p][g];
            for (int r = 0; r < rc; ++r) {
                // --- Record ---
                uint32_t formId = ffi.xedit_record_form_id
                    ? ffi.xedit_record_form_id(p, g, r) : 0;

                QString editorId = ffi.xedit_record_editor_id
                    ? ffiString([&](char* buf, int32_t len) {
                          return ffi.xedit_record_editor_id(p, g, r, buf, len);
                      })
                    : QString();

                QString sig = ffi.xedit_record_signature
                    ? ffiString([&](char* buf, int32_t len) {
                          return ffi.xedit_record_signature(p, g, r, buf, len);
                      })
                    : QString();

                insRec.addBindValue(p);
                insRec.addBindValue(g);
                insRec.addBindValue(r);
                insRec.addBindValue(static_cast<qint64>(formId));
                insRec.addBindValue(editorId);
                insRec.addBindValue(sig);
                insRec.exec();

                // --- Subrecords ---
                int subCount = ffi.xedit_record_subrecord_count
                    ? ffi.xedit_record_subrecord_count(p, g, r) : 0;
                if (subCount < 0) subCount = 0;

                for (int s = 0; s < subCount; ++s) {
                    QString subSig = ffi.xedit_subrecord_signature
                        ? ffiString([&](char* buf, int32_t len) {
                              return ffi.xedit_subrecord_signature(p, g, r, s, buf, len);
                          })
                        : QString();

                    int32_t subSize = ffi.xedit_subrecord_size
                        ? ffi.xedit_subrecord_size(p, g, r, s) : 0;

                    QByteArray subData;
                    if (ffi.xedit_subrecord_data && subSize > 0) {
                        const int bytesToRead = qMin(subSize, static_cast<int32_t>(kMaxSubrecordDataBytes));
                        subData.resize(bytesToRead);
                        int32_t got = ffi.xedit_subrecord_data(
                            p, g, r, s,
                            subData.data(),
                            static_cast<int32_t>(bytesToRead));
                        if (got > 0)
                            subData.resize(got);
                        else
                            subData.clear();
                    }

                    insSub.addBindValue(p);
                    insSub.addBindValue(g);
                    insSub.addBindValue(r);
                    insSub.addBindValue(s);
                    insSub.addBindValue(subSig);
                    insSub.addBindValue(subSize);
                    insSub.addBindValue(subData);
                    insSub.exec();
                }

                ++processed;
                if (processed % progressInterval == 0) {
                    double frac = static_cast<double>(processed) / static_cast<double>(totalRecords);
                    emit buildProgress(
                        QStringLiteral("Caching records... %1/%2")
                            .arg(processed).arg(totalRecords),
                        frac * 0.8);  // records phase = 0..80%
                }
            }
        }
    }

    m_db.commit();
}

// ---------------------------------------------------------------------------
// Populate refby
// ---------------------------------------------------------------------------

void RecordCache::populateRefBy()
{
    auto& ffi = XEditFFI::instance();
    if (!ffi.xedit_record_refby_count || !ffi.xedit_record_refby_entry)
        return;

    // We iterate all cached records and query their refby entries.
    QSqlQuery sel(m_db);
    sel.exec(QStringLiteral("SELECT plugin_idx, group_idx, record_idx FROM records"));

    struct Idx { int p, g, r; };
    QVector<Idx> allRecords;
    while (sel.next())
        allRecords.append({sel.value(0).toInt(), sel.value(1).toInt(), sel.value(2).toInt()});

    QSqlQuery ins(m_db);
    ins.prepare(QStringLiteral(
        "INSERT INTO refby (src_plugin, src_group, src_record, ref_plugin, ref_group, ref_record) "
        "VALUES (?, ?, ?, ?, ?, ?)"));

    m_db.transaction();

    const int total = allRecords.size();
    const int progressInterval = qMax(1, total / 100);

    for (int i = 0; i < total; ++i) {
        const auto& idx = allRecords[i];

        int refCount = ffi.xedit_record_refby_count(idx.p, idx.g, idx.r);
        if (refCount <= 0)
            continue;

        for (int ri = 0; ri < refCount; ++ri) {
            int32_t refP = 0, refG = 0, refR = 0;
            int32_t ok = ffi.xedit_record_refby_entry(
                idx.p, idx.g, idx.r, ri, &refP, &refG, &refR);
            if (ok < 0)
                continue;

            ins.addBindValue(idx.p);
            ins.addBindValue(idx.g);
            ins.addBindValue(idx.r);
            ins.addBindValue(refP);
            ins.addBindValue(refG);
            ins.addBindValue(refR);
            ins.exec();
        }

        if (i % progressInterval == 0) {
            double frac = 0.8 + 0.2 * (static_cast<double>(i) / static_cast<double>(total));
            emit buildProgress(
                QStringLiteral("Caching referenced-by... %1/%2").arg(i).arg(total),
                frac);
        }
    }

    m_db.commit();
}

// ---------------------------------------------------------------------------
// buildCache / buildCacheAsync
// ---------------------------------------------------------------------------

void RecordCache::buildCache()
{
    m_ready.store(false);

    emit buildProgress(QStringLiteral("Creating cache schema..."), 0.0);
    createSchema();

    emit buildProgress(QStringLiteral("Caching records..."), 0.0);
    populateRecords();

    emit buildProgress(QStringLiteral("Caching referenced-by data..."), 0.8);
    populateRefBy();

    m_ready.store(true);
    emit buildProgress(QStringLiteral("Cache ready."), 1.0);
    emit cacheReady();
}

void RecordCache::buildCacheAsync()
{
    if (m_workerThread) {
        // Already running or completed -- ignore duplicate calls.
        if (m_workerThread->isRunning())
            return;
        delete m_workerThread;
        m_workerThread = nullptr;
    }

    m_workerThread = QThread::create([this]() { buildCache(); });
    m_workerThread->setObjectName(QStringLiteral("RecordCacheBuilder"));
    m_workerThread->start();
}

// ---------------------------------------------------------------------------
// Query helpers
// ---------------------------------------------------------------------------

CachedRecord RecordCache::getRecord(int pluginIdx, int groupIdx, int recordIdx) const
{
    CachedRecord result;
    if (!m_ready.load())
        return result;

    QSqlQuery q(m_db);
    q.prepare(QStringLiteral(
        "SELECT form_id, editor_id, signature FROM records "
        "WHERE plugin_idx = ? AND group_idx = ? AND record_idx = ?"));
    q.addBindValue(pluginIdx);
    q.addBindValue(groupIdx);
    q.addBindValue(recordIdx);

    if (q.exec() && q.next()) {
        result.pluginIdx = pluginIdx;
        result.groupIdx  = groupIdx;
        result.recordIdx = recordIdx;
        result.formId    = static_cast<uint32_t>(q.value(0).toLongLong());
        result.editorId  = q.value(1).toString();
        result.signature = q.value(2).toString();
    }
    return result;
}

QVector<CachedSubrecord> RecordCache::getSubrecords(int pluginIdx, int groupIdx, int recordIdx) const
{
    QVector<CachedSubrecord> results;
    if (!m_ready.load())
        return results;

    QSqlQuery q(m_db);
    q.prepare(QStringLiteral(
        "SELECT signature, size, data FROM subrecords "
        "WHERE plugin_idx = ? AND group_idx = ? AND record_idx = ? "
        "ORDER BY sub_idx"));
    q.addBindValue(pluginIdx);
    q.addBindValue(groupIdx);
    q.addBindValue(recordIdx);

    if (q.exec()) {
        while (q.next()) {
            CachedSubrecord sub;
            sub.signature = q.value(0).toString();
            sub.size      = q.value(1).toInt();
            sub.data      = q.value(2).toByteArray();
            results.append(std::move(sub));
        }
    }
    return results;
}

QVector<CachedRefBy> RecordCache::getReferencedBy(int pluginIdx, int groupIdx, int recordIdx) const
{
    QVector<CachedRefBy> results;
    if (!m_ready.load())
        return results;

    QSqlQuery q(m_db);
    q.prepare(QStringLiteral(
        "SELECT ref_plugin, ref_group, ref_record FROM refby "
        "WHERE src_plugin = ? AND src_group = ? AND src_record = ?"));
    q.addBindValue(pluginIdx);
    q.addBindValue(groupIdx);
    q.addBindValue(recordIdx);

    if (q.exec()) {
        while (q.next()) {
            CachedRefBy ref;
            ref.pluginIdx  = q.value(0).toInt();
            ref.groupIdx   = q.value(1).toInt();
            ref.recordIdx  = q.value(2).toInt();
            results.append(ref);
        }
    }
    return results;
}

bool RecordCache::isReady() const
{
    return m_ready.load();
}
