# Crates overview

The `genicam-rs` workspace is split into small crates that mirror the structure of
the GenICam ecosystem:

- **Protocols & transport** (GenCP, GVCP/GVSP)
- **GenApi XML loading & evaluation**
- **Public “facade” API** for applications
- **Command-line tooling** for everyday camera work

This chapter is the “map of the territory”. It tells you *which* crate to use
for a given task, and where to look if you want to hack on internals.

---

## Quick map

| Crate       | Path                   | Role / responsibility                                             | Primary audience                  |
|-------------|------------------------|--------------------------------------------------------------------|-----------------------------------|
| `genicp`    | `crates/genicp`        | GenCP encode/decode + helpers for control path over GVCP         | Contributors, protocol nerds      |
| `tl-gige`   | `crates/tl-gige`       | GigE Vision transport: GVCP (control) + GVSP (streaming)          | End-users & contributors          |
| `genapi-xml`| `crates/genapi-xml`    | Load GenICam XML from device / disk, parse into IR                | Contributors (XML / SFNC work)    |
| `genapi-core` | `crates/genapi-core` | NodeMap implementation, feature access, SwissKnife, selectors     | End-users & contributors          |
| `genicam`   | `crates/genicam`       | High-level “one crate” façade combining transport + GenApi        | End-users                         |
| `gencamctl` | `crates/gencamctl`     | CLI tool for discovery, configuration, streaming, benchmarks      | End-users, ops, CI scripts        |

If you just want to **use a camera** from Rust, you’ll usually start with
`genicam` (or `gencamctl` from the command line) and ignore the lower layers.

---

## How the crates fit together

At a high level, the crates compose like this:

```text
           ┌───────────────┐      ┌────────────────┐
           │   genicp      │      │   genapi-core  │
           │ GenCP encode  │      │ NodeMap,       │
           │ / decode      │      │ SwissKnife,    │
           └─────┬─────────┘      │ selectors      │
                 │                └──────┬─────────┘
                 │                       │
           ┌─────▼─────────┐      ┌──────▼─────────┐
           │   tl-gige     │      │  genapi-xml    │
           │ GVCP / GVSP   │      │ XML loading &  │
           │ packet I/O    │      │ schema-lite IR │
           └─────┬─────────┘      └──────┬─────────┘
                 │                       │
                 └──────────┬────────────┘
                            │
                      ┌─────▼─────┐
                      │  genicam  │  ← public Rust API
                      └─────┬─────┘
                            │
                      ┌─────▼───────┐
                      │ gencamctl   │  ← CLI on top of `genicam`
                      └─────────────┘
```

Roughly:
* `tl-gige` knows how to talk UDP to a GigE Vision device (discovery, register
access, image packets, resends, stats, …).
* `genicp` provides the GenCP building blocks used on the control path.
* `genapi-xml` fetches and parses the GenApi XML that describes the device’s
features.
* `genapi-core` turns that XML into a NodeMap you can read/write, including
SwissKnife expressions and selector-dependent features.
* `genicam` stitches all of the above into a reasonably ergonomic API.
* `gencamctl` exposes common workflows from genicam as `cargo run -p gencamctl -- …`.

⸻

## When to use which crate

### I just want to use my camera from Rust

Use `genicam`.

Typical tasks:
* Enumerate cameras on a NIC
* Open a device, read/write features by name
* Start a GVSP stream, iterate over frames, look at stats
* Subscribe to events or send action commands

Start with the examples under `crates/genicam/examples/` and the Tutorials.

⸻

### I want a command-line tool for daily work

Use `gencamctl`.

Typical tasks:
* Discovery: list all cameras on a given interface
* Register/feature inspection and configuration
* Quick streaming tests and stress benchmarks
* Enabling/disabling chunk data, configuring events

This is also a good reference for how to structure a “real” application on top
of genicam.

⸻

### I need to touch GigE Vision packets / low-level transport

Use `tl-gige` (and `genicp` as needed).

Example reasons:
* You want to experiment with MTU, packet delay, resend logic, or custom stats
* You’re debugging interoperability with a weird device and need raw GVCP/GVSP
* You want to build a non-GenApi tool that only tweaks vendor-specific registers

The `tl-gige` chapter goes into more detail on discovery,
streaming, events, actions, and tuning.

⸻

### I want to work on GenApi / XML internals

Use `genapi-xml` and `genapi-core`.

Typical contributor activities:
* Supporting new SFNC features or vendor extensions
* Improving SwissKnife coverage or selector handling
* Adding tests for tricky XML from specific camera families

The following chapters are relevant:
* GenApi XML loader: genapi-xml￼
* GenApi core & NodeMap: genapi-core￼

If you’re not sure where a GenApi bug lives, the rule of thumb is:
* “XML can’t be parsed” → genapi-xml
* “Feature exists but behaves wrong” → genapi-core
* “Device returns odd data / status codes” → tl-gige or genicp

⸻

### I need a single high-level entry point

Use `genicam`.

This crate aims to expose just enough control/streaming surface for most applications without making you think about transports, XML, or NodeMap internals.

The genicam￼ crate chapter￼ shows:
* How to go from “no camera” to “frames in memory” in ~20 lines
* How to query and set features safely (with proper types)
* How to plug in your own logging, error handling, and runtime

⸻

## Crate deep dives

The rest of this section of the book contains crate-specific chapters:
* GenCP: genicp￼– control protocol building blocks.
* GigE Vision transport: `tl-gige`￼– discovery, streaming, events, actions.
* GenApi XML loader: `genapi-xml`￼– getting from device to IR.
* GenApi core & NodeMap: `genapi-core` – evaluating features, including SwissKnife.
* Facade API: `genicam`￼– the crate most end-users start with.
* Future / helper crates – notes on planned additions.

If you’re reading this for the first time, a good path is:
1. Skim this page.
2. Read the genicam￼ chapter.
3. Jump to tl-gige or genapi-core when you hit something you want to tweak.
