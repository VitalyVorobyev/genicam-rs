# Welcome & Goals

**viva-genicam** provides *pure Rust* building blocks for the GenICam ecosystem supporting **GigE Vision** and **USB3 Vision**, with first-class support for Windows, Linux, and macOS.

## Who is this book for?
- **End-users** building camera applications who want a practical high-level API and copy-pasteable examples.
- **Contributors** extending transports, GenApi features, and streaming -- who need a clear mental model of crates and internal boundaries.

## What works today
- **GigE Vision**: GVCP discovery, GVSP streaming with frame reassembly, events, action commands, chunk parsing, FORCEIP, persistent IP configuration.
- **USB3 Vision**: device discovery, GenCP register I/O, bulk-endpoint streaming, async frame iterator.
- **GenApi**: NodeMap with all standard node types (Integer, Float, Enum, Boolean, Command, Category, String, SwissKnife, Converter), pValue delegation, selectors, runtime access predicates (`pIsLocked`, `pIsAvailable`, `pIsImplemented`), node metadata and visibility filtering.
- **CLI** (`viva-camctl`): discovery, feature get/set, streaming, events, chunks, benchmarks, IP configuration, and a diagnostic report bundle.
- **Service bridge**: expose cameras over Zenoh for Viva Studio, the desktop app in `studio/`.

## What does not work yet

Worth knowing before you build on it:

- **Packet resend is not wired in.** The GVSP resend machinery exists and
  nothing calls it, so the `resends` statistic is always zero and means
  "not implemented" rather than "none were needed". Watch `drops`.
- **The Python bindings are narrower than the Rust API** — no chunks, events,
  time sync or action commands. [Python bindings](python.md) lists the gaps.
- **Viva Studio is experimental.** It works, and it has been driven against very
  little hardware.

> The protocol implementations follow the published EMVA specifications and are validated against built-in fake camera simulators (190+ automated tests). Testing against physical cameras from different manufacturers is ongoing -- bug reports and compatibility feedback are welcome.

## How this book is organized
- Start with **Quick Start** to build, test, and run the first discovery.
- Read the **Primer** and **Architecture** to get the big picture.
- Use **Crate Guides** and **Tutorials** for hands-on tasks.
- See **Networking** and **Troubleshooting** when packets don’t behave.
