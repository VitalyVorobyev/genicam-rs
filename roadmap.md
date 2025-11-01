Awesome—let’s set you up with a clean, scalable Rust workspace for a pure GenICam stack, starting with GigE Vision and paving the way for USB3 Vision. Below you’ll find:
	1.	a starter project structure (Cargo workspace + crate skeletons),
	2.	a roadmap with crisp milestones & acceptance criteria, and
	3.	a Codex prompt you can paste into your code assistant to begin implementing step-by-step.

⸻

1) Starter project structure (Cargo workspace)

genicam-rs/
├─ Cargo.toml                 # [workspace] members and shared profile
├─ rust-toolchain.toml        # pin stable
├─ .editorconfig
├─ .gitignore
├─ crates/
│  ├─ genicp/                 # GenCP: encode/decode control messages (transport-agnostic)
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  ├─ genapi-xml/             # XML loader + schema validation helpers (no node logic)
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  ├─ genapi-core/            # Node types, node map, evaluation, feature access
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  ├─ tl-gige/                # GigE Vision TL: discovery (GVCP), control (GenCP/GVCP), stream (GVSP)
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  ├─ tl-u3v/                 # USB3 Vision TL: control/stream via libusb (rusb) – later phase
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  ├─ pfnc/                   # Pixel format codes and utilities (PFNC)
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  ├─ sfnc/                   # Standard Features Naming Convention constants/helpers (SFNC)
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  ├─ genicam/                # Public, ergonomic API: Camera, NodeMap, Stream, Image
│  │   ├─ Cargo.toml
│  │   └─ src/lib.rs
│  └─ gentl-cti/              # (Optional, later) GenTL Producer (.cti) C-ABI wrapper
│      ├─ Cargo.toml
│      └─ src/lib.rs
└─ examples/
   ├─ list_cameras.rs         # enumerate via tl-gige, print model, IP, access
   ├─ grab_gige.rs            # start stream, dump first image to disk, print metadata
   └─ get_set_feature.rs      # read ExposureTime, set to new value, verify

2) Roadmap (phased, with acceptance criteria)

Phase 0 — Foundations & scaffolding
	•	Goal: Compile-ready workspace + logging + error taxonomy.
	•	Tasks:
	•	Set up tracing, error enums across crates, feature flags (gige, u3v).
	•	Decide on tokio (async) for sockets; keep TL crates async-first.
	•	Acceptance: cargo build works; examples/list_cameras.rs compiles (stub).

Phase 1 — GenCP core + GigE discovery & XML retrieval (control path MVP)
	•	Goal: Talk to a GigE Vision camera’s control channel.
	•	Tasks:
	•	genicp: implement opcodes and header encode/decode; map status codes.
	•	tl-gige:
	•	Discovery via GVCP broadcast; collect IP/MAC, model, name.
	•	Control: open control channel; implement GenCP ReadMem/WriteMem over GVCP.
	•	XML retrieval: read the camera’s XML (typical address advertised via registers) and return it.
	•	genapi-xml: minimal XML sanity parse (schema/version extraction).
	•	Acceptance: examples/list_cameras.rs returns real devices; examples/get_set_feature.rs can fetch and print XML size.

Phase 2 — GenApi minimal node map + get/set for common SFNC features
	•	Goal: Read/write real features.
	•	Tasks:
	•	genapi-core: implement Integer/Float/Enum/Bool/Command nodes with min/max, access mode, selector support (basic).
	•	genapi-xml: map SFNC names → Node instances (ignore complex formulas initially).
	•	genicam: map node writes to GenCP register writes (address comes from XML).
	•	Acceptance: Read ExposureTime, Gain, AcquisitionMode; set ExposureTime within range and verify by read-back.

Phase 3 — GVSP streaming MVP (images in, no fancy features)
	•	Goal: Receive images reliably.
	•	Tasks:
	•	tl-gige: open stream channel; packet reassembly, basic packet-loss handling; negotiate packet size/MTU; timeouts.
	•	pfnc: parse pixel format codes; deliver Image { width, height, stride, pixel_format, bytes }.
	•	genicam: start/stop acquisition, frame iterator or async stream.
	•	Acceptance: examples/grab_gige.rs saves frames to disk (Mono8, BayerRG8, RGB8) and reports fps.

Phase 4 — USB3 Vision control + streaming MVP
	•	Goal: Parity with Phase 1 & 3 for U3V.
	•	Tasks:
	•	tl-u3v: enumerate via rusb; claim interface; GenCP over bulk endpoints; streaming over bulk/iso endpoints.
	•	Acceptance: Same examples work with a USB3 Vision camera.

Phase 5 — Events, Chunk Data, Selectors, Dependency Graph
	•	Goal: Real-world robustness.
	•	Tasks:
	•	Event channel (GigE message channel; U3V equivalents).
	•	Chunk data parsing; deliver per-frame metadata.
	•	GenApi: implement formulas, cache invalidation, selector propagation, OnUpdate commands.
	•	Acceptance: Trigger events observed; chunk exposure time read per-frame; selector-driven features work.

Phase 6 — Performance & polish
	•	Zero-copy buffers where possible; NIC tuning (receive buffers, busy-poll optional); configurable thread/async model.
	•	Benchmarks: sustained >1 Gbps with packet loss <0.01% (typical NIC/OS permitting).

Phase 7 — (Optional) GenTL Producer (.cti)
	•	Provide C ABI for external GenTL consumers; basic conformance with common viewers.

Phase 8 — Conformance & docs
	•	Prep for A3 conformance tests (GigE/U3V); add developer guide and SFNC coverage table.
