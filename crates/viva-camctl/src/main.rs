//! Thin wrapper around [`viva_camctl::run`].
//!
//! Everything else lives in the library so the Python wheel's console script
//! runs the same code path.

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(viva_camctl::run(std::env::args_os()))
}
