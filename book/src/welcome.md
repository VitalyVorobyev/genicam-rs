# Welcome & Goals

**genicam-rs** provides *pure Rust* building blocks for the GenICam ecosystem with an Ethernet-first focus (GigE Vision), while keeping Windows, Linux, and macOS equally supported.

## Who is this book for?
- **End‑users** building camera applications who want a practical, high‑level API and copy‑pasteable examples.
- **Contributors** extending transports, GenApi features, and streaming—who need a clear mental model of crates and internal boundaries.

## What works today
- Device **discovery** over GigE Vision (GVCP) on a selected network interface.
- **Control path**: reading/writing device memory via GenCP over GVCP; fetching the device’s GenApi XML.
- **GenApi (tier 1)**: a basic NodeMap with common node kinds (Integer/Float/Enum/Bool/Command), ranges, access modes, and selector-based addressing.
- **CLI** (`gencamctl`) for common operations: discovery, feature get/set, streaming, events, chunks, and benchmarks.
- Early **streaming (GVSP)** support with reassembly, resend handling, MTU/packet sizing, and stats (evolving).

> Details evolve fast—check examples and release notes for the latest capabilities.

## What’s coming next
- Deeper GenApi evaluation (e.g., SwissKnife, formulas, dependencies).
- USB3 Vision transport.
- GenTL producer (.cti) and PFNC/SFNC coverage.

## How this book is organized
- Start with **Quick Start** to build, test, and run the first discovery.
- Read the **Primer** and **Architecture** to get the big picture.
- Use **Crate Guides** and **Tutorials** for hands‑on tasks.
- See **Networking** and **Troubleshooting** when packets don’t behave.
