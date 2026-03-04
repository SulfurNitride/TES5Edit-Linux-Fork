#include <QApplication>
#include <QMessageBox>
#include "ffi/XEditFFI.h"
#include "forms/MainWindow.h"
#include "util/CommandLineArgs.h"

int main(int argc, char* argv[])
{
    QApplication app(argc, argv);
    app.setApplicationName("xEdit");
    app.setApplicationVersion("4.1.6");
    app.setOrganizationName("xEdit");

    CommandLineArgs::instance().parse(app);

    // Load the Rust FFI library
    if (!XEditFFI::instance().load()) {
        QMessageBox::critical(nullptr, "xEdit",
            "Could not load libxedit_core.so.\n\n"
            "Make sure the library is in the same directory as the executable\n"
            "or in LD_LIBRARY_PATH.");
        return 1;
    }

    MainWindow w;
    w.show();

    int ret = app.exec();

    // Shutdown the engine before the library is unloaded
    auto& ffi = XEditFFI::instance();
    if (ffi.xedit_shutdown)
        ffi.xedit_shutdown();

    return ret;
}
