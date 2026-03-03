#include "ElementDetailDialog.h"
#include "ffi/XEditFFI.h"
#include "util/StringBuffer.h"

#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSet>
#include <QStringDecoder>
#include <QVBoxLayout>

// Forward declarations of static helpers defined below
static QString formatHex(const QByteArray& data);
static QString formatFlags(const QByteArray& data, const QString& sig);
static QString tryDecode(const QByteArray& data);

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

ElementDetailDialog::ElementDetailDialog(int pluginIdx, int groupIdx,
                                         int recordIdx, int subrecordIdx,
                                         QWidget* parent)
    : QDialog(parent)
    , m_pluginIdx(pluginIdx)
    , m_groupIdx(groupIdx)
    , m_recordIdx(recordIdx)
    , m_subrecordIdx(subrecordIdx)
{
    setWindowTitle(tr("Element Detail"));
    resize(600, 400);

    buildUi();
    loadData();
}

// ---------------------------------------------------------------------------
// UI construction
// ---------------------------------------------------------------------------

void ElementDetailDialog::buildUi()
{
    auto* mainLayout = new QVBoxLayout(this);

    // --- Field labels in a form layout ---
    auto* form = new QFormLayout;
    form->setFieldGrowthPolicy(QFormLayout::ExpandingFieldsGrow);

    m_lblElementPath = new QLabel(QStringLiteral("-"));
    m_lblType        = new QLabel(QStringLiteral("-"));
    m_lblSize        = new QLabel(QStringLiteral("-"));
    m_lblFlags       = new QLabel(QStringLiteral("-"));

    form->addRow(tr("Element Path:"), m_lblElementPath);
    form->addRow(tr("Type:"),         m_lblType);
    form->addRow(tr("Size:"),         m_lblSize);
    form->addRow(tr("Flags:"),        m_lblFlags);

    mainLayout->addLayout(form);

    // --- Raw hex data ---
    mainLayout->addWidget(new QLabel(tr("Raw Data:")));

    m_txtRawData = new QPlainTextEdit(this);
    m_txtRawData->setReadOnly(true);
    m_txtRawData->setFont(QFont(QStringLiteral("monospace"), 9));
    m_txtRawData->setLineWrapMode(QPlainTextEdit::WidgetWidth);
    m_txtRawData->setMaximumHeight(160);
    mainLayout->addWidget(m_txtRawData, 1);

    // --- Decoded value ---
    auto* valueForm = new QFormLayout;
    m_editValue = new QLineEdit(this);
    m_editValue->setReadOnly(true); // default; loadData may enable
    valueForm->addRow(tr("Value:"), m_editValue);
    mainLayout->addLayout(valueForm);

    mainLayout->addStretch();

    // --- Buttons ---
    auto* btnLayout = new QHBoxLayout;
    btnLayout->addStretch();

    m_btnOk = new QPushButton(tr("OK"), this);
    m_btnOk->setDefault(true);
    m_btnOk->setVisible(false); // shown only when editable
    btnLayout->addWidget(m_btnOk);

    m_btnCancel = new QPushButton(tr("Close"), this);
    btnLayout->addWidget(m_btnCancel);

    mainLayout->addLayout(btnLayout);

    connect(m_btnOk,     &QPushButton::clicked, this, &QDialog::accept);
    connect(m_btnCancel, &QPushButton::clicked, this, &QDialog::reject);
}

// ---------------------------------------------------------------------------
// Data loading via FFI
// ---------------------------------------------------------------------------

void ElementDetailDialog::loadData()
{
    auto& ffi = XEditFFI::instance();
    if (!ffi.isLoaded())
        return;

    // --- Signature (element path) ---
    if (ffi.xedit_subrecord_signature) {
        m_signature = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_subrecord_signature(
                m_pluginIdx, m_groupIdx, m_recordIdx, m_subrecordIdx, buf, len);
        });
    }

    // Build element path:  <record_sig> \ <subrecord_sig>
    QString recordSig;
    if (ffi.xedit_record_signature) {
        recordSig = ffiString([&](char* buf, int32_t len) {
            return ffi.xedit_record_signature(
                m_pluginIdx, m_groupIdx, m_recordIdx, buf, len);
        });
    }
    QString path = recordSig.isEmpty()
        ? m_signature
        : QStringLiteral("%1 \\ %2").arg(recordSig, m_signature);
    m_lblElementPath->setText(path);

    // --- Size ---
    if (ffi.xedit_subrecord_size) {
        m_dataSize = ffi.xedit_subrecord_size(
            m_pluginIdx, m_groupIdx, m_recordIdx, m_subrecordIdx);
    }
    m_lblSize->setText(QStringLiteral("%1 bytes").arg(m_dataSize));

    // --- Type heuristic ---
    m_lblType->setText(QStringLiteral("Subrecord (%1)").arg(m_signature));

    // --- Raw data ---
    if (m_dataSize > 0 && ffi.xedit_subrecord_data) {
        m_rawData.resize(m_dataSize);
        int32_t bytesRead = ffi.xedit_subrecord_data(
            m_pluginIdx, m_groupIdx, m_recordIdx, m_subrecordIdx,
            m_rawData.data(), m_dataSize);
        if (bytesRead < 0) bytesRead = 0;
        m_rawData.truncate(bytesRead);
    }

    m_txtRawData->setPlainText(formatHex(m_rawData));

    // --- Flags ---
    m_lblFlags->setText(formatFlags(m_rawData, m_signature));

    // --- Decoded value ---
    m_decodedValue = tryDecode(m_rawData);
    m_editValue->setText(m_decodedValue);

    // Enable editing for known text subrecords
    static const QSet<QString> textSigs = {
        QStringLiteral("EDID"), QStringLiteral("FULL"),
        QStringLiteral("DESC"), QStringLiteral("MODL"),
        QStringLiteral("ICON"), QStringLiteral("MICO"),
    };
    m_editable = textSigs.contains(m_signature);
    if (m_editable) {
        m_editValue->setReadOnly(false);
        m_btnOk->setVisible(true);
        m_btnCancel->setText(tr("Cancel"));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

QString ElementDetailDialog::formatHex(const QByteArray& data)
{
    if (data.isEmpty())
        return QStringLiteral("(empty)");

    // Classic hex-dump: 16 bytes per line with offset column
    QString result;
    const int total = data.size();
    for (int offset = 0; offset < total; offset += 16) {
        // Offset
        result += QStringLiteral("%1  ").arg(offset, 8, 16, QLatin1Char('0'));

        // Hex bytes
        QString ascii;
        for (int i = 0; i < 16; ++i) {
            if (offset + i < total) {
                auto byte = static_cast<uint8_t>(data.at(offset + i));
                result += QStringLiteral("%1 ").arg(byte, 2, 16, QLatin1Char('0'));
                ascii += (byte >= 0x20 && byte < 0x7F)
                    ? QChar(byte) : QChar(u'.');
            } else {
                result += QStringLiteral("   ");
                ascii += u' ';
            }
            if (i == 7)
                result += u' '; // extra space between groups of 8
        }

        result += QStringLiteral(" |%1|\n").arg(ascii);
    }
    return result;
}

QString ElementDetailDialog::formatFlags(const QByteArray& data,
                                         const QString& signature)
{
    Q_UNUSED(data)
    Q_UNUSED(signature)
    // Placeholder -- real flag decoding requires record-definition tables.
    // For now report whether the subrecord appears to contain text.
    if (data.isEmpty())
        return QStringLiteral("(none)");

    // Check if it looks like a null-terminated string
    if (!data.isEmpty() && data.back() == '\0') {
        bool allPrintable = true;
        for (int i = 0; i < data.size() - 1; ++i) {
            auto ch = static_cast<uint8_t>(data.at(i));
            if (ch < 0x20 || ch > 0x7E) { allPrintable = false; break; }
        }
        if (allPrintable)
            return QStringLiteral("zString (null-terminated text)");
    }
    return QStringLiteral("(none)");
}

// ---------------------------------------------------------------------------
// Text decode (same logic as RecordViewModel)
// ---------------------------------------------------------------------------

static QString tryDecode(const QByteArray& data)
{
    if (data.isEmpty())
        return {};

    // Strip trailing nulls
    QByteArray trimmed = data;
    while (!trimmed.isEmpty() && trimmed.back() == '\0')
        trimmed.chop(1);

    if (trimmed.isEmpty())
        return {};

    auto toUtf16 = QStringDecoder(QStringDecoder::Utf8);
    QString result = toUtf16(trimmed);
    if (toUtf16.hasError())
        return data.toHex(' ').toUpper();

    for (const QChar& ch : result) {
        if (ch.unicode() < 0x20 && ch != u'\n' && ch != u'\r' && ch != u'\t')
            return data.toHex(' ').toUpper();
    }
    return result;
}
