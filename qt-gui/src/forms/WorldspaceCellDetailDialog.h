#pragma once

#include <QDialog>

class QComboBox;
class QLabel;
class QPlainTextEdit;
class QPushButton;
class QRadioButton;
class QSpinBox;
class QSplitter;
class QTableWidget;

/// Dialog showing worldspace cell grid information.
/// Provides a worldspace selector, a cell coordinate grid, and a detail
/// panel for the currently selected cell.  Mirrors the original Delphi
/// TfrmWorldspaceCellDetails with expanded grid visualization.
class WorldspaceCellDetailDialog : public QDialog {
    Q_OBJECT
public:
    explicit WorldspaceCellDetailDialog(QWidget* parent = nullptr);

    /// Retrieve the user's selection after the dialog is accepted.
    bool isPersistent() const;
    int  cellX() const;
    int  cellY() const;
    QString selectedWorldspace() const;

private slots:
    void onWorldspaceChanged(int index);
    void onCellSelected(int row, int col);
    void onCellTypeChanged();

private:
    void buildUi();
    void loadWorldspaces();
    void populateCellGrid();
    void updateCellInfo(int gridX, int gridY);
    void updateSummary();

    // --- Top controls ---
    QComboBox*       m_cboWorldspace    = nullptr;
    QRadioButton*    m_rbPersistent     = nullptr;
    QRadioButton*    m_rbTemporary      = nullptr;
    QSpinBox*        m_spinX            = nullptr;
    QSpinBox*        m_spinY            = nullptr;

    // --- Grid area ---
    QTableWidget*    m_cellGrid         = nullptr;

    // --- Info panel ---
    QLabel*          m_lblCellCount     = nullptr;
    QLabel*          m_lblLoadedCells   = nullptr;
    QPlainTextEdit*  m_txtCellInfo      = nullptr;

    // --- Buttons ---
    QPushButton*     m_btnOk            = nullptr;
    QPushButton*     m_btnCancel        = nullptr;

    // Internal grid range (demonstrates a reasonable default)
    static constexpr int kGridRadius = 16;   // -16..+16 = 33x33 grid
};
