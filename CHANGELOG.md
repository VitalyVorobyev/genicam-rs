# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`Mono12` and `Mono14`** — the two pixel formats the Micro-Epsilon
  scanCONTROL 850050 advertises that #93 did not cover. Its `PixelFormat`
  enumeration offers ten formats; eight were named.

### Fixed

- **Chunk data can be enabled against the fake camera, so the chunk path has an
  end-to-end test for the first time.** `viva-fake-gige` declared
  `ChunkModeActive` and `ChunkEnable` as `<Integer>`; SFNC defines both as
  IBoolean, and of the 23 vendor-corpus documents that declare
  `ChunkModeActive`, 23 use `<Boolean>` over a backing `<IntReg>` and none uses
  `<Integer>`. `Camera::configure_chunks` calls `set_bool`, which is right, so
  the call failed with a type mismatch and the only camera the project can test
  against could not turn chunks on. The fake is fixed, not the library. The
  consequence was larger than one command: **no test anywhere exercised chunk
  acquisition**, which is why the trailer-offset defect above was found in a
  user's log rather than in CI (backlog `TC-19`).
- **A pixel format we cannot name is no longer treated as one byte per pixel.**
  Bits 23-16 of every PFNC code are the pixel's bit depth, so
  `PixelFormat::bytes_per_pixel` now derives a size for `Unknown(code)` instead
  of returning `None`. That `None` mattered because essentially every caller
  writes `.unwrap_or(1)`: the Windows receive path truncated frames to
  `width * height` bytes and the U3V builder under-allocated its payload buffer
  by the same factor. It still returns `None` when the depth is not a whole
  number of bytes — `Mono12Packed`, `Mono10Packed`, `YUV411Packed` and the two
  packed Bayer formats all declare 12 bits, and eleven of the 37 vendor-corpus
  documents offer at least one of them. Rounding those up would overstate a
  frame by a third, which a length check reads as a *short* payload; a
  confidently wrong size is worse than an absent one (backlog `SR-11`).
- **The Zenoh bridge no longer truncates a frame whose format it cannot name.**
  It sized every frame through `pfnc_to_zenoh`, which collapses anything the
  wire enum does not carry to `Unknown` — and `Unknown` reports 1.0 bytes per
  pixel. A `Coord3D_ABC32f` profile was therefore cut to **one twelfth of
  itself** and published as valid, with the "trimming trailing bytes" warning
  emitted once and every later frame corrupted in silence. Frames are now sized
  from the camera's own PFNC code, and a format with no whole-byte size is
  published unmodified with a warning naming it, because an expected length we
  cannot compute must not become a length we enforce. Not specific to 3D:
  `Mono10`, `Mono12` and `Confidence8` all took the same path (backlog `DC-01`).
  The arithmetic also moves from `f32` to integers, which no longer loses
  precision above 2^24 bytes.

- **The GVSP data trailer is read at the offset the specification gives it.**
  The trailer payload is eight bytes — `reserved(2) | payload_type(2) |
  size_y(4)` — and the chunk region starts after them. `parse_trailer` read two
  and handed the remaining six to the chunk parser, producing a
  `chunk header truncated remaining=6` warning on every frame: those six bytes
  *are* `payload_type` and `size_y`. Not cosmetic — with chunk mode on the
  six-byte prefix desynchronises every chunk that follows, so chunks could not
  decode on any conforming camera. The same two bytes were reported as the
  trailer's `status`, which both frame-reassembly paths test to reject a bad
  frame; they are reserved and always zero, so that check could never fire. The
  real GVSP status is at offset 0 of the packet header, where `parse_packet` was
  instead shifting it right by four, calling it a "payload type" and passing it
  to a parser that discarded it — so the status was examined nowhere in the
  receive path. `GvspPacket::Trailer` now carries `payload_type` and `size_y`.
  Found in the log attached to #70; `viva-fake-gige` was already emitting a
  correct trailer, making this the first case where the fake was right and only
  the client was wrong (#96).
- **GigE discovery reaches link-local cameras on Linux.** Discovery took the
  broadcast address from `if-addrs`' optional `broadcast` field, and on Linux an
  address configured without an explicit `brd` — a manually assigned
  `169.254.0.0/16`, typically — can report the interface address itself there,
  turning the broadcast into a unicast to self. It is now derived from the
  netmask, which is authoritative. Complements #57, which fixed enumeration of
  link-local addresses; even once enumerated, discovery was not leaving the
  host. Confirmed on hardware by @Katze719, whose Micro-Epsilon scanCONTROL
  8500-50 at `169.254.7.189` is now discovered automatically (#95).

## [0.3.1] - 2026-07-31

### Added

- **Six pixel formats used by Micro-Epsilon scanCONTROL 8500/8200 laser
  profile scanners** — `Mono10`, `Confidence8`, `Coord3D_C32f`,
  `Coord3D_AC16`, `Coord3D_AC32f` and `Coord3D_ABC32f`, with codes, names and
  `bytes_per_pixel` (#93, thanks @Katze719). The `Coord3D_*` group is the first
  3D point-cloud format family in `viva-pfnc`, and a profile scanner is a
  device class the project had had no report from.
- **`NodeMap::register_address(name, io)`** — resolves the device register
  address and length backing a feature, so a caller doing raw register I/O
  through `RegisterIo` does not have to reimplement GenICam addressing (#92,
  thanks @Katze719). Address terms, `<pIndex>` scaling and selector blocks
  resolve exactly as they do for `get_integer`. The motivating case is a
  file-transfer buffer, whose address the XML supplies through `<pAddress>` and
  which no typed accessor covers — the same gap that leaves `<Register>` nodes
  unsupported (backlog `GA-09`). The addressing types themselves stay private,
  so that model can still change without a breaking release.
- **`pip install viva-genicam` now installs the `viva-camctl` CLI.** We tell
  reporters to run `viva-camctl report` or `viva-camctl xml` when we cannot open
  their camera — right after telling them to pip-install — and the wheel shipped
  no such command (#45). It is linked into the extension module rather than
  staged as a per-target binary, so it is present in every wheel *and* in an
  sdist build, with no Rust toolchain needed at the user's end.

  `viva-camctl` gained a library entry point (`viva_camctl::run(argv) -> u8`) so
  the binary and the console script run the same code; the binary's `main` is now
  three lines.
- `viva-genicam` example `fetch_xml` — fetch a camera's GenApi XML through
  `fetch_and_load_xml`, print its schema version, top-level features and the
  nodes the parser had to skip. It is the layer below `connect_gige`, so it
  works on a camera that cannot be opened.

### Fixed

- **GigE acquisition works on a Windows APIPA link.** Streaming resolved the
  receiving interface by passing the *camera's* IP to `Iface::from_ipv4`, which
  expects a host interface address — so on any real network it failed with
  `no interface with IPv4 <camera>`, and it only ever appeared to work against
  the loopback fake, where the two addresses coincide. `Iface::from_remote_ipv4`
  now asks the OS which local interface serves the camera. The GVSP socket also
  binds to that interface rather than `0.0.0.0`, Windows reads its real MTU via
  `GetIfEntry2`, and a `#[cfg(windows)]` blocking receive thread delivers frames
  the async path did not. Reported, fixed and confirmed on hardware by
  @InsuJeong496 (#70, #72) — a JAI on a 169.254.0.0/16 link now streams
  2048×1536 at 75 Mbit/s with no drops.

  This is the fix the release waited on. Every gate CI can run was green on
  merge day; none of them says anything about a `#[cfg(windows)]` path against a
  camera, so 0.3.1 was held until a user with the hardware said it worked.
- **A slow GVCP command is no longer executed twice** (#91, thanks @Katze719).
  Each retry allocated a *fresh* request ID, so a device answering after the
  first receive deadline replied with the ID we had stopped listening for; the
  late acknowledgement was discarded as unmatched and the command was sent
  again. For a non-idempotent operation — a file write, a flash commit — that
  means it ran a second time. Retries now keep the original request ID, so a
  delayed answer still matches its transaction, and stale or unrelated
  acknowledgements are discarded while waiting rather than ending the wait.
  Covered by a UDP regression test that delivers a stale acknowledgement ahead
  of the real reply and asserts the command is not resent.
- **An idle GigE session no longer loses control of the camera.** A GigE Vision
  device revokes control privilege if it receives no GVCP command within
  `GevHeartbeatTimeout` (3 000 ms is typical), and GVSP image traffic does not
  count towards it — so a camera could be streaming at full rate, or simply
  sitting at a Python prompt, and the next `set()` would fail with
  `ACCESS_DENIED`. The library refreshed that timer nowhere: `connect_gige`
  claimed privilege and nothing kept it. `GigeRegisterIo` now owns the
  keepalive. It reads the device's own `GevHeartbeatTimeout` and pings at a
  quarter of it, so a device asking for a shorter window gets one, and it stops
  when the transport is dropped — holding a `Camera` is all a caller has to do.

  This also removes the three app-layer copies that had grown around the gap
  (`viva-service`, `viva-camctl stream`, and the studio's embedded backend, the
  last two added in #72). Each contended for the application's *camera* lock
  rather than the device lock, which is why `viva-service` needed
  `pause_heartbeat`/`resume_heartbeat` around reconnection; the transport-owned
  keepalive needs no such coordination and that API is gone.
- The `get_set_feature` example never got or set a feature. Both the README and
  the book named it as the get/set template, and it fetched XML and ended on
  `println!("Stub: would map register ... to a GenApi feature")`. It now uses
  `connect_gige` and `Camera::{get,set,enum_entries}` and takes
  `--name`/`--value`.
- `demo_fake_camera` wrapped every feature access in `spawn_blocking`, on the
  grounds that "block_on can't nest in async". `GigeRegisterIo::{read,write}`
  have guarded that with `block_in_place` since before the comment was written,
  so the zero-hardware demo was teaching a wrapper nobody needs.

### Changed

- **CI no longer lets one failing gate hide the others.** `ci.yml` and
  `studio-ci.yml` both ran fmt → clippy → test as sequential steps in a single
  job, so the first failure masked everything after it. On #72 that hid a real
  streaming regression for a day — `cargo test` never ran once across eight
  contributor commits. Formatting and `cargo deny` moved to their own job, and
  the remaining gates run even when an earlier one fails.
- **`viva-pygenicam` is linted.** It sits outside the root workspace, so nothing
  in CI had ever formatted or clippy-checked it; it had drifted out of rustfmt
  in 35 places, carried two clippy findings, and its separate `Cargo.lock` went
  stale for a whole PR unnoticed. `python.yml` now runs `cargo fmt --check` and
  a `--locked` clippy over it.
- `GigeDevice` gained `heartbeat_timeout_ms()` and `ping_control_channel()`, and
  `gvcp::consts` gained `HEARTBEAT_TIMEOUT` (`0x0938`) and
  `CCP_CONTROLLER_BITS`. `ping_control_channel` returns `Ok(false)` rather than
  an error when the device reports privilege cleared: the register read
  succeeded, and losing privilege to another controller is an answer, not a
  transport failure.
- `viva-fake-gige` can enforce the heartbeat rule (`--enforce-heartbeat`,
  `FakeCameraBuilder::enforce_heartbeat`) and report a custom
  `GevHeartbeatTimeout`. It is off by default so that unrelated tests are not
  sensitive to a multi-second stall on a loaded CI runner. Without it a fake
  camera cannot tell a working keepalive from a missing one, which is how this
  defect survived three reimplementations of the loop above it.
- **The book documented an API that never existed, and now cannot again.**
  Chapters showed `viva_genicam::Client::connect`, `cam.get_f64`,
  `viva_gige::control::ControlClient`, `net::InterfaceSelector`,
  `genicam::Context::new` and an example called `stream_basic` — so the first
  snippet a reader copied failed to compile. The `viva-gencp` chapter was
  invented end to end: not one of `Request`, `Reply`, `Status`, `bitops`,
  `chunk::ChunkPlan` or `helpers::read_u32` exists. Every Rust snippet in the
  book is now an mdBook `{{#include}}` of an anchored region in
  `crates/viva-genicam/examples/`, which `cargo clippy --workspace
  --all-targets` compiles, so a snippet cannot drift from the API again without
  failing a gate. Enforced by `crates/viva-genicam/tests/book_includes.rs`
  rather than by mdBook, which exits 0 on a missing include file and renders a
  missing anchor as an empty code block with no diagnostic at all.
- **Prose claims corrected against the code.** The Python docs advertised
  chunks, events and time sync, none of which the bindings expose, and promised
  a vendor-alias fallback in `set_exposure_time_us` that does not exist. The
  streaming tutorial taught readers to watch the `resends` statistic, which
  nothing in a live stream increments — `resends=0` means "not implemented",
  not "none were needed". `GevFirstURL` is at `0x0200`, not `0x0000`.
  `viva-camctl --json` is a top-level flag and every example placed it after the
  subcommand, where clap rejects it. Two of `book/src/api.md`'s five rustdoc
  links were 404s on the published site.

## [0.3.0] - 2026-07-30

The GenApi layer now implements the GenICam formula language and register
address model rather than plausible approximations of them. **Every user on
0.2.x should upgrade**: 27 of the 30 real camera descriptions in our vendor
corpus could not be opened at all on 0.2.8, and those that could were reading
the wrong registers.

This release grew out of a Hikrobot MV-CS050-10GC report (#35) whose author
attached a dump of their parsed model. Cross-checking it against the vendor
corpus showed that none of the defects it exposed were vendor-specific. See
[ADR-0018](docs/adrs/adr0018-genapi-conformance-over-convenience.md) for the
full audit and the policy that came out of it.

### Added

- **`viva-camctl report` — one command that produces a bug report.** The
  maintainer has no cameras, so every camera-specific defect fixed so far was
  diagnosed from an artifact the reporter assembled by hand. This collects all
  of them in one pass: the network interfaces *as the library sees them* (an
  interface we cannot enumerate is invisible to discovery no matter what the OS
  reports — that was #57), the discovery reply, 24 bootstrap registers with
  decoded values, the GenApi XML, and every feature the camera has that we
  could not build. Each section records either its findings or why it has none,
  so a camera that cannot be opened still produces a report — that camera being
  the one worth reporting. Output is plain text in one file, because that is
  what an issue tracker accepts as an attachment.
- **`viva-camctl xml` — dump a camera's GenApi XML.** The fetch existed but was
  reachable only through the nodemap path, which is precisely the step that
  fails on the cameras whose XML we most need. The reporter of #45 was told
  camctl could do this; it could not, so their BFS-PGE-31S4C-C description is
  still missing and four other models stood in for it. This command parses
  nothing.
- `Iface::list` and `Iface::all_ipv4` expose the library's own view of the
  host's interfaces, including every IPv4 on a multi-homed NIC.
- **The fake camera implements actions and events.** It acknowledges an
  `ACTION_CMD` addressed to its device and group keys and ignores one that is
  not, and emits `GEV_EVENT_START_OF_TRANSFER` per frame once
  `EventNotification` is `On` for it — both backed by real registers and real
  SFNC features. Per [ADR-0019](docs/adrs/adr0019-transport-conformance-and-spec-derived-fakes.md),
  a protocol feature the fake does not implement is not considered implemented;
  neither of these had ever been exercised, which is how both opcode errors
  survived. Golden-byte fixtures for `ACTION_CMD` and `EVENT_CMD` are asserted
  against literal spec-derived arrays, independently of our own encoder.

### Fixed

- **A camera that asks for more time is granted it** (#60). GVCP lets a device
  answer a slow command with `PENDING_ACK` (0x0089) instead of the real
  acknowledgement, requesting an extension. We did not know the opcode, so the
  decode failed and the command was reported as a protocol error — cameras use
  this for exactly the operations you least want to see fail, flash writes and
  mode changes. The controller now extends its own deadline and keeps waiting
  on the same request id rather than resending, since a resend could execute
  the command twice. Bounded by `MAX_PENDING_ACKS` and `MAX_PENDING_ACK_WAIT`.
  Handled in the GVCP layer rather than in `viva-gencp`: the U3V side of GenCP
  signals the same condition with status `0x8006`, so the two transports do not
  share a representation.
- **GigE discovery works on Windows APIPA networks** (#57). A camera and host
  both on IPv4 link-local addresses (`169.254.0.0/16`) were undiscoverable: the
  `if-addrs` `link-local` feature was off, and that crate drops `169.254.x.x`
  on Windows without it. Reported against a JAI FS-3200T-10GE-NNC.
- **The Discovery ACK MAC address is read from the right offset** (#57). It
  begins at payload offset 10, not 12; the last two bytes of the reported
  address were actually the first two of `SupportedIPConfiguration`. A camera
  at `00:0C:DF:06:5B:2F` was reported as `DF:06:5B:2F:C0:00`. The fake camera
  emitted the same shifted layout, so producer and consumer agreed with each
  other while both disagreed with the standard — the third occurrence of that
  pattern, and the reason for
  [ADR-0019](docs/adrs/adr0019-transport-conformance-and-spec-derived-fakes.md).
- **One failing interface no longer aborts the whole discovery** (#57). A bind
  failure, an `SO_BROADCAST` rejection, a send error or a receive error on one
  NIC now logs and continues, keeping the cameras already found. Unrelated GVCP
  traffic arriving on the discovery socket — a foreign request id, a `READREG`
  ack, an error status, a runt datagram — is ignored rather than failing the
  call. On Windows a loopback probe could raise `WSAECONNRESET` and discard a
  perfectly good camera found on the wired NIC.
- **`Iface::from_ipv4` reports the address that was asked for.** It resolved
  the interface by name and then kept whichever IPv4 the OS listed last, so a
  multi-homed NIC — a stale DHCP lease alongside a link-local address — could
  bind to a different address than the caller selected.
- **The interface index is the kernel's, not a guess.** `Iface` now takes the
  index from `if_addrs`, which on Windows reports the adapter's real `IfIndex`.
  The previous Windows path returned the enumeration position + 1, which feeds
  multicast joins.
- **A truncated Discovery ACK no longer panics.** Reading past the end of a
  short payload hit `advance out of bounds`; every field after the IP address
  is now optional.
- **`=` and `<>` are the GenICam equality operators.** We required the C
  spellings `==` / `!=` and rejected `=` outright with "use `==` for equality".
  27 of 30 corpus documents contain at least one formula we therefore refused —
  531 of 679 in `Basler_acA1600_20gm` alone. `==` and `!=` remain accepted.
- **`<IntSwissKnife>` and `<IntConverter>` evaluate in integer arithmetic.**
  Everything ran through `f64`, so `(HIGH << 32) | LOW` lost its low bits and
  `(IDX / 2) * 4` did not truncate. A wide shift could also overflow and panic
  outright in a debug build.
- **A node's numeric type comes from its element name.** `<Output>` is not a
  GenApi element — it appears zero times across the corpus, while
  `<IntSwissKnife>` appears 3 125 times — so every integer knife on every real
  camera was modelled as a float.
- **Register addresses are the sum of their terms.** `<Address>`, `<pAddress>`
  and `<pIndex>` all contribute; we kept one and logged
  `ignoring fixed <Address> in favour of <pAddress>` for the rest. 42 registers
  on the reporter's camera — including the stream-channel registers — resolved
  to the wrong address, as did 454 on `FLIR_ORX_10G_51S5M` and 418 on
  `PGR_BlackflyS_13Y3M`.
- **`<pIndex Offset="N">` is now parsed** (197 occurrences across AVT,
  Prosilica and the GenICam conformance document). It was ignored, so every
  indexed register read the block base.
- **`<StructReg>` shares all of its address terms with its entries.** Only
  `<Address>` was read, and a `<StructReg>` without one defaulted to address 0:
  860 nodes on the reporter's camera, and 33 of 38 struct registers in
  `PGR_Blackfly_13E4C-CS`. A `<StructReg>` with no address at all is now
  reported instead of silently pointed at register zero.
- **Registers are unsigned unless `<Sign>Signed</Sign>` says otherwise.** We
  always sign-extended, so any register with its top bit set read negative — a
  `GevCurrentIPAddress` of 192.168.1.160 came back as `-1062731360`, and mask
  comparisons such as `(CTRL | 0xFDFFFFFF) = 0xFFFFFFFF` could never be true.
  `<StructReg>` entries also declared the full `i64` range, which made a set bit
  sign-extend to `-1` and quietly broke every `(INQ = 1)` test.
- **Converter reads use `<FormulaFrom>`, writes use `<FormulaTo>`.** The two
  were swapped. An identity converter behaves the same either way, which is how
  this survived; a `FROM * 100` / `TO / 100` pair read back wrong by 10⁴.
- **Converters can be written.** `<FormulaTo>` was parsed and never evaluated,
  so converter features were read-only. Writes now bind `FROM` to the incoming
  value and `OLD` to the register's current contents, so read-modify-write
  formulas such as `FROM | (OLD & 0xffff0000)` preserve the bits they should.
- **`<Constant>` and named `<Expression>` elements are supported**, resolved by
  substitution when the node is built.
- **`LG()` and the `E` / `PI` constants are supported.** `LG` is used by every
  Baumer TXG description for its dB scale.
- **`connect_gige` no longer panics on a malformed model** (backlog SR-01).
  `NodeMap::from` called `.expect()` on remote input.
- **An event could still be stranded behind the event channel's receive lock**
  (TC-14, Codex review on #69). The previous fix added a recheck of the queue
  after acquiring the receive lock, which closed the wide window but left a few
  instructions of a narrow one: the first event of a decoded datagram was
  returned and the rest queued *after* the lock was released, so a second
  consumer could take the lock, find the queue empty, and block in `recv_from`
  on a device that had already sent everything it was going to send. Every
  event a datagram carries is now published in one step while the lock is held,
  and the caller pops its own result from the queue like any other consumer —
  the first/remainder split that had to be ordered correctly is gone.
- **A node type we do not implement now says so** (GA-02). `is_node_tag` gated
  the parser's isolation path, so a tag not on its list fell through to
  `skip_element` and vanished — no log line, no `XmlModel::skipped` entry, and
  nothing for the corpus tests to trip over. `<Register>`, 56 declarations
  across 14 corpus documents, disappeared exactly this way. An element is now
  taken for a node declaration if it carries a `Name`, which the GenApi schema
  requires on every node and on nothing else at that level.
- **`NodeMap` no longer discards the XML layer's losses.** `try_from_xml` built
  its skip list from scratch and dropped `XmlModel::skipped` on the floor, so
  any consumer holding a nodemap — camctl, Python, Studio — could not tell a
  feature we failed to parse from one the camera does not have. The two lists
  now travel together.
- **Two concurrent `EventSocket::recv` callers could strand an event.** Both
  could observe an empty pending queue, after which one would decode a
  multi-event datagram, queue the remainder and return; the other was already
  committed to `recv_from` and never rechecked, so it blocked on a device that
  had finished speaking while its events sat in the queue. Found by codex
  review on #68.
- **The fake camera tracked event notification globally.** `EventNotification`
  is selected by `EventSelector`, so enabling two events writes one address
  twice — and a single stored word let the second write silently disable the
  first. State is now per event id. Found by codex review on #68.
- **Action commands were indistinguishable from register reads** (#61).
  `ACTION_CMD` was sent as opcode 0x0080 — which is `READREG_CMD`. A camera
  receiving one saw a register read with a 24-byte payload, so an action either
  failed or was interpreted as six register accesses. The opcode is 0x0100
  (`ACTION_ACK` 0x0101), and the payload is 12 bytes — `device_key`,
  `group_key`, `group_mask` — extended to 20 by a 64-bit action time only when
  the scheduled-action flag (bit 7 of the GVCP flags byte) is set. The old
  encoder always sent the time, plus a stream-channel field and a reserved word
  that the format does not have. `ActionParams::channel` is gone.
- **The GVCP event channel could not have worked against any camera** (#62).
  Four defects stacked on top of each other:
  - The parser required opcode 0x000D, which is not a GVCP opcode. Events
    arrive as `EVENT_CMD` (0x00C0) or `EVENTDATA_CMD` (0x00C2).
  - It decoded the datagram as an acknowledgement, reading the 0x42 command key
    and flags byte as a status word. Every field after that was shifted: the
    event identifier was read out of the reserved word, and the timestamp out
    of the stream channel and block id.
  - One `EVENT_CMD` may pack several events (`length / 16`, or `/ 24` with
    GigE Vision 2.0 extended block IDs). Only the first was considered.
  - No `EVENT_ACK`/`EVENTDATA_ACK` was ever returned, so a device that set the
    acknowledge-required flag would retransmit indefinitely.

  All four are fixed, and `EventPacket::block_id` widens to `u64` to carry
  extended block IDs.
- **Message-channel bootstrap registers pointed at nothing.** The destination
  address and port were written to 0x0900_0200 and 0x0900_0204. The real
  registers are `GevMCDA` at 0x0B10 and `GevMCP` at 0x0B00 — 0x0900 is
  `GevNumberOfMessageChannels`, and its value was being used as a base, so
  every write landed roughly 150 MB into the device's register space. The port
  was additionally written as a bare `u16` into a 32-bit register, placing it
  in the high half.

### Removed

- **The raw event-enable fallback**, which toggled a bit in a "notification
  mask" at 0x0900_0300. No such bootstrap register exists: which events a
  device emits is selected through the GenApi `EventSelector` and
  `EventNotification` features. A camera exposing neither now gets an error
  naming what is missing, instead of a write to an invented address.
  `GigeDevice::enable_event_raw` is gone with it.

### Changed

- **A node we cannot build costs that one feature, not the whole camera.** The
  per-node isolation added to the XML layer in 0.2.8 now extends to nodemap
  construction: failures are recorded in `NodeMap::skipped()` and logged rather
  than aborting. `connect_gige` logs one summary line naming how many features
  are unavailable.
- The vendor corpus test now has a second stage in `viva-genapi` that builds a
  `NodeMap` from each document and evaluates every node in it. Parsing was only
  ever half the job — every defect above lived above the parser, where the old
  test could not see it. All 35 documents now pass: 31 737 nodes.
- The corpus grew to 35 documents with four FLIR Blackfly / Blackfly S
  descriptions contributed by @themightyoarfish on #45. Two of them contain
  `SerialPortSelectorValueToIndex`, the `IntSwissKnife` whose `=` equality
  operator is what made their camera unopenable — so the regression fixture for
  that bug is now the vendor's own XML rather than our reconstruction of it.
- The in-tree fake camera's XML uses conformant GenICam spelling (`=`, `&lt;&gt;`,
  no `<Output>`), and exercises a summed `<Address>` + `<pIndex>` stream-channel
  register and a `<pAddress>`-addressed `<StructReg>` end to end.
- **The fake camera answers the mandatory bootstrap block.** `Version`,
  `DeviceMode`, the MAC registers, the IP configuration and the channel counts
  all read as zero before, so a register dump taken from the fake looked
  nothing like one taken from a camera. The MAC in the registers is now
  asserted against the MAC in the Discovery ACK — two copies of one fact
  drifting apart is how #57 happened.
- **The issue templates ask for artifacts GitHub will actually accept.** The
  camera template asked for a `.xml` attachment, which GitHub rejects, and put
  `render: shell` on the field where it told reporters to attach a file, which
  disables uploads (DOC-12, DOC-13 — both found by codex review on #58). It now
  asks for the `viva-camctl report` bundle.
- **ADR-0018 and ADR-0019 no longer treat aravis as an authority.** ADR-0018
  said to verify against "the reference implementation" and named `../aravis`
  the tiebreaker for formula semantics; ADR-0019 called aravis and Wireshark
  "the practical authorities". Both are amended. `CLAUDE.md` carries the
  ranking they now defer to: real hardware, then the specification, then the
  vendor XML corpus, then independent implementations as corroboration that is
  cited but never decisive. A question they cannot settle goes to the backlog
  to wait for a device (TC-09, TC-12).
- Both READMEs install with `cargo add viva-genicam` rather than a pinned
  `viva-genicam = "X.Y"` line. Nothing built that line, so it rotted to `"0.1"`
  and stayed there through all of 0.2; and a pin is wrong at one end or the
  other whenever the tree is ahead of what crates.io serves.

### Breaking

- `gige::DeviceInfo` gains `version`, `serial` and `user_name`, all parsed from
  the Discovery ACK (offsets 136, 216 and 232) and previously discarded. Use
  `DeviceInfo::from_ip` to build a record for a camera addressed directly by IP.
  Viva Studio now shows the camera's own serial number instead of its MAC.
- Reported MAC addresses shift by two bytes relative to 0.2.x — the old value
  was wrong. Anything keyed on it (a saved device list, a config file) needs
  updating.
- `Addressing::Fixed` and `Addressing::Indirect` are replaced by
  `Addressing::Sum { terms, len }` with the new `AddressTerm` and `IndexOffset`
  types. `Addressing::fixed()`, `byte_len()` and `referenced_nodes()` are
  provided as helpers.
- `impl From<XmlModel> for NodeMap` is replaced by `TryFrom`; use
  `NodeMap::try_from_xml`.
- `bytes_to_i64` and `i64_to_bytes` take a `Sign`.
- `NodeDecl::Integer` and `IntegerNode` gain a `sign` field; the SwissKnife and
  Converter declarations gain `bindings`.
- `viva_genapi::swissknife` is now public: `evaluate` takes an `EvalMode` and
  works in `Value` rather than `f64`.

## [0.2.8] - 2026-07-28

Second hotfix in the same family as 0.2.7: a single unusual node in a camera's
GenApi XML could make the whole camera unopenable. **Hikrobot users blocked on
#35 should upgrade** — and after this release, this class of problem degrades to
a missing feature rather than a failed connect.

### Fixed

- **Constant-formula SwissKnife nodes no longer abort the XML load** — a
  `<IntSwissKnife>` / `<SwissKnife>` whose `<Formula>` needs no inputs is legal
  GenICam and appears in the standard's own conformance documents, but we
  required at least one `<pVariable>` and failed the whole document without it.
  This blocked a Hikrobot MV-CS050-10GC on `PixelDynamicRangeMin_Value`
  (reported in #35 after the original fix landed).
- **Unrecognized `<AccessMode>` values no longer abort the XML load** — `R` and
  `W` (seen in third-party GenICam documents, including the standard's
  conformance fixtures) now map to `RO` / `WO`, `WR` maps to `RW`, and anything
  else falls back to `RW` with a warning instead of rejecting the document. `RW`
  is what an absent `<AccessMode>` already meant, and the device still enforces
  its own access rules.

### Changed

- **A node we cannot parse now costs that one feature, not the whole camera.**
  Every node element is parsed in isolation: the document reader consumes the
  element up front, so a parse failure can neither desync it nor abort the load.
  Failures are recorded in the new `XmlModel::skipped` (tag, `Name`, error) and
  logged at `warn`, so the loss is visible rather than silent. This is the
  structural fix behind #45, #35 and the two bugs above — all of them were a
  single odd node making an entire camera unopenable.

## [0.2.7] - 2026-07-28

Hotfix for a 0.2.6 regression that made cameras with certain vendor XML
impossible to open. **Anyone on 0.2.6 should upgrade.**

### Fixed

- **Cannot connect to FLIR (and other vendors') cameras: `unescape error: Cannot find ';' after '&'`** (#45).
  Connecting failed outright — `connect_gige` / `connect_u3v` returned an error and no
  camera handle — whenever the device's GenApi XML put a literal `&` somewhere it is
  perfectly legal, most commonly a `<![CDATA[...]]>` tooltip or a comment inside
  `<ToolTip>` / `<Description>`. Reported against a FLIR BFS-PGE-31S4C-C, but nothing
  about it is FLIR-specific.

  A regression from the quick-xml 0.31 → 0.41 migration in 0.2.6: the migration added
  entity unescaping (0.31 never unescaped at all, so `&amp;` used to leak through
  literally) but applied it to `Reader::read_text`'s **raw** span, which still contains
  markup. quick-xml documents that method as explicitly not unescaping, precisely because
  the span can contain CDATA.

  Text extraction is now an explicit event walk: character data is decoded, CDATA is taken
  literally, character and predefined entity references are resolved, and comments and
  processing instructions are dropped. An entity with no definition (GenICam declares no
  DTD, so e.g. `&copy;` has nothing to resolve against) is kept as written instead of
  failing the document.
- **A lone `&` in vendor XML no longer blocks connecting.** `parse` and
  `parse_into_minimal_nodes` enable quick-xml's `allow_dangling_amp`, so technically
  non-conformant XML — which we cannot fix and the camera will not stop shipping — loads
  with the `&` carried through verbatim. A cosmetic tooltip is never worth failing a
  camera connect over.
- **Malformed GenApi XML is now reported as a parse error, not a transport error.**
  `connect_gige` / `connect_u3v` mapped XML parse failures to `GenicamError::Transport`
  (Python: `TransportError`), which sent debugging off toward the network. They now return
  `GenicamError::Parse` (Python: `ParseError`) with the stage named in the message.
  **Note for Python users:** if you catch `viva_genicam.TransportError` around a connect
  call to handle bad XML, switch to `ParseError` (or the shared `GenicamError` base).

### Added

- **Regression coverage for vendor-XML text quirks** — parser tests for CDATA, a dangling
  `&`, comments inside text elements, entity references and the whitespace around them,
  numeric character references, and undefined entities. The fake GigE camera's `Gain`
  tooltip is now a CDATA section containing `&` and `<`, as several vendors ship, so
  `test_connect_with_cdata_tooltip` exercises the fix through a full end-to-end connect.

## [0.2.6] - 2026-07-25

### Fixed

- **Every GVSP frame was reassembled at the wrong stride, and short frames were delivered as if complete** ([#34](https://github.com/VitalyVorobyev/viva-genicam/pull/34), thanks [@Katze719](https://github.com/Katze719)). The per-packet payload stride is `GevSCPSPacketSize` minus 36 bytes of overhead (IP 20 + UDP 8 + GVSP 8), not minus 8, so every packet after the first was written to the wrong offset; the expected-packet count was off by one for payloads that divide evenly; and `is_complete()` was never consulted, so a frame missing packets was handed to the caller as a valid image rather than dropped. *(Entry added retroactively on 2026-07-31: this shipped in 0.2.6 — `69c6c80` is an ancestor of `v0.2.6` — but was omitted from the changelog at the time.)*
- Cap decompressed GenICam XML at 64 MiB to prevent memory exhaustion from malicious or corrupt ZIP metadata served by a device.
- **GigE connect fails on Hikrobot cameras with `InvalidParameter`** (#35) -- two independent fixes:
  - `read_mem` now rounds every READMEM byte count up to a multiple of 4 as GVCP requires and drops the padding; strict cameras (Hikrobot) reject unaligned counts, which broke the final partial block of the XML download.
  - `fetch_and_load_xml` transparently decompresses ZIP-packed GenApi XML (`PK\x03\x04` magic), which Hikrobot/Basler/FLIR cameras commonly serve, and falls back to `GevSecondURL` (0x0400) when the first URL register is empty or unreadable.

### Added

- **Fake GigE camera realism** -- the fake GVCP server now rejects unaligned READMEM requests with `GEV_STATUS_INVALID_PARAMETER` exactly like strict real cameras (regression guard for #35), and `FakeCameraBuilder::zip_xml(true)` serves the GenApi XML as a ZIP archive. New integration test `test_connect_with_zipped_xml` covers the zipped + unaligned-length connect path end-to-end.

### Changed

- **quick-xml 0.31 → 0.41** -- clears RUSTSEC-2026-0194 (quadratic duplicate-attribute check) and RUSTSEC-2026-0195 (unbounded `NsReader` namespace allocation). API migration: `Reader::trim_text` → `config_mut().trim_text(true)`, `Attribute::unescape_value` → `normalized_value(XmlVersion::Implicit1_0)`, `read_text` now returns `BytesText` (decode + unescape explicitly).
- **Dependency refresh** -- full `cargo update` to the latest semver-compatible versions after three dormant months (139 lock entries), which also clears the advisories against anyhow (RUSTSEC-2026-0190 unsound `downcast_mut`), crossbeam-epoch (RUSTSEC-2026-0204), quinn-proto (RUSTSEC-2026-0185), and rustls-webpki (RUSTSEC-2026-0104), and replaces yanked spin/stabby releases. `cargo deny check` is fully green again.
- **License checking enforced** -- `deny.toml` gains `[licenses]` (permissive allow-list, MPL-2.0 exception for `option-ext`, documented libusb1-sys vendored-LGPL caveat), `[bans]`, and `[sources]` sections; CI and the weekly audit now run the full `cargo deny check`. The weekly audit workflow switches from cargo-audit to cargo-deny so `deny.toml` is the single source of truth for accepted advisories.
- **PyPI license metadata corrected for vendored libusb** -- the binary wheel statically links libusb 1.0.27 (LGPL-2.1-or-later) via `rusb`'s `vendored` feature, but the package metadata claimed plain MIT. The license expression is now `MIT AND LGPL-2.1-or-later`, the LGPLv2+ trove classifier is added, and the wheel/sdist ship `THIRD-PARTY-NOTICES.md` plus the full LGPL-2.1 text (`LICENSES/LGPL-2.1.txt`), including LGPL §6 relinking instructions (rebuild from source against a system libusb with the vendored feature disabled).

## [0.2.5] - 2026-04-15

### Changed

- **PyO3 / NumPy 0.22 → 0.28** -- port the Python bindings to current PyO3: drop `_bound`-suffixed constructors (`PyDict::new_bound`, `PyBytes::new_bound`, `PyArray1::from_slice_bound`, `PyModule::new_bound`, `py.import_bound`, `py.get_type_bound`, `from_vec_bound`, `empty_bound`), migrate `Python::allow_threads` to `Python::detach`, replace the removed `PyObject` type alias with `Py<PyAny>`, opt in to `FromPyObject` on `Clone + pyclass` types explicitly, handle `PyList::new`'s new `PyResult` return type. Wheel rebuilds cleanly with zero warnings.

### Fixed

- **Logo renders identically on every platform** -- the SVG at the top of the README loaded Nunito Black and Fira Code from Google Fonts via `@import`, which GitHub's SVG sanitizer strips. On Windows the fallback font shifted "viva" so it overlapped "genicam" and misplaced the red dot. Flatten all text to SVG paths (instanced the variable Nunito font at weight 900) so the logo is font-independent, and re-center the dot over the dotless `ı`.
- **EADDRINUSE race in the fake GigE camera** -- `FakeCamera::stop` used to fire `JoinHandle::abort()` and return immediately; the tokio task still held an `Arc<UdpSocket>` clone briefly, so rebinding the same port back-to-back (e.g. between module-scoped pytest fixtures) intermittently failed on macOS-14 / Python 3.9. `stop` is now `async` and awaits both join handles after aborting; the Python binding drives it via the shared runtime. Also set `SO_REUSEPORT` on the GVCP socket as defense-in-depth since `SO_REUSEADDR` is a UDP no-op on macOS.
- **CI build failure for `viva-fake-gige`** -- `socket2::set_reuse_port` sits behind socket2's `all` feature; local builds picked it up via transitive feature unification with `viva-gige`, but clean CI builds that don't pull in `viva-gige` failed with `no method set_reuse_port found`. Enable `socket2 = { features = ["all"] }` on `viva-fake-gige` directly.

### Added

- **`CLAUDE.md` pre-push checklist** -- document the three local gates CI runs with warnings-as-errors (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo doc --workspace --all-features --no-deps`) and the version-bump procedure (root `Cargo.toml`, `viva-pygenicam/Cargo.toml`, `viva-pygenicam/pyproject.toml`, `CHANGELOG.md`). Also require verifying the latest crate version on crates.io before any dependency bump.

## [0.2.4] - 2026-04-13

### Added

- **Python bindings (`viva-genicam` on PyPI)** -- new `crates/viva-pygenicam` PyO3 crate plus pure-Python facade in `python/viva_genicam/`. Ships as an abi3 wheel covering discovery, control, introspection, and streaming for both GigE Vision and USB3 Vision cameras. Frames expose a NumPy-friendly `to_numpy()` / `to_rgb8()` API, streams are sync iterators over a managed Tokio runtime (no asyncio required), errors map onto a `GenicamError` subclass hierarchy, and `py.typed` + `.pyi` stubs give IDEs full completion. Fake-camera pytest suite (19 tests) runs against the built wheel.
- **Python wheels CI (`.github/workflows/python.yml`)** -- cross-platform wheel matrix (Linux x86_64, macOS arm64, Windows x86_64) × Python 3.9–3.13; libusb is statically vendored into the extension so no system libusb is required by the wheel. Publishes to PyPI via OIDC on `py-v*` tags. Test install uses `pip --no-index --find-links dist` so CI never falls back to a published PyPI wheel while validating the just-built artifact.
- **Auto-detected GigE streaming interface** -- `camera.stream()` now picks the NIC whose IPv4 subnet contains the camera's IP when `iface` is omitted; loopback cameras resolve to `lo`/`lo0` automatically. An explicit `iface=` override remains available on both `connect_gige` and `stream`.
- **`book/src/python.md`** -- Python API tutorial chapter; README gains a Python section.
- **Python examples** -- `crates/viva-pygenicam/examples/` ships five runnable scripts (`discover.py`, `get_set_feature.py`, `node_browser.py`, `grab_frame.py`, `demo_fake_camera.py`) plus an `examples/README.md` that describes each.
- **Expanded book tutorial** -- `book/src/python.md` becomes an index; five sibling pages under `book/src/python/` walk through install, discovery, control & introspection, streaming, and a full API reference.
- **In-process fake camera (`viva_genicam.testing.FakeGigeCamera`)** -- the `viva-fake-gige` crate is now bound as a Python class shipped inside the wheel. `pip install viva-genicam` alone is enough to run the full demo end-to-end; no subprocess, no binary to build, no repo clone required. The `demo_fake_camera.py` example and `tests/conftest.py` were migrated onto the in-process path.

### Changed

- Root `Cargo.toml` gains `workspace.exclude = ["crates/viva-pygenicam"]` so PyO3/maturin stays out of the default `cargo test --workspace` path.

## [0.2.3] - 2026-04-12

### Added

- **Zenoh API v2 `FeatureState` contract** -- new `FeatureState`, `NumericRange`, and `CommandResult` wire types expose live feature introspection (is_implemented / is_available / access_mode / numeric range / enum_available) per node; `introspect` queryable wired into `viva-service` and `viva-service-u3v`. Legacy `NodeValueUpdate` stays wire-compatible. See ADR-010.
- **GenApi predicate evaluation** -- `NodeMap::is_implemented`, `is_available`, `effective_access_mode`, and `available_enum_entries` evaluate `pIsImplemented` / `pIsAvailable` / `pIsLocked` / per-enum-entry predicates via the existing `resolve_numeric` machinery (Integer / Boolean / SwissKnife / IntConverter / Converter / Enum providers all supported, with cycle detection)
- **Predicate refs on every `NodeDecl` variant** -- `PredicateRefs` (`p_is_implemented` / `p_is_available` / `p_is_locked`) parsed from `<pIsImplemented>` / `<pIsAvailable>` / `<pIsLocked>` and plumbed through `NodeMap::try_from_xml` with proper dependency registration; `Node::predicates()` exposes the refs for external evaluators
- **Realistic predicate wiring in the fake GigE camera** -- `ExposureTime.pIsLocked` ← `ExposureAuto != Off`, `Gain.pIsLocked` ← `GainAuto != Off`, `AcquisitionFrameRate.pIsAvailable` ← new `AcquisitionFrameRateEnable` Boolean, `PixelFormat` entries gated by a new `SensorType` enum (Monochrome / BayerRG / Color)

### Changed

- **`DeviceHandle::get_feature_state`** now reports live `is_implemented` / `is_available` / `access_mode` / `enum_available` from the predicate evaluators instead of hardcoded permissive defaults; each predicate call is guarded so a single bad formula doesn't break the whole feature snapshot

### Fixed

- **Float bit-pattern bug** -- `<FloatReg>` and bare `<Float>` with `Length in {4, 8}` and no `<Scale>`/`<Offset>` now auto-infer IEEE 754 encoding. Before this fix, `AcquisitionFrameRate` came back as `1106247680` (the f32 bit pattern of 30.0) and `ExposureTime` as `4662219572839973000` because float registers were always read as scaled i64. New `FloatEncoding` (Ieee754 / ScaledInteger) + `byte_order` on `NodeDecl::Float`; `get_float` / `set_float` dispatch on encoding.

## [0.2.2] - 2026-04-12

### Changed

- Rename old repo name `viva-genicam` to the new `viva-genicam`

## [0.2.1] - 2026-04-12

### Added

- **Multi-platform release binaries** -- release workflow now produces prebuilt `viva-camctl` and `viva-service` archives for Linux x86_64, macOS aarch64 (Apple Silicon), and Windows x86_64; each archive bundles the binaries with `README.md`, `LICENSE`, and `CHANGELOG.md`, and a `SHA256SUMS.txt` is published alongside
- **`viva-camctl` on crates.io** -- the CLI is now published, so `cargo install viva-camctl` works

### Changed

- Internal workspace dependency version requirements simplified from `"0.2.0"` to `"0.2"` (semver-equivalent, but avoids a sweep on every patch bump)
- Release workflow dropped the redundant `.crate` packaging step -- those archives are hosted on crates.io via the publish-crates workflow

### Fixed

- `GigeRegisterIo` now detects async context via `Handle::try_current()` and wraps `block_on` in `tokio::task::block_in_place` only when inside a runtime, preventing nested-runtime panics while preserving plain synchronous usage

## [0.2.0] - 2026-04-11

### Added

- **USB3 Vision streaming** -- `U3vFrameStream` async frame iterator wrapping blocking bulk reads via `spawn_blocking`, `U3vStreamBuilder` for configuring U3V streams through the same pattern as GigE
- **USB3 Vision service** -- `viva-service-u3v` now supports real USB cameras (previously `--fake` only); `U3vDeviceHandle` is generic over `T: UsbTransfer`
- **USB3 Vision CLI** -- `viva-camctl stream-usb` command for frame streaming from USB3 Vision cameras
- **FORCEIP command** -- GVCP opcode 0x0004 for temporary IP assignment via broadcast (targets device by MAC address)
- **Persistent IP configuration** -- read/write bootstrap registers for persistent IP, subnet, and gateway; `enable_persistent_ip()` method on `GigeDevice`
- **IP configuration CLI** -- `viva-camctl set-ip` command with `--force` (FORCEIP) and persistent register modes
- **Reconnection with backoff** -- `DeviceHandle::refresh_connection()` retries up to 5 times with exponential backoff (500ms base, 16s max)
- **GenApi node metadata** -- `NodeMeta` struct with `Visibility`, `Description`, `ToolTip`, `DisplayName`, `Representation` fields; parsed from XML and exposed on all node types
- **Visibility filtering** -- `Visibility` enum (Beginner/Expert/Guru/Invisible), `Representation` enum (Linear/Logarithmic/HexNumber/etc.), `NodeMap::nodes_at_visibility()` for UI filtering
- **`U3vDevice::transport()`** -- public accessor for the shared USB transport `Arc<T>`
- **Bayer 16-bit pixel formats** -- `PixelFormat` enum now includes BayerGR16, BayerRG16, BayerGB16, BayerBG16 with correct PFNC codes; `PixelFormat::from_name()` for string-to-enum conversion
- **`PixelFormat::from_name()`** -- parse PFNC name strings (e.g. "RGB8", "Mono16", "BayerRG16") to `PixelFormat`

### Changed

- MSRV raised from 1.85 to 1.88 (resolves `time` crate security advisory RUSTSEC-2026-0009)
- Project tagline updated from "Ethernet-first" to "GigE Vision and USB3 Vision" reflecting dual-transport support
- `viva-service-u3v` Cargo.toml now enables `u3v-usb` feature for real USB support
- Added `cargo deny check advisories` to CI pipeline with `deny.toml` allow-list for zenoh transitive advisories

### Fixed

- GitHub Pages deployment error ("Tag v0.1.0 not allowed to deploy") by removing wildcard tag trigger from `publish-docs.yml`
- SVG logo dot alignment and genicam text spacing for correct browser rendering
- `time` crate DoS vulnerability (RUSTSEC-2026-0009) by upgrading to 0.3.47

### Known Issues

- `lz4_flex 0.10.0` (RUSTSEC-2026-0041, high) and `rsa 0.9.10` (RUSTSEC-2023-0071, medium) are transitive dependencies through `zenoh 1.9.0` and cannot be updated until zenoh releases a fix. Neither is exploitable through our usage (lz4 decompression of untrusted data, RSA timing attack). Tracked in `deny.toml`.

## [0.1.0] - 2026-04-10

Initial public release of the viva-genicam workspace.

### Added

- **viva-genicam** -- High-level facade crate with `Camera<T>`, discovery, streaming, events, and action commands
- **viva-gige** -- GigE Vision transport layer: GVCP discovery, GenCP register I/O, GVSP streaming with resend and reassembly
- **viva-genapi** -- In-memory GenApi node map with typed feature access (Integer, Float, Enum, Boolean, Command, SwissKnife, Converter, String)
- **viva-genapi-xml** -- GenICam XML parsing into an intermediate representation with async XML fetch
- **viva-gencp** -- Transport-agnostic GenCP protocol encode/decode
- **viva-u3v** -- USB3 Vision transport: bootstrap registers, GenCP-over-USB control, and bulk streaming
- **viva-pfnc** -- Pixel Format Naming Convention (PFNC) tables and helpers
- **viva-sfnc** -- Standard Feature Naming Convention (SFNC) string constants
- **viva-zenoh-api** -- Shared Zenoh API payload types (no Zenoh dependency)
- **viva-service** -- Zenoh bridge exposing GenICam cameras as network services

### Protocol Features

- GVCP discovery (broadcast and unicast)
- GenCP register read/write with retry and backoff
- GVSP streaming with frame reassembly
- Packet resend with bitmap tracking and exponential backoff
- Automatic packet size negotiation from MTU
- Multicast stream support (IGMP join/leave)
- GVCP event channel with timestamp mapping
- Action commands with scheduled execution
- Chunk data parsing (timestamp, exposure time, gain, line status)
- Extended ID support (64-bit block IDs, 32-bit packet IDs per GigE Vision 2.0+)

### Testing

- `viva-fake-gige` -- In-process fake GigE Vision camera for self-contained integration testing (no external dependencies required)
- `viva-fake-u3v` -- In-process fake USB3 Vision camera for testing

[Unreleased]: https://github.com/VitalyVorobyev/viva-genicam/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.3.1
[0.3.0]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.3.0
[0.2.8]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.8
[0.2.7]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.7
[0.2.6]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.6
[0.2.5]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.5
[0.2.4]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.4
[0.2.3]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.3
[0.2.2]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.2
[0.2.1]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.1
[0.2.0]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.2.0
[0.1.0]: https://github.com/VitalyVorobyev/viva-genicam/releases/tag/v0.1.0
