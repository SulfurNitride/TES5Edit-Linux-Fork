#pragma once
#include <QDialog>
#include <QCheckBox>
#include <QLabel>
#include <QPushButton>
#include <QTimer>

class EditWarningDialog : public QDialog {
    Q_OBJECT
public:
    explicit EditWarningDialog(const QString& message, QWidget* parent = nullptr);

    bool dontShowAgain() const;

    /// Shows the warning dialog and returns true if the user confirmed.
    static bool confirmEdit(QWidget* parent, const QString& message);

private slots:
    void onTimerTick();

private:
    QLabel*      m_messageLabel;
    QCheckBox*   m_dontShowCheck;
    QPushButton* m_confirmButton;
    QPushButton* m_cancelButton;
    QTimer*      m_timer;
    int          m_countdown;
};
