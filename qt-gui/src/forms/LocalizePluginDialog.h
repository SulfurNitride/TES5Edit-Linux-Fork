#pragma once

#include <QDialog>
#include <QDialogButtonBox>
#include <QLabel>
#include <QPushButton>
#include <QSortFilterProxyModel>
#include <QStandardItemModel>
#include <QTableView>

class LocalizePluginDialog : public QDialog {
    Q_OBJECT
public:
    explicit LocalizePluginDialog(const QString& pluginName,
                                  QWidget* parent = nullptr);

    // Data for each localizable string in the plugin
    struct LocalizableString {
        quint32 stringId;
        QString currentValue;
        QString localizedValue;
    };

    void setStrings(const QVector<LocalizableString>& strings);

    // After accepted, retrieve the (possibly edited) localized values
    QVector<LocalizableString> strings() const;

private slots:
    void onLocalize();

private:
    void setupUi(const QString& pluginName);

    QLabel*                 m_descriptionLabel;
    QTableView*             m_tableView;
    QStandardItemModel*     m_model;
    QDialogButtonBox*       m_buttonBox;
    QPushButton*            m_btnLocalize;
    QPushButton*            m_btnCancel;
};
