# Backlog

Immediate, actionable tasks. Mid-term direction lives in
[roadmap.md](roadmap.md); shipped work is recorded in
[CHANGELOG.md](../CHANGELOG.md).

**Legend**

- Priority: **P0** (do next) / **P1** (soon) / **P2** (when convenient)
- Size: **S** (hours) / **M** (a day or two) / **L** (several days) /
  **XL** (a week+)
- Status: `planned` / `in-progress` / `done` / `blocked`

Every row carries its evidence — a corpus count, a `file:line`, or an issue
number — so priority can be argued from data rather than from intuition
([ADR-0018](adrs/adr0018-genapi-conformance-over-convenience.md)).

## REL — Ship 0.3.0 (roadmap Phase 0)

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| REL-01 | Fix GigE discovery on Windows APIPA networks (#57) | P0 | M | planned | Eight defects, see the table below |
| REL-02 | Tag and publish 0.3.0 (`v0.3.0`, `py-v0.3.0`) | P0 | S | blocked | Blocked on REL-01. All six version touchpoints already done on `main`; crates.io/PyPI still serve 0.2.8, so #35 and #45 are blocked on a tag, not on code |
| REL-03 | Ask #35, #45, #57 to retest against 0.3.0 | P0 | S | blocked | Blocked on REL-02. #57's reporter has JAI APIPA hardware and should confirm REL-01 before the tag |
| REL-04 | `.github/ISSUE_TEMPLATE/` asking for the artifacts that resolved #35/#45/#57 | P0 | S | planned | Model, OS, install source + version, `RUST_LOG=debug` trace, raw GenApi XML. The Python XML snippet given in #45 was wrong and had to be retracted — the template carries the correct one |

**REL-01 detail.** Reported in #57 (1–4), found alongside them (5–8):

| # | Defect | Evidence |
|---|---|---|
| 1 | APIPA NICs invisible on Windows | `if-addrs` 0.11.1 drops `169.254.x.x` on Windows unless the `link-local` feature is enabled (`sockaddr.rs:57-69`); the workspace enables no features (`Cargo.toml:43`). Linux/macOS enumerate link-local already |
| 2 | MAC parsed from the wrong offset | `parse_discovery_payload` reads `payload[12..18]`; the MAC is at `[10..16]` (`crates/viva-gige/src/gvcp.rs:484-497`). The doc-comment table at `:458-483` is independently wrong from offset 18 onward |
| 3 | One bad interface aborts all discovery | `res??` in the `join_set` drain (`gvcp.rs:415-421`) discards results already collected from healthy interfaces; `parse_discovery_ack` also errors on any unexpected opcode (`gvcp.rs:440`), so one stray GVCP packet kills the whole call |
| 4 | Studio scans loopback | `studio/apps/viva-studio-tauri/src-tauri/src/backend/embedded.rs:588` uses `discover_all`; `viva-service` uses `discover` for the same job |
| 5 | `Iface::from_ipv4` can resolve to a different address | `Iface::from_system` keeps the **last** IPv4 on a NIC (`crates/viva-gige/src/nic.rs:110-120`) — exactly the APIPA + stale-DHCP case |
| 6 | Windows interface index is a guess | `iface_name_to_index` returns enumeration position + 1 (`nic.rs:74-87`), not a real interface index; it feeds multicast joins |
| 7 | `DeviceInfo` drops serial and user-defined name | Present in the ACK at offsets 216 and 232, never parsed (`gvcp.rs:484-525`). Studio consequently shows the MAC in its `serial` field |
| 8 | The fake reproduces the same +2 MAC shift | `crates/viva-fake-gige/src/gvcp_server.rs:134-141`. The one discovery assertion in the suite is a disjunction that checks no values: `assert!(fake.model.is_some() \|\| fake.manufacturer.is_some())` (`crates/viva-genicam/tests/fake_camera.rs:88-91`) |

## TC — Transport conformance (roadmap Phase 1, ADR-0019)

The GVCP/GVSP audit ADR-0018 never reached. Opcodes cross-checked against
`../aravis/src/arvgvcpprivate.h:267-282`.

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| TC-01 | Handle `PENDING_ACK` (0x0089) | P0 | M | planned | `OpCode::from_ack` (`crates/viva-gencp/src/lib.rs:62-70`) knows only the four GenCP acks, so a pending-ack yields `UnknownOpcode` → hard failure. `transact_with_retry` does not retry on decode failure (`gvcp.rs:661`). Cameras use this for flash writes and mode changes |
| TC-02 | `ACTION_COMMAND` collides with `READREG` | P0 | S | planned | `crates/viva-gige/src/action.rs:19-21` defines 0x0080/0x0081; `viva-gencp/src/lib.rs:39` uses 0x0080 for `ReadRegister`. Two commands cannot share an opcode |
| TC-03 | Event-channel opcode is not a GVCP opcode | P0 | M | planned | `crates/viva-gige/src/message.rs:20` keys on 0x000D and rejects everything else; GVCP events are 0x00C0–0x00C3. No EVENT_ACK is ever sent back to the device |
| TC-04 | Spec-derived golden-byte fixtures for the fakes | P0 | M | planned | The structural fix. Third occurrence of fake-and-client sharing one wrong assumption (SCPS overhead, unaligned READMEM, #57's MAC). Assert the fake's wire bytes against literal spec-derived arrays, independently of the client parser |
| TC-05 | Accept the payload types cameras actually send | P1 | M | planned | `parse_leader` rejects everything but 0x01 (`crates/viva-gige/src/gvsp.rs:255-283`), so Image Extended Chunk (0x4001) — the normal chunk delivery mechanism — never opens a frame. Packet format 0x04 (single-packet block) is likewise `Unsupported` |
| TC-06 | Chunk trailer layout is self-consistent, not conformant | P1 | M | planned | `parse_chunks` decodes front-to-back big-endian (`gvsp.rs:112-143`) while `crates/viva-genicam/src/chunks.rs:114-142` reads values little-endian. Real cameras append backward-scanned `[data][id][len]` tuples. The fake emits our layout, so the round trip proves nothing |
| TC-07 | Implement ACTION and EVENT in `viva-fake-gige` | P1 | M | planned | Neither is implemented by the fake, so TC-02 and TC-03 have no test vehicle |
| TC-08 | Reconcile packet-size accounting | P1 | S | planned | `best_packet_size` subtracts 42 (incl. Ethernet L2, `crates/viva-gige/src/nic.rs:199-205`); the receiver and the fake subtract 36 (`crates/viva-genicam/src/stream.rs:399-407`). 6 bytes of permanent slack |
| TC-09 | Verify FORCEIP's opcode against hardware | P1 | S | planned | aravis labels 0x0004/0x0005 `BYE_CMD`/`BYE_ACK`; Wireshark's dissector labels the same pair `FORCEIP`. `gvcp.rs:31,33` assumes the latter, and only our own fake has ever answered it |
| TC-10 | `viva-gencp::encode_cmd` is dead *and* divergent | P2 | S | planned | It writes a different header shape than `viva-gige` actually uses and has no callers outside its own tests (`crates/viva-gencp/src/lib.rs:179-188`) |
| TC-11 | `mtu()` returns a hardcoded 1500 off Linux | P2 | M | planned | `crates/viva-gige/src/nic.rs:173-193`. Jumbo frames can never be selected on macOS or Windows |

## DX — Diagnostics loop (roadmap Phase 2)

Every fix so far was diagnosed from a user-supplied artifact, and the library
offers no supported way to produce one.

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| DX-01 | `viva-camctl xml` — dump raw GenApi XML without building a nodemap | P0 | S | planned | #45's reporter was told camctl could already do this. It cannot. `fetch_xml` exists at `crates/viva-camctl/src/common.rs:84` but is only reachable via the nodemap path at `:107-109` — which is what fails on the cameras we most need the XML from |
| DX-02 | `viva-camctl report` — one-command diagnostic bundle | P1 | M | planned | Discovery ACK raw bytes, bootstrap registers, XML, skipped nodes, versions, OS/NIC inventory. What we ask for by hand in every issue thread |
| DX-03 | Surface `NodeMap::skipped()` beyond a log line | P1 | M | planned | Consumed today only by `viva-genicam/src/lib.rs:986-1021` and the corpus test; not reachable from camctl, Python or Studio |
| DX-04 | Report discovery fields we currently discard | P1 | S | planned | Serial number and user-defined name (REL-01 #7). Users identify cameras by the label on the case, not by MAC |

## SR — Streaming reliability (roadmap Phase 3)

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| SR-01 | Make connect fallible on malformed camera XML | P0 | S | done | 0.3.0: `From` replaced by `TryFrom`; per-node failures land in `NodeMap::skipped()` instead of aborting |
| SR-02 | SCPS read-back after write; stride follows negotiated value | P0 | M | planned | Cameras clamp the requested packet size. No read-back exists today |
| SR-03 | Per-stream ephemeral ports + `source_filter` enforcement | P0 | M | planned | `source_filter` is configured at `crates/viva-genicam/src/stream.rs:270-274` and never read, because `PacketSource::recv` discards the source address (`stream.rs:51-61`) |
| SR-06 | Fix unsound `unsafe impl Sync` on `MockUsbTransfer` (`RefCell` → `Mutex`) | P0 | S | planned | Soundness bug at `crates/viva-u3v/src/usb.rs:99-101`. The type is `pub` in a published crate and satisfies `UsbTransfer`, so it can legally be moved onto a `spawn_blocking` thread |
| SR-04 | Wire packet resend end-to-end, or delete it | P1 | L | planned | `ResendPlanner` (`gvsp.rs:490`), `request_resend` (`gvcp.rs:1004`) and `resend_enabled` have zero production callers. `README.md:26` advertises resend as shipping — pick one |
| SR-05 | Library-owned heartbeat keepalive | P1 | M | planned | Consumers lose CCP after ~3 s idle. The only keepalive in the workspace is in `viva-service` (`crates/viva-service/src/device.rs:252-273`), above the transport |
| SR-07 | Honest streaming telemetry | P1 | M | planned | `resends`, `resend_ranges`, `late_frames`, `pool_exhaustions`, `backpressure_drops` are permanently zero — nothing in the live path calls their recorders. Every GVSP parse error is swallowed at `trace` and counted nowhere (`stream.rs:630-636`) |
| SR-08 | IGMP leave on multicast stream teardown | P2 | S | planned | `join_multicast_v4` is called (`nic.rs:322`); nothing ever leaves the group |
| SR-09 | Delete or wire the unused reassembly stack | P2 | M | planned | `Reassembler`, `FrameAssembly`, `FrameQueue`, `BufferPool` and `GigeDevice::negotiate_stream` have no callers; `StreamBuilder::build` re-implements `negotiate_stream` inline (`stream.rs:220-266`). Tenet 4 (YAGNI) names this exact case |

## GA — GenApi conformance, round 2 (roadmap Phase 4)

Supersedes the previous `XML` section. Corpus counts are from the 30 documents
fetched by `scripts/fetch-xml-corpus.sh`, reproducible with a tag frequency
count over `fixtures/vendor-xml/*.xml`.

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| GA-01 | Parse and honour `pInvalidator` | P0 | L | planned | **11 795 occurrences in 27 of 30 documents** — the largest unparsed element in the corpus by an order of magnitude. Invalidation currently fires only on writes made through the NodeMap |
| GA-02 | Record unknown node tags instead of dropping them | P0 | S | planned | `is_node_tag` (`crates/viva-genapi-xml/src/lib.rs:839-859`) gates the isolation path, so an unlisted tag falls to `_ => skip_element` at `:981` and never reaches `XmlModel::skipped`. `<Register>` (35 uses, 9 docs) vanishes this way. A hole in the guarantee CLAUDE.md states, and one `EXPECTED_SKIPS` can never catch |
| GA-03 | `<pSelected>` is parsed with the direction inverted | P1 | M | planned | 1 012 occurrences in 26 docs. The standard puts it on the *selector* naming the *selected* feature (`fixtures/vendor-xml/Basler_acA1600_20gm.xml:213`); `crates/viva-genapi-xml/src/parsers/mod.rs:116-131` treats the text as this node's selector, registering invalidation edges backwards |
| GA-04 | Activate `pMin`/`pMax` | P1 | M | planned | 885 / 583 occurrences, 30 / 29 docs. Parsed (Integer only), stored on `IntegerNode`, registered as dependencies (`nodemap.rs:1489-1494`) — and never read. Range checks use the static limits at `nodemap.rs:271-273`, so a dynamic limit is silently ignored |
| GA-05 | Honour `Cachable` and `PollingTime` | P1 | L | planned | 2 221 / 27 docs and 211 / 23 docs, both unparsed. Every readable node is cached until a dependency is written |
| GA-06 | Enforce `pIsLocked` in the setters | P1 | S | planned | 966 occurrences, 26 docs. Parsed and consulted by `effective_access_mode` (`nodemap.rs:580-603`) but not by `set_integer`/`set_float`/`set_enum`/`set_bool`, which only check the static access mode |
| GA-07 | Honour `<Slope>` on Converter min/max propagation | P1 | S | planned | Was "Unconfirmed, P2" (old XML-05). Confirmed: **437 occurrences in 24 of 30 documents**. A decreasing converter inverts min and max; we propagate neither |
| GA-08 | Parse `ImposedAccessMode` | P1 | S | planned | 1 382 occurrences, 23 docs, unparsed |
| GA-09 | Support the `<Register>` node type | P2 | M | planned | 35 occurrences, 9 docs. Depends on GA-02 to become visible in the first place |
| GA-10 | Parse `pInc` | P2 | S | planned | 175 occurrences, 20 docs. Static `Inc` is honoured; the dynamic form is not |
| GA-11 | Corpus test evaluates against zeros | P1 | M | planned | The `viva-genapi` stage uses `NullIo`, which returns zeros (`crates/viva-genapi/src/io.rs:18-28`). "30 documents, 21 785 nodes evaluated" proves nothing panics, not that any value is right |
| GA-12 | GenApi chunk adapter | P2 | L | planned | Replaces the hardcoded 4-entry table at `crates/viva-genicam/src/chunks.rs:114-142`. `ChunkID` appears 89 times in 9 docs and is ignored; `RegisterIo` has no port abstraction to address a chunk port with |
| GA-13 | Support negative `<Address>` (chunk-relative offsets) | P2 | M | planned | Confirmed: `Baumer_HXG20` `ChunkImageLength`; the sole `EXPECTED_SKIPS` entry in both corpus tests. Needs a signed `AddressTerm::Fixed` plus a chunk base |
| GA-14 | `Streamable` — no persistence/save-load feature exists | P2 | L | planned | 1 405 occurrences, 13 docs. Feature-set save/restore is a standard GenApi capability we do not offer |
| GA-15 | Fall back to lossy decoding for non-UTF-8 GenApi XML | P2 | S | planned | Unconfirmed. `fetch.rs` `String::from_utf8` is strict, so an ISO-8859-1 document fails the connect outright |
| GA-16 | Accept uppercase `0X` hex prefix in `parse_u64`/`parse_i64` | P2 | S | planned | Unconfirmed. Hex digits are already case-insensitive, only the prefix is not. No corpus document uses it |
| GA-17 | Inline `<IntSwissKnife>` as a register address term | P2 | M | planned | Unconfirmed: no corpus document nests one inside a register, but the schema allows it. Today `skip_element` drops the term silently |
| GA-18 | Decide the sign of scaled `<Float>` and `<Enumeration>` payloads | P2 | S | planned | GenICam declares no `<Sign>` for either; 0.3.0 kept the historical signed reading. Worth confirming against hardware before changing |

## API — 0.4.0 consolidation (roadmap Phase 5, breaking)

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| API-01 | Single frame-reassembly implementation | P1 | L | planned | See SR-09 |
| API-02 | Curated public surfaces | P1 | L | planned | Kill blanket `pub mod` (viva-gige, viva-u3v, viva-service, viva-camctl, viva-fake-gige). Re-export unnameable types: `ConverterNode`, `IntConverterNode`, `StringNode` and `PredicateRefs` are public payloads of public enum variants but are not re-exported — matchable, unnameable |
| API-03 | Error source chains (no `String` payloads); `#[non_exhaustive]` policy | P1 | M | planned | Also bump `thiserror` 1 → 2 while doing it |
| API-04 | Dedupe viva-service vs viva-service-u3v behind a `StreamSource` trait | P1 | L | planned | ~60% copy-paste today |
| API-05 | Typed accessors on `Camera` | P1 | M | planned | `Camera::get`/`set` are string-only (`crates/viva-genicam/src/lib.rs:195,238`) while `NodeMap` already has `get_integer`/`get_float`/`get_bool`/`get_enum`. Floats round-trip through `to_string`/`parse` |
| API-06 | Type-gate the GigE-only methods on `Camera<T>` | P1 | M | planned | `configure_events` (`lib.rs:464`) and `configure_stream_multicast` (`lib.rs:576`) write GVCP bootstrap registers but are defined on the generic `Camera<T>`, so they compile — and silently misbehave — against a U3V camera |
| API-07 | Stop exposing node cache internals | P2 | S | planned | `SkNode.cache`, `ConverterNode.cache`, `IntConverterNode.cache`, `StringNode.cache` are `pub RefCell<Option<(_, u64)>>` (`crates/viva-genapi/src/nodes.rs:329,395,424,441`), publishing the internal generation counter; `IntegerNode.cache` is `pub(crate)`. Pick one |
| API-08 | `Camera` get/set inconsistencies | P2 | S | planned | `get` on a Category returns `Ok("")` (`lib.rs:232`); `set` on a Command executes and ignores the value (`lib.rs:279-282`); Converter/IntConverter are readable but not writable through `Camera` although `NodeMap::set_converter` exists |
| API-09 | Missing `NodeMap`/`Node` accessors | P2 | S | planned | No `Node::unit()`/`min()`/`max()`/`inc()` though the data is stored; `set_string` takes `&self` while every other setter takes `&mut self`; `get_integer` handles `IntConverter` and `SwissKnife` but not `Converter`; `enum_entries` sorts and dedups, discarding the XML order GUIs need |
| API-10 | Fakes import transport-crate register constants; viva-pfnc as single `PixelFormat` authority; workspace lints (`missing_docs`, `unreachable_pub`) | P2 | M | planned | `viva-zenoh-api` defines a second `PixelFormat` bridged by `crates/viva-service/src/pixel_format.rs:7` |
| API-11 | U3V/GigE facade parity | P2 | L | planned | U3V has no events, chunks (`chunks: None` hardcoded, `stream.rs:829`), stats, time sync, or stream params; its builder takes a `Camera` where GigE's takes a device, and its `build()` is sync where GigE's is async |

## SVC — Services (roadmap Phase 6)

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| SVC-01 | Announce cadence exceeds Studio's expiry window | P1 | S | planned | GigE re-announces every `discovery_timeout` + `discovery_interval` ≈ 7 s (`crates/viva-service/src/config.rs:15-20`); Studio expires at 6 s (`studio/apps/viva-studio-tauri/src-tauri/src/commands/device.rs:339`) and `docs/studio/zenoh-api.md:17-32` documents 2 s. Devices can flicker |
| SVC-02 | U3V introspection is typeless | P1 | M | planned | `U3vDeviceHandle` never overrides `DeviceOps::get_feature_state`, so the degraded default (`crates/viva-service/src/device.rs:42-54`: `access_mode: "RW"`, `kind: "Unknown"`, no ranges) serves every U3V camera |
| SVC-03 | U3V service streaming never configures the SIRM | P1 | M | planned | `crates/viva-service-u3v/src/device.rs:39-48` builds `U3vStream::new` directly with hardcoded 256-byte leader/trailer, bypassing `U3vDevice::open_stream` (`crates/viva-u3v/src/device.rs:159-183`) which reads the SIRM and enables streaming |
| SVC-04 | U3V service is single-device with no lifecycle | P2 | M | planned | Takes `devices[0]` once, never re-scans, never publishes disconnect (`crates/viva-service-u3v/src/main.rs:247-253`); GigE has a full discovery loop |
| SVC-05 | GigE service clap name is still `genicam-service` | P2 | S | planned | `crates/viva-service/src/config.rs:6`, pre-rebrand |

## CI

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| CI-01 | Per-crate feature matrix (`cargo hack --each-feature`) | P1 | M | planned | **Restated** — the previous wording ("U3V is never compiled in CI") was inverted. `viva-camctl`/`viva-pygenicam`/`viva-service-u3v` all enable `u3v-usb`, so unification builds it. The real gap: `viva-genicam` with its *own* default features (`[]`) is never verified |
| CI-02 | Lint and test `viva-pygenicam` | P1 | S | planned | It is excluded from the root workspace (`Cargo.toml:17`), so `cargo test --workspace` and `cargo clippy --workspace` never touch it; `python.yml` only builds wheels and runs pytest. It is the crate two of three reporters use |
| CI-03 | MSRV job | P2 | S | planned | `rust-version = "1.88"` is declared and never checked |
| CI-04 | Windows wheel is built and published but never tested | P2 | M | planned | `python.yml` test matrix is ubuntu + macos-14 only. #57 is a Windows issue |
| CI-05 | Scope release.yml / publish-docs.yml permissions to the jobs that need write | P2 | S | planned | |
| CI-06 | Drop hardcoded `sleep 30` in publish-crates.yml | P2 | S | planned | cargo ≥ 1.66 waits for the index |
| CI-07 | python.yml path filter misses viva-gencp and root Cargo.toml/Cargo.lock | P2 | S | planned | |
| CI-08 | `cargo doc` in ci.yml lacks `--all-features` + `RUSTDOCFLAGS` | P2 | S | planned | |
| CI-09 | Fuzz the packet and XML parsers | P2 | L | planned | Both consume untrusted network input |
| CI-10 | cargo-semver-checks on release tags | P2 | M | planned | |

## DOC — Documentation

Several of these are *wrong* documentation rather than missing documentation,
which is why they are not all P2.

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| DOC-01 | Book chapters document APIs that never existed | P1 | M | planned | `book/src/crates/viva-genapi.md:21-36` uses `viva_genicam::Client`, `Client::connect`, `cam.get_f64`; `book/src/crates/viva-gige.md:103-110,234-254` uses `viva_gige::control::ControlClient`, `net::InterfaceSelector`, `discovery::discover_on`; `book/src/tutorials/{genapi-xml,registers,streaming}.md` use `genicam::Context::new()` / `open_by_ip`. None of these exist |
| DOC-02 | Python docs promise features the bindings do not expose | P1 | S | planned | `book/src/python.md:38` advertises chunks, events and time sync — none are exposed. `python/control.md:26` claims vendor-alias fallback for the typed setters (only `time_calibrate`/`configure_events` use aliases). `python/control.md:51` and `python/api.md:146` claim one `except vg.GenicamError` catches everything (`PanicException` subclasses `BaseException`; `unsendable` handles raise bare `RuntimeError`). `python/streaming.md:12` claims other Python threads stay runnable |
| DOC-03 | README overstates streaming | P1 | S | planned | `README.md:26` claims "GVSP with packet resend … and backpressure"; see SR-04. README also omits `viva-pygenicam` from the workspace layout, and its quick-start snippet imports `Camera` without using it |
| DOC-04 | `quick-start.md` is factually wrong | P1 | S | planned | `:6` claims MSRV 1.75 and a `rust-toolchain.toml` that does not exist (actual: 1.88, edition 2024); `:31` has a broken code fence (`\ n`) |
| DOC-05 | Book: empty chapters and broken links | P2 | M | planned | `errors-logging.md` and `contributing.md` are 24-line stubs; `api.md` rustdoc links use hyphens where rustdoc emits underscores; `welcome.md:17` still points at the external genicam-studio repo; `crates/README.md:22,24` has pre-rebrand paths and lists 6 of 15 crates |
| DOC-06 | Book: fill the three crate pages; add the missing ones | P2 | L | planned | No page exists for viva-u3v, viva-genapi-xml, viva-pfnc, viva-sfnc, viva-zenoh-api, the services, viva-camctl or the fakes. USB3 Vision has no tutorial anywhere |
| DOC-07 | `book/book/` is a committed build output | P2 | S | planned | Sitting next to `book/src/`; `.gitignore` already lists `book/book` |
| DOC-08 | Stale cross-references | P2 | S | planned | `docs/design.md:16,121` still call 0.3.0 the consolidation release (now 0.4.0); `docs/studio/zenoh-api.md:22` shows `api_version: 1` while `API_VERSION = 2`; `studio/apps/viva-studio-tauri/src-tauri/src/backend/embedded.rs:619` cites `docs/handoffs/`, which does not exist; `.claude/skills/release/SKILL.md` lists five version touchpoints where CLAUDE.md lists six |
| DOC-09 | `studio/README.md` is stale throughout | P2 | M | planned | Still "GenICam Studio"; references `AGENTS.md`, `crates/genicam_xml_model` and `crates/*_wasm` (actual: `viva_xml_model`, `viva_streamer`, no wasm crate); says live device connection is "stubbed" while `backend/embedded.rs` implements it in 859 lines |
| DOC-10 | Add missing `description` to viva-service-u3v and viva-fake-u3v Cargo.toml | P2 | S | planned | |
| DOC-11 | Python `frame.ts_host` is silently wrong for GigE | P1 | S | planned | `build_gige_stream` always passes `Some(time_sync)` (`crates/viva-pygenicam/src/stream.rs:96-99`), but `time_calibrate` is not exposed and `TimeSync::to_host_time` with no origin returns `SystemTime::now()` (`crates/viva-gige/src/time.rs:308-312`). Always wall-clock, never device-mapped — worse than `None`, because it looks like data. Fix the behaviour or the docs, not neither |

## ST — Studio

| ID | Task | Priority | Size | Status | Notes |
|----|------|----------|------|--------|-------|
| ST-03 | Revive studio e2e against in-repo service/fake (drop aravis-from-source) | P1 | L | planned | |
| ST-06 | Release packaging pipeline: DMG (macOS), AppImage (Linux), MSI (Windows) on tag push | P1 | L | planned | M11 |
| ST-07 | Bundle viva-service binary as Tauri sidecar (auto-start if no external service) | P1 | M | planned | M11 |
| ST-01 | Modernize studio crates to edition 2024 + workspace-dep inheritance | P2 | M | planned | |
| ST-02 | Retire apps/viva-mock-service in favor of viva-service + viva-fake-gige | P2 | M | planned | |
| ST-08 | Frame annotation rendering engine (frame ID/timestamp/FPS burned into BMP stream) | P2 | M | planned | M11 |
| ST-09 | Annotation toggle in viewer toolbar | P2 | S | planned | M11 |
| ST-10 | Recording playback engine: load .gsr, play/pause, frame step, speed, seek | P2 | L | planned | M12 |
| ST-11 | Recording export to TIFF stack / uncompressed AVI | P2 | M | planned | M12; interop with ImageJ, MATLAB |
| ST-12 | Auto-update via Tauri v2 updater plugin | P2 | M | planned | M12 |
| ST-13 | Studio performance benchmarks in CI | P2 | M | planned | M12; fail on >10% regression |
| ST-14 | Embedded backend has no U3V discovery | P2 | M | planned | `backend/embedded.rs:122-123` reads only the GigE cache despite `u3v-usb` being enabled |
| ST-04 | Refresh stale studio docs | — | S | done | Done by the monorepo import |
| ST-05 | Delete stale package-lock.json | — | S | done | Done by the monorepo import |
