#pragma once
#include <QDialog>
#include <QLineEdit>
#include <QListWidget>
#include <QPushButton>
#include <QStringList>

class ModGroupEditDialog : public QDialog {
    Q_OBJECT
public:
    explicit ModGroupEditDialog(const QString& modGroupName = {},
                                const QStringList& plugins = {},
                                QWidget* parent = nullptr);

    QString modGroupName() const;
    QStringList plugins() const;

    void setModGroupName(const QString& name);
    void setPlugins(const QStringList& plugins);

private slots:
    void onAddPlugin();
    void onRemovePlugin();
    void onMoveUp();
    void onMoveDown();
    void onNameChanged(const QString& text);
    void onSelectionChanged();
    void validateState();

private:
    void setupUI();
    void updateMoveButtons();

    QLineEdit* m_edName;
    QListWidget* m_lstPlugins;

    QPushButton* m_btnAdd;
    QPushButton* m_btnRemove;
    QPushButton* m_btnMoveUp;
    QPushButton* m_btnMoveDown;

    QPushButton* m_btnOk;
    QPushButton* m_btnCancel;
};
