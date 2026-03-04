cmake_minimum_required(VERSION 3.19)
project(xEdit VERSION 4.1.6 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets Core Gui)

set(SOURCES
    src/main.cpp
    # FFI
    src/ffi/XEditFFI.cpp
    # Models
    src/models/NavTreeItem.cpp
    src/models/NavTreeModel.cpp
    src/models/OverrideViewModel.cpp
    src/models/RecordViewModel.cpp
    src/models/RefByTableModel.cpp
    src/models/SpreadsheetModel.cpp
    # Delegates
    src/delegates/ConflictColorDelegate.cpp
    src/delegates/NavColorDelegate.cpp
    # Utilities
    src/util/SignatureNames.cpp
    src/util/ConflictColors.cpp
    src/util/ModGroupFile.cpp
    src/util/StringTableParser.cpp
    # Highlighting
    src/highlighting/PascalHighlighter.cpp
    # Forms
    src/forms/MainWindow.cpp
    src/forms/WaitDialog.cpp
    src/forms/ModuleSelectDialog.cpp
    src/forms/OptionsDialog.cpp
    src/forms/FilterOptionsDialog.cpp
    src/forms/LegendDialog.cpp
    src/forms/ScriptDialog.cpp
    src/forms/LogAnalyzerDialog.cpp
    src/forms/LocalizationDialog.cpp
    src/forms/LocalizePluginDialog.cpp
    src/forms/LODGenDialog.cpp
    src/forms/ModGroupSelectDialog.cpp
    src/forms/ModGroupEditDialog.cpp
    src/forms/FileSelectDialog.cpp
    src/forms/EditWarningDialog.cpp
    src/forms/DeveloperMessageDialog.cpp
    src/forms/TipDialog.cpp
    src/forms/RichEditDialog.cpp
    src/forms/ElementDetailDialog.cpp
    src/forms/ViewElementsDialog.cpp
    src/forms/WorldspaceCellDetailDialog.cpp
)

set(HEADERS
    src/ffi/XEditFFI.h
    src/ffi/XEditTypes.h
    src/util/StringBuffer.h
    src/util/SignatureNames.h
    src/util/ConflictColors.h
    src/util/SettingsManager.h
    src/util/ModGroupFile.h
    src/util/StringTableParser.h
    src/models/NavTreeItem.h
    src/models/NavTreeModel.h
    src/models/OverrideViewModel.h
    src/models/RecordViewModel.h
    src/models/RefByTableModel.h
    src/models/SpreadsheetModel.h
    src/delegates/ConflictColorDelegate.h
    src/delegates/NavColorDelegate.h
    src/highlighting/PascalHighlighter.h
    src/forms/MainWindow.h
    src/forms/WaitDialog.h
    src/forms/ModuleSelectDialog.h
    src/forms/OptionsDialog.h
    src/forms/FilterOptionsDialog.h
    src/forms/LegendDialog.h
    src/forms/ScriptDialog.h
    src/forms/LogAnalyzerDialog.h
    src/forms/LocalizationDialog.h
    src/forms/LocalizePluginDialog.h
    src/forms/LODGenDialog.h
    src/forms/ModGroupSelectDialog.h
    src/forms/ModGroupEditDialog.h
    src/forms/FileSelectDialog.h
    src/forms/EditWarningDialog.h
    src/forms/DeveloperMessageDialog.h
    src/forms/TipDialog.h
    src/forms/RichEditDialog.h
    src/forms/ElementDetailDialog.h
    src/forms/ViewElementsDialog.h
    src/forms/WorldspaceCellDetailDialog.h
)

add_executable(xEdit ${SOURCES} ${HEADERS})

target_include_directories(xEdit PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src)

if(WIN32)
    target_link_libraries(xEdit PRIVATE Qt6::Widgets Qt6::Core Qt6::Gui)
else()
    target_link_libraries(xEdit PRIVATE Qt6::Widgets Qt6::Core Qt6::Gui dl)
endif()

# Export compile commands for clangd
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Install
install(TARGETS xEdit RUNTIME DESTINATION bin)
