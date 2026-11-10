//! The QApplication run loop and the window grab, in C++ (see `cpp/window.cpp`).

#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("uq-settings/cpp/window.h");

        /// Run the app on `url` (see `cpp/window.h`): the exit code, or -1 with no window.
        fn uqs_run(version: &str, url: &str) -> i32;
        fn uqs_grab_window(path: &str) -> bool;
    }
}
