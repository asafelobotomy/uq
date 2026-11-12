//! The QApplication run loop, the theme's look and the window grab, in C++ (see `cpp/window.cpp`).

#[cxx::bridge]
pub mod ffi {
    /// What the app takes from uQ's theme. Empty strings and a size of 0 leave that part alone.
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct UqsLook {
        /// The KDE colour scheme uqd renders (`<theme_dir>/kde/uq.colors`).
        colors: String,
        icon_theme: String,
        font: String,
        point_size: i32,
    }

    unsafe extern "C++" {
        include!("uq-settings/cpp/window.h");

        /// Run the app on `url` with `look` (see `cpp/window.h`): the exit code, or -1 with no
        /// window.
        fn uqs_run(version: &str, url: &str, look: &UqsLook) -> i32;
        /// Re-apply the theme's look to the running app (on the Qt thread).
        fn uqs_apply_look(look: &UqsLook);
        fn uqs_grab_window(path: &str) -> bool;
    }
}
