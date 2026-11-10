//! Application setup and the window grab, in C++ (see `cpp/window.cpp`).

#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("uq-settings/cpp/window.h");

        fn uqs_describe_application(version: &str);
        fn uqs_has_window() -> bool;
        fn uqs_grab_window(path: &str) -> bool;
    }
}
