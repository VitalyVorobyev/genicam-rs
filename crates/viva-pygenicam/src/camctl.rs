//! The `viva-camctl` CLI, reachable from the installed wheel.
//!
//! `pip install viva-genicam` is what we point users at when we need a
//! `viva-camctl report` or a `viva-camctl xml` from a camera we cannot open —
//! and until now the wheel shipped no such command, so the instruction was
//! wrong for exactly the people least able to build it from source (#45).
//!
//! Linking the CLI into this extension module keeps one implementation and one
//! artifact: every wheel and every `pip install` from an sdist gets the command,
//! with no per-target binary to stage. `viva-camctl` is a sibling leaf crate, not
//! a layer below this one, so the dependency adds no cycle to the stack in
//! `docs/design.md`.

use pyo3::prelude::*;

/// Run the CLI and return the exit code the process should use.
///
/// `argv` excludes the program name; the console script passes `sys.argv[1:]`.
///
/// The GIL is released for the duration: a `viva-camctl stream` runs for as long
/// as the user asked, and holding the GIL would freeze any other thread in the
/// interpreter for all of it.
#[pyfunction]
fn camctl_main(py: Python<'_>, argv: Vec<String>) -> u8 {
    py.detach(|| {
        let mut args = Vec::with_capacity(argv.len() + 1);
        args.push("viva-camctl".to_string());
        args.extend(argv);
        // Return the code rather than calling `std::process::exit`, which would
        // skip the interpreter's own shutdown.
        viva_camctl::run(args)
    })
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(camctl_main, m)?)?;
    Ok(())
}
