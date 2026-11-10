use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    let builder = CxxQtBuilder::new_qml_module(QmlModule::new("org.uq.settings").qml_files([
        QmlFile::from("qml/Main.qml"),
        QmlFile::from("qml/Sidebar.qml"),
        QmlFile::from("qml/components/UqLabel.qml"),
        QmlFile::from("qml/components/SettingRow.qml"),
        QmlFile::from("qml/pages/AppearancePage.qml"),
        QmlFile::from("qml/pages/WallpaperPage.qml"),
    ]))
    .qt_module("Gui")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .files(["src/app.rs", "src/window.rs"])
    .cpp_file("cpp/window.cpp");
    // SAFETY: only adds a warning flag; GCC 16 warns about Qt's own headers.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.flag_if_supported("-Wno-sfinae-incomplete");
        })
    };
    builder.build();
}
