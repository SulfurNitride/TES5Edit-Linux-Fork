#pragma once

#include <QColor>
#include <QFont>
#include <QSettings>
#include <QString>
#include <QVariant>

#include "forms/OptionsDialog.h"

class SettingsManager {
public:
    static void save(const OptionsDialog& dlg)
    {
        QSettings s = settings();

        // General
        s.setValue("general/hideUnused",         dlg.hideUnused());
        s.setValue("general/hideIgnored",        dlg.hideIgnored());
        s.setValue("general/loadBSAs",           dlg.loadBSAs());
        s.setValue("general/simpleRecords",      dlg.simpleRecords());
        s.setValue("general/showGroupRecordCount", dlg.showGroupRecordCount());

        // UI Theme
        const char* themeStr = "system";
        switch (dlg.selectedTheme()) {
        case OptionsDialog::Light: themeStr = "light"; break;
        case OptionsDialog::Dark:  themeStr = "dark";  break;
        default: break;
        }
        s.setValue("ui/theme", QString(themeStr));

        // Font
        s.setValue("ui/font",     dlg.selectedFont().family());
        s.setValue("ui/fontSize", dlg.selectedFont().pointSize());

        // Cleaning
        s.setValue("cleaning/udrMovedRef",       dlg.udrDetectMovedRef());
        s.setValue("cleaning/udrDeletedRef",     dlg.udrDetectDeletedRef());
        s.setValue("cleaning/udrDeletedNavmesh", dlg.udrDetectDeletedNavmesh());

        // Conflict colors
        static const QStringList colorKeys = {
            "Not Defined", "Benign Conflict", "Override",
            "Conflict", "Critical Conflict"
        };
        for (const QString& key : colorKeys) {
            s.setValue("colors/conflictAll/"  + key, dlg.conflictAllColor(key).name());
            s.setValue("colors/conflictThis/" + key, dlg.conflictThisColor(key).name());
        }

        s.sync();
    }

    static void load(OptionsDialog& dlg)
    {
        QSettings s = settings();

        // General
        if (s.contains("general/hideUnused"))
            dlg.setHideUnused(s.value("general/hideUnused").toBool());
        if (s.contains("general/hideIgnored"))
            dlg.setHideIgnored(s.value("general/hideIgnored").toBool());
        if (s.contains("general/loadBSAs"))
            dlg.setLoadBSAs(s.value("general/loadBSAs").toBool());
        if (s.contains("general/simpleRecords"))
            dlg.setSimpleRecords(s.value("general/simpleRecords").toBool());
        if (s.contains("general/showGroupRecordCount"))
            dlg.setShowGroupRecordCount(s.value("general/showGroupRecordCount").toBool());

        // UI Theme
        if (s.contains("ui/theme")) {
            QString theme = s.value("ui/theme").toString();
            if (theme == "light")      dlg.setTheme(OptionsDialog::Light);
            else if (theme == "dark")  dlg.setTheme(OptionsDialog::Dark);
            else                       dlg.setTheme(OptionsDialog::System);
        }

        // Font
        if (s.contains("ui/font") && s.contains("ui/fontSize")) {
            QFont f(s.value("ui/font").toString(),
                    s.value("ui/fontSize").toInt());
            dlg.setSelectedFont(f);
        }

        // Cleaning
        if (s.contains("cleaning/udrMovedRef"))
            dlg.setUdrDetectMovedRef(s.value("cleaning/udrMovedRef").toBool());
        if (s.contains("cleaning/udrDeletedRef"))
            dlg.setUdrDetectDeletedRef(s.value("cleaning/udrDeletedRef").toBool());
        if (s.contains("cleaning/udrDeletedNavmesh"))
            dlg.setUdrDetectDeletedNavmesh(s.value("cleaning/udrDeletedNavmesh").toBool());

        // Conflict colors
        static const QStringList colorKeys = {
            "Not Defined", "Benign Conflict", "Override",
            "Conflict", "Critical Conflict"
        };
        for (const QString& key : colorKeys) {
            QString allKey  = "colors/conflictAll/"  + key;
            QString thisKey = "colors/conflictThis/" + key;
            if (s.contains(allKey))
                dlg.setConflictAllColor(key, QColor(s.value(allKey).toString()));
            if (s.contains(thisKey))
                dlg.setConflictThisColor(key, QColor(s.value(thisKey).toString()));
        }
    }

    static QVariant value(const QString& key, const QVariant& defaultValue = {})
    {
        return settings().value(key, defaultValue);
    }

    static void setValue(const QString& key, const QVariant& val)
    {
        QSettings s = settings();
        s.setValue(key, val);
        s.sync();
    }

    /// Apply the persisted theme to QApplication without needing an OptionsDialog.
    static void applyThemeFromSettings()
    {
        QString theme = settings().value("ui/theme", "system").toString();
        if (theme == "dark") {
            qApp->setStyleSheet(
                "QWidget { background-color: #2b2b2b; color: #dcdcdc; }"
                "QTreeView, QTableView, QPlainTextEdit, QTextBrowser, QLineEdit, QComboBox "
                "{ background-color: #1e1e1e; color: #dcdcdc; }"
                "QMenuBar, QMenu { background-color: #323232; color: #dcdcdc; }"
                "QMenu::item:selected { background-color: #505050; }"
                "QToolBar { background-color: #323232; }"
                "QHeaderView::section { background-color: #3c3c3c; color: #dcdcdc; }"
                "QTabWidget::pane { border: 1px solid #555; }"
                "QTabBar::tab { background: #3c3c3c; color: #dcdcdc; padding: 6px 12px; }"
                "QTabBar::tab:selected { background: #505050; }"
            );
        } else if (theme == "light") {
            qApp->setStyleSheet(
                "QWidget { background-color: #f5f5f5; color: #1e1e1e; }"
                "QTreeView, QTableView, QPlainTextEdit, QTextBrowser, QLineEdit, QComboBox "
                "{ background-color: #ffffff; color: #1e1e1e; }"
            );
        } else {
            qApp->setStyleSheet(QString());
        }
    }

    /// Apply the persisted font to QApplication without needing an OptionsDialog.
    static void applyFontFromSettings()
    {
        QSettings s = settings();
        if (s.contains("ui/font") && s.contains("ui/fontSize")) {
            QFont f(s.value("ui/font").toString(),
                    s.value("ui/fontSize").toInt());
            qApp->setFont(f);
        }
    }

private:
    static QSettings settings()
    {
        return QSettings("xEdit", "xEdit Linux");
    }
};
