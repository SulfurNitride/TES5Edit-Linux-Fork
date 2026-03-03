#include "LegendDialog.h"
#include "util/ConflictColors.h"
#include <QVBoxLayout>
#include <QGridLayout>
#include <QGroupBox>
#include <QLabel>
#include <QPushButton>

static QLabel* makeColorSwatch(const QColor& color)
{
    auto* swatch = new QLabel;
    swatch->setFixedSize(24, 24);
    if (color.isValid() && color.alpha() > 0) {
        swatch->setStyleSheet(
            QStringLiteral("background-color: %1; border: 1px solid #888;").arg(color.name()));
    } else {
        swatch->setStyleSheet(QStringLiteral("background-color: transparent; border: 1px solid #888;"));
    }
    return swatch;
}

static QLabel* makeTextLabel(const QString& text, const QColor& color)
{
    auto* label = new QLabel(text);
    if (color.isValid()) {
        label->setStyleSheet(QStringLiteral("color: %1;").arg(color.name()));
    }
    return label;
}

LegendDialog::LegendDialog(QWidget* parent)
    : QDialog(parent)
{
    setWindowTitle("Conflict Color Legend");
    setFixedSize(650, 500);

    auto* mainLayout = new QVBoxLayout(this);

    // --- Section 1: Record Status (Background Colors) ---
    auto* bgGroup = new QGroupBox("Record Status (Background Colors)", this);
    auto* bgGrid = new QGridLayout(bgGroup);

    struct BgEntry {
        ConflictAll value;
        const char* description;
    };
    const BgEntry bgEntries[] = {
        { ConflictAll::Unknown,    "Unknown"           },
        { ConflictAll::OnlyOne,    "Single Record"     },
        { ConflictAll::NoConflict, "No Conflict"       },
        { ConflictAll::Benign,     "Benign Conflict"   },
        { ConflictAll::Override,   "Override"           },
        { ConflictAll::Conflict,   "Conflict"          },
        { ConflictAll::Critical,   "Critical Conflict"  },
    };

    for (int i = 0; i < int(std::size(bgEntries)); ++i) {
        QColor bg = ConflictColors::backgroundFor(bgEntries[i].value);
        bgGrid->addWidget(makeColorSwatch(bg), i, 0);
        bgGrid->addWidget(new QLabel(bgEntries[i].description), i, 1);
    }

    bgGrid->setColumnStretch(1, 1);
    mainLayout->addWidget(bgGroup);

    // --- Section 2: File Status (Text Colors) ---
    auto* txtGroup = new QGroupBox("File Status (Text Colors)", this);
    auto* txtGrid = new QGridLayout(txtGroup);

    struct TxtEntry {
        ConflictThis value;
        const char* description;
    };
    const TxtEntry txtEntries[] = {
        { ConflictThis::Unknown,                        "Unknown"                        },
        { ConflictThis::Ignored,                        "Ignored"                        },
        { ConflictThis::NotDefined,                     "Not Defined"                    },
        { ConflictThis::IdenticalToMaster,              "Identical to Master"            },
        { ConflictThis::OnlyOne,                        "Single Record"                  },
        { ConflictThis::HiddenByModGroup,               "Hidden by Mod Group"            },
        { ConflictThis::Master,                         "Master"                         },
        { ConflictThis::ConflictBenign,                 "Benign Conflict"                },
        { ConflictThis::Override,                       "Override"                       },
        { ConflictThis::IdenticalToMasterWinsConflict,  "Identical to Master (Wins)"     },
        { ConflictThis::ConflictWins,                   "Conflict Wins"                  },
        { ConflictThis::ConflictLoses,                  "Conflict Loses"                 },
    };

    for (int i = 0; i < int(std::size(txtEntries)); ++i) {
        QColor tc = ConflictColors::textColorFor(txtEntries[i].value);
        txtGrid->addWidget(makeTextLabel(txtEntries[i].description, tc), i, 0);
    }

    txtGrid->setColumnStretch(0, 1);
    mainLayout->addWidget(txtGroup);

    // --- Close button ---
    auto* btnLayout = new QHBoxLayout();
    btnLayout->addStretch();
    auto* btnClose = new QPushButton("Close", this);
    btnClose->setDefault(true);
    btnLayout->addWidget(btnClose);
    mainLayout->addLayout(btnLayout);

    connect(btnClose, &QPushButton::clicked, this, &QDialog::accept);
}
