#pragma once
#include <QDialog>
#include <QLabel>
#include <QProgressBar>
#include <QPushButton>
#include <QVBoxLayout>

class WaitDialog : public QDialog {
    Q_OBJECT
public:
    explicit WaitDialog(QWidget* parent = nullptr);

    void setMessage(const QString& message);
    void setProgress(double fraction); // -1.0 for indeterminate, 0.0-1.0 for progress

    // Static method to register as FFI progress callback
    static void progressCallback(const char* message, double progress);

signals:
    void progressUpdated(const QString& message, double progress);

private slots:
    void onProgressUpdated(const QString& message, double progress);

private:
    QLabel* m_label;
    QProgressBar* m_progressBar;
    QPushButton* m_cancelButton;
};
