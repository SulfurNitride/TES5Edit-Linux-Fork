#pragma once
#include <QDialog>
#include <QCheckBox>
#include <QLabel>
#include <QStringList>

class TipDialog : public QDialog {
    Q_OBJECT
public:
    explicit TipDialog(const QStringList& tips, QWidget* parent = nullptr);

    bool showTipsAtStartup() const;

private slots:
    void nextTip();
    void previousTip();

private:
    QLabel*     m_tipLabel;
    QCheckBox*  m_showAtStartupCheck;
    QStringList m_tips;
    int         m_currentIndex;

    void displayCurrentTip();
};
