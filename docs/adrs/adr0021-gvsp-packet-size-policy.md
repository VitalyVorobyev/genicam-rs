# ADR-0021: GVSP Packet-Size Policy (Preserve, `--auto`, Explicit)

**Status:** Proposed
**Date:** 2026-08-04

## Context

`GevSCPSPacketSize` is both a camera register and a path property. The host NIC
MTU, every switch hop, and the camera’s Max must all clear the size that
actually goes on the wire. Endpoints accepting a size does **not** mean the
path will deliver it.

**Hardware.** A Vieworks FS-3200T on a host with jumbo enabled (NIC jumbo 16128,
IPv4 MTU 16114) through an ipTIME PoE4002 (~9216-byte frame ceiling) configures
16114 at both ends and streams `frames=0`. The same camera **direct** to that
NIC streams with max working **16114** (min failing 16115). Hand bisection and
SR-13’s GVSP test-packet probe both point at the path, not a camera clamp
([#112](https://github.com/VitalyVorobyev/viva-genicam/issues/112); local
confirm on PoE4002 vs direct).

**What the library does today (0.4.x after SR-10 / SR-13).**

1. If the caller does not pass an explicit size, `StreamBuilder` writes
   `best_packet_size(nic_mtu)` into `GevSCPSPacketSize`.
2. It reads the register back (SR-02) and follows a clamp.
3. By default it **path-probes** with GVSP test packets and bisects downward
   when the requested size does not arrive (SR-13; `StreamBuilder::probe`,
   default on).

That is a good engine for “make this link work at the largest safe size.” It is
a bad default for “leave the camera alone”: a user or vendor tool that already
set 9000 for a narrow switch loses that value on the next Start when the NIC
advertises 16114.

**SR-10 history.** Pre-0.4, `viva-camctl --auto` defaulted to **false** and the
`false` branch discarded the probed MTU in favour of 1500. SR-10 deleted the
flag and made “follow NIC MTU” the only path. That fixed the 1500 trap and
overcorrected: there is no longer a way to say “do not write the camera.”

Older READMEs still show `--auto` as if it existed; the CLI does not offer it.

## Decision

Adopt a three-way packet-size policy. Implementation is tracked in the backlog;
this ADR fixes the intended contract.

| Mode | Meaning | Write `GevSCPSPacketSize`? | Path probe (SR-13)? |
|------|---------|----------------------------|---------------------|
| **Default** (no flag) | Use the camera’s current value | **No** (read only; size recv buffers from it) | Optional / off by default for preserve, or probe-without-raising only if we can prove it never writes larger — prefer **off** so preserve is literal |
| **`--auto`** (explicit) | Start from `best_packet_size(nic_mtu)`, then negotiate the path | **Yes** | **Yes** — try the NIC MTU size first; if the test packet (or equivalent) fails, **bisect** downward with the existing SR-13 probe |
| **`--packet-size N`** | Caller-chosen ceiling | **Yes** (to N, after clamps) | Yes by default (never raises above N); mutually exclusive with `--auto` |

### `--auto` procedure

1. Resolve host interface MTU → `requested = best_packet_size(mtu)`.
2. Write / read-back `GevSCPSPacketSize` (SR-02).
3. Run the existing GVSP test-packet probe (SR-13): control probe at the floor;
   if the device answers and `requested` fails, bisect to the largest size the
   path delivers; if the device never answers probes, keep `requested` (do not
   punish non-implementers by walking to 1500).
4. Stream with the negotiated size.

So `--auto` is not “magic unknown algorithm”; it is **NIC MTU first, then the
already-shipped bisection** when the path is narrower than the NIC. That matches
the PoE4002 class of failure without a separate `--probe-packet-size` flag.

### What we are not deciding

- PMTUD / ICMP as the primary signal (unreliable; not GVSP).
- Quiet automatic step-down on every silent stream in service/studio without an
  explicit auto mode (diagnosis becomes impossible).
- A mandatory floor of 1500 for search: library minimum is 576; 1500 remains a
  useful *sanity* check (if even ~1500 fails, blame firewall / CCP / trigger,
  not path jumbo), not a protocol requirement.

## Consequences

### Positive

- Default stops overwriting a camera value the operator already tuned for a
  switch.
- `--auto` reappears with an honest meaning (MTU + path bisect), fixing the
  stale docs without reviving “`--auto false` ⇒ 1500”.
- Reuses SR-13 instead of inventing a second probe CLI.
- `--packet-size` remains the immediate workaround when the path limit is known.

### Negative

- Breaking relative to today’s “always write NIC MTU” default: callers that
  relied on the overwrite must pass `--auto` (or set `StreamBuilder` to auto).
- Preserve-by-default means a camera left at a tiny factory size stays tiny
  until someone opts into `--auto` or sets a size — document that clearly.
- Devices that mis-handle test packets still need `probe(false)` escapes (already
  true for SR-13).

### Follow-up

- Implement the three-way API in `StreamBuilder`, `viva-camctl`, and Python
  (backlog SR-14).
- Until then, user-facing examples must describe **current** behaviour and must
  not show a working `--auto` flag.
