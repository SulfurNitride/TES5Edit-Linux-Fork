#pragma once

#include <QDialog>

class QLabel;
class QLineEdit;
class QPlainTextEdit;
class QPushButton;

/// Dialog showing detailed information about a single record element/subrecord.
/// Displays the element path (signature), type, size, flags, raw hex data,
/// and a decoded value (editable when the element supports it).
class ElementDetailDialog : public QDialog {
    Q_OBJECT
public:
    /// Construct for a specific subrecord within a record.
    explicit ElementDetailDialog(int pluginIdx, int groupIdx,
                                 int recordIdx, int subrecordIdx,
                                 QWidget* parent = nullptr);

private:
    void loadData();
    void buildUi();

    static QString formatHex(const QByteArray& data);
    static QString formatFlags(const QByteArray& data, const QString& signature);

    int m_pluginIdx;
    int m_groupIdx;
    int m_recordIdx;
    int m_subrecordIdx;

    // Header fields
    QLabel*          m_lblElementPath  = nullptr;
    QLabel*          m_lblType         = nullptr;
    QLabel*          m_lblSize         = nullptr;
    QLabel*          m_lblFlags        = nullptr;

    // Data views
    QPlainTextEdit*  m_txtRawData      = nullptr;
    QLineEdit*       m_editValue       = nullptr;

    // Buttons
    QPushButton*     m_btnOk           = nullptr;
    QPushButton*     m_btnCancel       = nullptr;

    // Cached data
    QString          m_signature;
    int32_t          m_dataSize        = 0;
    QByteArray       m_rawData;
    QString          m_decodedValue;
    bool             m_editable        = false;
};
