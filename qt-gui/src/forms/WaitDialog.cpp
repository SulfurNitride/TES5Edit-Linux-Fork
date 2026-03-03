#include "WaitDialog.h"
#include <QMetaObject>

static WaitDialog* s_instance = nullptr;

WaitDialog::WaitDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Please wait...");
    setFixedSize(450, 150);
    setModal(true);

    auto* layout = new QVBoxLayout(this);

    m_label = new QLabel("Loading...", this);
    layout->addWidget(m_label);

    m_progressBar = new QProgressBar(this);
    m_progressBar->setRange(0, 0); // indeterminate by default
    layout->addWidget(m_progressBar);

    m_cancelButton = new QPushButton("Cancel", this);
    layout->addWidget(m_cancelButton);

    connect(m_cancelButton, &QPushButton::clicked, this, &QDialog::reject);
    connect(this, &WaitDialog::progressUpdated, this, &WaitDialog::onProgressUpdated);

    s_instance = this;
}

void WaitDialog::setMessage(const QString& message)
{
    m_label->setText(message);
}

void WaitDialog::setProgress(double fraction)
{
    if (fraction < 0.0) {
        m_progressBar->setRange(0, 0); // indeterminate
    } else {
        m_progressBar->setRange(0, 100);
        m_progressBar->setValue(static_cast<int>(fraction * 100.0));
    }
}

void WaitDialog::progressCallback(const char* message, double progress)
{
    if (!s_instance)
        return;

    QString msg = QString::fromUtf8(message);
    WaitDialog* inst = s_instance;
    QMetaObject::invokeMethod(
        inst,
        [inst, msg, progress]() {
            emit inst->progressUpdated(msg, progress);
        },
        Qt::QueuedConnection
    );
}

void WaitDialog::onProgressUpdated(const QString& message, double progress)
{
    setMessage(message);
    setProgress(progress);
}
