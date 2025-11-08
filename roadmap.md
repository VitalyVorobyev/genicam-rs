P4 — Stream hardening: MTU/packet size, resend, backpressure, multicast
	•	Auto MTU & packet size: probe NIC MTU, negotiate GevSCPSPacketSize, set GevSCPD (packet delay), detect jumbo frames; expose StreamConfig::auto().
	•	Resend tuning: coalesce missing ranges; bounded retries with jitter; per-frame deadlines; stats (resend reqs, acks, late/duplicate).
	•	Backpressure & zero-copy: ring buffer + preallocated frame pools; Bytes/Arc<[u8]> storage; “drop-oldest” policy when consumer lags; per-stream channel capacity knob.
	•	Multicast receive: configure destination multicast (SFNC), join_multicast() with NIC binding; filter by (src, block_id).
	•	Acceptance: sustain ≥900 Mb/s on 1 GbE with <0.1% drops on clean link; verified resend activity on induced loss; multicast demo across two receivers.

P5 — GenApi engine (Compliance Tier-1)
	•	Node graph: dependency graph, selector propagation, cache invalidation.
	•	Evaluators: Integer/Float with increment, min/max; Enum (entries/values); Boolean; Command with OnUpdate; visibility/access mode; SwissKnife formulas (subset).
	•	Reg map: address/length mapping, concatenated registers, endian helpers.
	•	SFNC helpers: typed getters/setters for common features; feature watchers.
	•	Acceptance: read/write Exposure/Gain/TriggerMode/PixelFormat end-to-end via node map (no raw register calls), selectors honored.

P6 — Tooling & UX
	•	CLI gencamctl: list, features get/set, start stream → PGM/PPM/BMP dump, events view, chunk dump, multicast join; --iface, --mtu, --packet-size, --delay.
	•	Bench gencam-bench: throughput/latency/resend/drops; JSON report.
	•	Windows support: socket opts, RCVBUF, NIC binding; firewall notes.
	•	Acceptance: CI builds on Linux/macOS/Windows; soak test (10 min) stable.

⸻

2. Roadmap for finishing the mdBook

Let’s group work into three phases so we don’t drown.

Phase 1 – Make the book useful for end-users (MVP)

Goal: someone with a GigE camera and basic Rust can discover, configure, and stream using only the book + README.

1.1. Crate overview page

Fill book/src/crates/README.md with:
	•	A short table with: crate → role → key public types / modules → who should read it (end-user vs contributor).
	•	Cross-links to existing deep-dives (genicp, tl-gige) and placeholders (genapi-core, genapi-xml, genicam).

1.2. Tutorials skeletons → real tutorials

Turn the empty tutorials into concrete, copy-pasteable guides that reuse your existing examples and gencamctl commands:
	•	tutorials/README.md
	•	Short intro + “recommended path”: Discovery → Registers → GenApi XML → Streaming.
	•	tutorials/discovery.md
	•	“Pre-flight” checklist (NIC, subnet, firewall, jumbo frames optional).
	•	gencamctl -- list and example output.
	•	Using the genicam example list_cameras.
	•	Troubleshooting section (“no devices”, “wrong interface”, “Windows firewall”, etc).
	•	tutorials/registers.md
	•	Read/write via CLI (get / set) + one feature as a worked example (e.g. ExposureTime).
	•	Low-level peek/poke example using tl-gige or genicp for people debugging vendor weirdness.
	•	“When to not use raw registers and stick to features.”
	•	tutorials/genapi-xml.md
	•	Fetch XML using control path (CLI + Rust example).
	•	How genapi-xml parses into IR and hands off to genapi-core (high-level only; deep details in crate pages).
	•	Where to look in docs to understand SwissKnife/selectors in XML.
	•	tutorials/streaming.md
	•	Basic streaming with gencamctl -- stream (auto MTU, save N frames).
	•	Interpreting log output (dropped packets, resends).
	•	Minimal Rust example using tl-gige::StreamBuilder from the book.  ￼

1.3. Networking essentials (for users)

Fill networking.md with a user-level “GigE Vision networking cookbook”:
	•	How to choose NIC / subnet design (dedicated NIC vs shared, IPv4 assumptions).
	•	MTU / jumbo frames: when to care, what to set, rough defaults.
	•	Windows specifics (firewall, admin first run, NIC settings), echoing Quick Start but slightly more detailed.  ￼
	•	Simple recipes: “single camera directly connected”, “multiple cameras via switch”.

1.4. FAQ (user focus first)

Fill faq.md with 6–10 Q&A entries based on your typical pain points:
	•	“Why does discovery show nothing?”
	•	“I get drops at high FPS, what knobs do I tweak?”
	•	“Why does streaming work in vendor viewer but not here?”
	•	“How do I select the right interface on a multi-NIC system?”
	•	“Does this work on Windows?” etc.

We can add contributor questions later.

⸻

Phase 2 – Contributor-level docs & internals

Goal: someone new can contribute to GenApi or transport code without asking you in chat for the mental model.

2.1. genapi-xml chapter

Fill crates/genapi-xml.md with:
	•	Mental model: schema-lite, tolerated subset of GenApi XML, what is in IR vs ignored.
	•	Fetch pipeline: control path → address → XML → parse → IR.
	•	Extensibility: how to add support for new node attributes or SFNC variants.
	•	Testing strategies: golden XML files, “nasty” vendor XML, fuzzing/parsing.

2.2. genapi-core chapter

Fill crates/genapi-core.md with:
	•	Node types overview (list the implemented node kinds clearly, including SwissKnife).
	•	Node evaluation mechanics: dependency graph, caching, selectors, side-effects.
	•	How SwissKnife is implemented (tie back to Primer).  ￼
	•	Examples of reading/writing a feature through NodeMap, including selector-dependent ones.
	•	Extension advice: how to add a new node type, how to write tests for it.

2.3. genicam façade chapter

Fill crates/genicam.md with:
	•	High-level API: what a simple application does in 10–20 lines (discover → connect → NodeMap → get/set → stream).
	•	How it composes tl-gige, genicp, genapi-xml, genapi-core.
	•	Where the boundary is between “public API” and “internal extension points”.

2.4. errors-logging.md

Document:
	•	Error categorisation (transport vs protocol vs GenApi vs evaluation).
	•	How to enable logging (RUST_LOG, levels, maybe JSON).  ￼
	•	What common error messages mean and which chapter/tutorial helps.

2.5. testing.md

Describe:
	•	Workspace test strategy: unit tests per crate; integration tests with mocked transports; potential hardware-in-the-loop tests.
	•	How to run tests locally (you already describe cargo test --workspace in Quick Start → link to this chapter for details).  ￼
	•	CI expectations (even a rough outline is enough).

2.6. contributing.md + glossary.md
	•	contributing.md: coding style, MSRV policy, how to run formatting/lints/tests, how to propose changes, how to run examples.
	•	glossary.md: short explanations of GenApi, GenCP, GVCP, GVSP, PFNC, SFNC, NodeMap, SwissKnife, etc., with links back to primer and crate chapters.

⸻

Phase 3 – Tie-ups, license page, and future-facing bits

Goal: polish and align book, README, and technical roadmap.

3.1. license.md
	•	Simple: mirror README’s License section and link to LICENSE.  ￼

3.2. crates/placeholders.md
	•	Brief overview of planned crates or expansion (USB3 transport, GenTL producer, helper crates).
	•	Make it explicit what’s speculative vs already under development, so it doesn’t go stale.

3.3. tutorials & networking cross-links
	•	From tutorials, link back into crate chapters for people who want to dig deeper.
	•	From crate chapters, link to tutorials that demonstrate “how to actually use this”.

3.4. Periodic consistency pass
	•	After bigger releases (e.g., USB3 Vision transport, GenTL producer), update:
	•	welcome.md “What works today / What’s coming next”.  ￼
	•	primer.md section on standards mapping.  ￼
	•	crates/placeholders.md and README “Current status”.

We can also later add a short “Release notes” or “Changelog highlights” section in the book if you like, but that’s optional.

⸻

3. Roadmap for updating the README

Here’s what I’d change, in order.

R1. Fix OS support line

In “Prereqs”, replace:

- Linux/macOS (Windows planned)  ￼

with something aligned with the book, e.g.:

- Windows / Linux / macOS (tested on recent versions; see docs for specifics)

and maybe mention admin+firewall caveats right there or link to “Networking Troubleshooting” in the book.

R2. Split the “future blob” in Current status

Currently you have:

- USB3 Vision; SwissKnife & advanced GenApi; GenTL producer (.cti).  ￼

Given SwissKnife is now implemented and documented, I’d propose:
	•	Add a ✅ bullet:
	•	✅ SwissKnife expression nodes (subset) and selector-aware NodeMap (see book “Primer” & “genapi-core”).
	•	Keep future work explicit:
	•	- USB3 Vision transport (planned)
	•	- Advanced GenApi nodes (Converter, complex expressions, richer SFNC coverage — planned)
	•	- GenTL producer (.cti) and PFNC/SFNC utilities (planned)

R3. Add “Documentation” section

Right after “Workspace layout”, add:
	•	Link to the mdBook (once you’re happy with the basic MVP):
	•	e.g. “User & contributor book: <link> (built with mdBook; sources under book/)”.
	•	Link to rustdoc on GitHub Pages if/when that’s working again.
	•	Short note that the book is the primary user guide and README is a landing page.

R4. Cross-link CLI and examples to book

In the “Run examples” and “gencamctl CLI” sections, add a small parenthetical link:
	•	“See Tutorials → Discovery in the book for more context.”
	•	“See Streaming (GVSP) – first steps tutorial for interpreting stats/logs.”

So README becomes the quick TL;DR, and the book the narrative.

R5. Tiny consistency tweaks
	•	Ensure terminology matches the book:
	•	GenCP / GVCP / GVSP spelled consistently.
	•	Names of examples/commands the same as in tutorials.
	•	If you decide on a short “one-liner” slogan in Welcome (“Ethernet-first, pure Rust GenICam building blocks”), reuse it in README’s tagline.

⸻

4. How we can proceed step-by-step

To keep things focused and incremental, I suggest this order:
	1.	README fixes (R1 + R2) – trivial edits that immediately remove contradictions.
	2.	Crates overview page (crates/README.md) – sets the frame for the rest of the book.
	3.	Tutorials (discovery → registers → GenApi XML → streaming) – this gives you a user-facing MVP.
	4.	Networking.md + FAQ.md – operational support.
	5.	Crate internals (genapi-xml, genapi-core, genicam).
	6.	Errors/logging, testing, contributing, glossary, license, placeholders.
