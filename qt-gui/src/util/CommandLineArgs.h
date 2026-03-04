#pragma once

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QHash>
#include <QString>
#include <QStringList>

struct CommandLineArgs {
    // Game mode, in xEdit token form (e.g. "sse", "fo4", "tes5vr").
    QString gameMode;

    // Quick modes
    bool quickShowConflicts = false;
    bool veryQuickShowConflicts = false;
    bool quickClean = false;
    bool quickAutoClean = false;

    // Auto
    bool autoLoad = false;
    bool autoExit = false;
    bool autoGameLink = false;

    // Safety
    bool iKnowWhatImDoing = false;
    bool allowMasterFilesEdit = false;
    bool iKnowIllBreakMyGameWithThis = false;
    bool allowMakePartial = false;

    // Paths
    QString dataPath;
    QString outputPath;
    QString scriptsPath;
    QString backupPath;
    QString cachePath;

    // BSA
    bool skipBsa = false;
    bool forceBsa = false;

    // Refs
    bool noBuildRefs = false;

    // Cache
    bool dontCache = false;
    bool dontCacheLoad = false;
    bool dontCacheSave = false;

    // Parse status
    bool valid = true;
    QString error;

    static CommandLineArgs& instance()
    {
        static CommandLineArgs args;
        return args;
    }

    void parse(const QCoreApplication& app)
    {
        reset();

        const QStringList args = app.arguments();
        for (int i = 1; i < args.size(); ++i) {
            const QString raw = args.at(i);
            if (raw.size() < 2)
                continue;
            if (raw[0] != '-' && raw[0] != '/')
                continue;

            const QString body = raw.mid(1);
            if (body.isEmpty())
                continue;

            const int sep = body.indexOf(':');
            const QString key = (sep >= 0 ? body.left(sep) : body).toLower();
            const QString value = (sep >= 0 ? body.mid(sep + 1) : QString());

            if (isGameMode(key)) {
                gameMode = key;
                continue;
            }

            if (key == "qsc" || key == "quickshowconflicts") {
                quickShowConflicts = true;
                continue;
            }
            if (key == "vqsc" || key == "veryquickshowconflicts") {
                quickShowConflicts = true;
                veryQuickShowConflicts = true;
                autoLoad = true;
                continue;
            }
            if (key == "qc" || key == "quickclean") {
                quickClean = true;
                continue;
            }
            if (key == "qac" || key == "quickautoclean") {
                quickClean = true;
                quickAutoClean = true;
                continue;
            }

            if (key == "autoload") {
                autoLoad = true;
                continue;
            }
            if (key == "autoexit") {
                autoExit = true;
                continue;
            }
            if (key == "autogamelink" || key == "agl") {
                autoLoad = true;
                autoGameLink = true;
                continue;
            }

            if (key == "iknowwhatimdoing") {
                iKnowWhatImDoing = true;
                continue;
            }
            if (key == "allowmasterfilesedit") {
                allowMasterFilesEdit = true;
                continue;
            }
            if (key == "iknowillbreakmygamewiththis") {
                iKnowIllBreakMyGameWithThis = true;
                continue;
            }
            if (key == "allowmakepartial") {
                allowMakePartial = true;
                continue;
            }

            if (key.size() == 1 && sep >= 0) {
                const QChar c = key[0].toUpper();
                if (c == 'D') {
                    dataPath = value;
                    continue;
                }
                if (c == 'O') {
                    outputPath = value;
                    continue;
                }
                if (c == 'S') {
                    scriptsPath = value;
                    continue;
                }
                if (c == 'B') {
                    backupPath = value;
                    continue;
                }
                if (c == 'C') {
                    cachePath = value;
                    continue;
                }
            }

            if (key == "skipbsa") {
                skipBsa = true;
                continue;
            }
            if (key == "forcebsa") {
                forceBsa = true;
                continue;
            }
            if (key == "nobuildrefs") {
                noBuildRefs = true;
                continue;
            }

            if (key == "dontcache") {
                dontCache = true;
                continue;
            }
            if (key == "dontcacheload") {
                dontCacheLoad = true;
                continue;
            }
            if (key == "dontcachesave") {
                dontCacheSave = true;
                continue;
            }
        }

        if (gameMode.isEmpty())
            gameMode = detectGameModeFromExe(app);

        if (autoLoad && quickShowConflicts)
            veryQuickShowConflicts = true;

        if (skipBsa)
            forceBsa = false;

        if (dontCache)
            dontCacheLoad = dontCacheSave = true;

        if (dontCacheLoad && dontCacheSave)
            dontCache = true;

        int activeQuickModes = 0;
        if (quickShowConflicts)
            ++activeQuickModes;
        if (quickClean)
            ++activeQuickModes;
        if (autoGameLink)
            ++activeQuickModes;

        if (activeQuickModes > 1) {
            valid = false;
            error = "Can't activate more than one out of Quick Clean, Quick Show Conflicts, or Auto GameLink modes same time.";
        }
    }

    static QString toFFIGameName(const QString& mode)
    {
        const QString m = mode.toLower();
        if (m == "sse" || m == "enderalse")
            return "SSE";
        if (m == "tes5" || m == "tes5vr" || m == "enderal")
            return "TES5";
        if (m == "fo4" || m == "fo4vr")
            return "FO4";
        if (m == "fnv")
            return "FNV";
        if (m == "fo3")
            return "FO3";
        if (m == "fo76")
            return "FO76";
        if (m == "sf1")
            return "SF1";
        if (m == "tes4")
            return "TES4";
        if (m == "tes3")
            return "TES3";
        return {};
    }

    /// Detect game mode by scanning a Data directory for known master ESM files.
    /// Returns a game mode token (e.g. "sse", "fo4") or empty string on failure.
    static QString detectGameFromDataContents(const QString& dataPath)
    {
        if (dataPath.isEmpty())
            return {};

        QDir dir(dataPath);
        if (!dir.exists())
            return {};

        // Check for known master files. Order matters: more specific first.
        // Starfield uses a different master extension (.esm) but has a unique name.
        static const struct { const char* masterFile; const char* gameMode; } kMasters[] = {
            { "Starfield.esm",      "sf1"  },
            { "Fallout4.esm",       "fo4"  },
            { "Fallout76.esm",      "fo76" },
            { "FalloutNV.esm",      "fnv"  },
            { "Fallout3.esm",       "fo3"  },
            { "Skyrim.esm",         "sse"  },   // Could be LE or SE; default to SE
            { "Oblivion.esm",       "tes4" },
            { "Morrowind.esm",      "tes3" },
            { "Enderal - Forgotten Stories.esm", "enderalse" },
        };

        for (const auto& m : kMasters) {
            if (dir.exists(QString::fromLatin1(m.masterFile)))
                return QString::fromLatin1(m.gameMode);
        }

        return {};
    }

    /// Try to detect game from MO2's ModOrganizer.ini in a given directory.
    /// Returns a game mode token or empty string on failure.
    static QString detectGameFromMO2Ini(const QString& mo2Dir)
    {
        if (mo2Dir.isEmpty())
            return {};

        // MO2 stores gameName in ModOrganizer.ini under [General]
        // e.g. gameName=Skyrim Special Edition
        QString iniPath = mo2Dir + "/ModOrganizer.ini";
        QFile ini(iniPath);
        if (!ini.open(QIODevice::ReadOnly | QIODevice::Text))
            return {};

        static const QHash<QString, QString> kMO2GameNames = {
            {"skyrim special edition", "sse"},
            {"skyrim vr",              "tes5vr"},
            {"skyrim",                 "tes5"},
            {"fallout 4",              "fo4"},
            {"fallout 4 vr",           "fo4vr"},
            {"fallout 76",             "fo76"},
            {"new vegas",              "fnv"},
            {"fallout nv",             "fnv"},
            {"fallout 3",              "fo3"},
            {"oblivion",               "tes4"},
            {"morrowind",              "tes3"},
            {"starfield",              "sf1"},
            {"enderal",                "enderal"},
            {"enderal special edition","enderalse"},
        };

        while (!ini.atEnd()) {
            const QString line = QString::fromUtf8(ini.readLine()).trimmed();
            if (line.startsWith("gameName=", Qt::CaseInsensitive)) {
                const QString gameName = line.mid(9).trimmed().toLower();
                const QString mode = kMO2GameNames.value(gameName);
                if (!mode.isEmpty())
                    return mode;
                // Fallback: try partial matching
                for (auto it = kMO2GameNames.cbegin(); it != kMO2GameNames.cend(); ++it) {
                    if (gameName.contains(it.key()))
                        return it.value();
                }
                break;
            }
        }
        return {};
    }

private:
    static bool isGameMode(const QString& key)
    {
        return key == "sse" || key == "fo4" || key == "fnv" || key == "tes5"
            || key == "fo3" || key == "fo76" || key == "sf1" || key == "tes4"
            || key == "tes3" || key == "tes5vr" || key == "fo4vr"
            || key == "enderal" || key == "enderalse";
    }

    static QString detectGameModeFromExe(const QCoreApplication& app)
    {
        const QString exeName = QFileInfo(app.applicationFilePath()).completeBaseName().toLower();

        // Keep longer names first to avoid partial substring collisions.
        static const char* kModes[] = {
            "enderalse", "enderal", "tes5vr", "fo4vr",
            "fo76", "tes5", "tes4", "tes3", "sse", "fo4", "fnv", "fo3", "sf1"
        };

        for (const char* mode : kModes) {
            const QString token = QString::fromLatin1(mode);
            if (exeName.contains(token))
                return token;
        }

        return {};
    }

    void reset()
    {
        *this = CommandLineArgs{};
    }
};
