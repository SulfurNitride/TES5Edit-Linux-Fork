#pragma once
#include <QDialog>
#include <QCheckBox>
#include <QTextBrowser>

class DeveloperMessageDialog : public QDialog {
    Q_OBJECT
public:
    explicit DeveloperMessageDialog(const QString& htmlContent, QWidget* parent = nullptr);

    bool dontShowAgain() const;

private:
    QTextBrowser* m_browser;
    QCheckBox*    m_dontShowCheck;
};
