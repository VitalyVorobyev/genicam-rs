# ADR-0019: Transport Conformance and Spec-Derived Fakes

**Status:** Accepted
**Date:** 2026-07-29

## Context

[ADR-0018](adr0018-genapi-conformance-over-convenience.md) audited the GenApi
layer against the specification and found eight defects, each a
reasonable-looking approximation of a rule the standard states explicitly. It
did not touch the wire.

Issue #57 — a JAI FS-3200T-10GE-NNC undiscoverable on a Windows APIPA network —
prompted the same audit of GVCP/GVSP. It found the same class of error:

| What we have | What the standard specifies | Reach |
|---|---|---|
| MAC read from Discovery ACK offset 12 | offset 10 | every GigE camera; reported MAC is shifted two bytes |
| `PENDING_ACK` (0x0089) unknown | a valid ack that extends the deadline | any camera that answers a slow WRITEMEM/READMEM |
| `ACTION_COMMAND = 0x0080` | 0x0080 is `READREG` | action commands are wire-indistinguishable from register reads |
| Event channel keys on 0x000D | GVCP events are 0x00C0–0x00C3 | no real camera event is ever accepted |
| Chunks decoded front-to-back, prefix-length | trailing tuples scanned backwards | every camera that sends chunks |
| Leader accepts payload type 0x01 only | 0x4001 is how cameras deliver chunks | chunk mode cannot work at all |

`PENDING_ACK` is the sharpest of these. `viva-gencp`'s ack decoder knows only
the four GenCP acks, so a camera that answers a flash write or a mode change
with a pending-ack produces `UnknownOpcode` → a hard failure with no retry and
no deadline extension. That is normal camera behaviour, not an edge case.

**The structural problem is more important than any individual defect.**
Issue #57's MAC offset is the *third* time our fake camera and our client have
shared one identical wrong assumption:

1. The fake's GVSP sender and our receiver both ignored the 36-byte IP+UDP+GVSP
   overhead when interpreting `GevSCPSPacketSize`.
2. The fake accepted unaligned READMEM that real Hikrobot hardware rejects.
3. The fake emits the MAC at Discovery ACK offset 12, exactly where the parser
   reads it (`crates/viva-fake-gige/src/gvcp_server.rs:134-141` against
   `crates/viva-gige/src/gvcp.rs:495-497`).

In each case producer and consumer agreed with each other and jointly disagreed
with the standard, so every test passed. ADR-0013's realism policy already says
a fake must implement the *standard's* semantics rather than mirror the
implementation's assumptions — but nothing enforces it, and a policy that
depends on the author already knowing the right answer cannot catch the case
where they do not.

The tests could not catch it either. The only discovery assertion in the suite
is a disjunction that checks no values at all:

```rust
assert!(fake.model.is_some() || fake.manufacturer.is_some(),
        "expected device identity fields");
```

No test anywhere asserts the MAC bytes, the serial number, the user-defined
name, the device version or the spec version.

## Decision

**ADR-0018's rule extends to the wire: where the GigE Vision, GVCP or GVSP
specification is explicit, we implement the specification and verify against
independent references rather than against our own fake.**

Concretely:

1. **Opcodes and payload offsets are cited in the source**, with the reference
   that establishes them *and the rank that reference carries*. The GigE
   Vision specification is normative but sits behind an A3 registration form;
   where we can state what it requires, we cite it. Independent
   implementations — aravis (`../aravis/src/arvgvcpprivate.h` for the GVCP
   command enum) and Wireshark's `packet-gvcp.c` dissector for the fields
   aravis does not implement — are cited as corroboration, never as the rule.
   Where they disagree with each other or with our reading — 0x0004/0x0005 is
   `BYE` in aravis and `FORCEIP` in Wireshark — the discrepancy is recorded as
   an open question for hardware to settle, not resolved by preference. See
   the evidence hierarchy in `CLAUDE.md`.

2. **Fake wire fixtures are spec-derived byte arrays, asserted independently of
   the client parser.** A test that round-trips the fake through our own parser
   proves only that the two agree. Each wire format the fake emits gets a
   golden fixture written out from the specification's field table, and the
   fake is asserted against those bytes directly — not against what
   `parse_discovery_payload` makes of them. The parser is then asserted against
   the same fixtures from the other side.

3. **Identity assertions check values, not presence.** Discovery tests assert
   the exact MAC, IP, manufacturer, model, serial and user-defined name the
   fake was configured with. A disjunction over `is_some()` is not a test.

4. **A protocol feature with no fake implementation is not considered
   implemented.** ACTION and EVENT currently have client code and no fake
   counterpart, which is why their opcode errors survived. Wiring the fake is
   part of the fix, not a follow-up.

5. **Unknown acks are handled, not fatal.** `PENDING_ACK` extends the
   transaction deadline; an unrecognised opcode on a shared socket is discarded
   rather than failing the operation that happened to be waiting.

Where the standard is genuinely ambiguous or the reference implementations
disagree, we say so in a comment and open a backlog item to resolve it against
hardware — the same discipline ADR-0018 applied to the sign of scaled `<Float>`
payloads.

## Consequences

### Positive

- The conformance gate moves to the layer where these bugs live. A future
  offset or opcode error fails a golden-fixture test on merge rather than
  reaching a user with hardware we do not have.
- The fake stops being a second implementation of our own assumptions and
  becomes an independent check, which is what ADR-0013 always claimed it was.
- Chunk mode, action commands and events become testable for the first time.
- Discovery reports fields it currently discards (serial, user-defined name),
  so users and Viva Studio can identify a camera the way its label does.

### Negative

- Behaviour changes that are correct but observable. Reported MACs shift by two
  bytes; anything keyed on the old value — a saved device list, a config file —
  will not match. This lands in 0.3.0 alongside the GenApi break rather than as
  a silent patch.
- The golden fixtures are a maintenance cost, and a wrong fixture is now a
  wrong test. They carry the reference citation inline so a reviewer can check
  them against the source rather than against intuition.
- We cannot always quote the normative document. A3 delivers the GigE Vision
  specification only after a registration form, so some questions have no
  citable answer short of hardware. aravis and Wireshark are independent
  implementations carrying their own bugs and approximations, so they
  corroborate such a question but do not close it.
- Some of this needs hardware we do not have. FORCEIP has only ever been
  answered by our own fake; confirming it, and the chunk trailer layout, needs
  a user with a real camera and a packet capture. TC-09 and TC-12 are the two
  open cases, and they stay open rather than being decided by whichever
  reference we find more persuasive.

## Amendment — 2026-07-29

Decision item 1 originally called aravis and Wireshark "the citable
references", and the Negative section called them "the practical authorities
and their disagreements are ours to resolve". Both are corrected above. They
corroborate; a question they cannot settle goes to the backlog to wait for a
device, which is what TC-09 has always done and what TC-12 (the `PENDING_ACK`
field width) was filed under.

The project-wide evidence hierarchy this defers to lives in `CLAUDE.md`. The
rest of this ADR stands — and item 2 is what makes the demotion practical: a
golden fixture written out from the specification's own field table needs no
second implementation to vouch for it.
