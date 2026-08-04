# ADR-0021: GVSP Packet-Size Policy (Preserve, `--auto`, Explicit)

**Status:** Accepted
**Date:** 2026-08-04

## Context

`GevSCPSPacketSize` is both a camera register and a path property. The host NIC
MTU, every switch hop, and the camera’s Max must all clear the size that
actually goes on the wire. Endpoints accepting a size does **not** mean the
path will deliver it.

**Hardware.** A Vieworks FS-3200T on a host with jumbo enabled (NIC jumbo 16128,
IPv4 MTU 16114) through an ipTIME PoE4002 (~9216-byte frame ceiling) configures
16114 at both ends and streams `frames=0`. The same camera **direct** to that
NIC streams with max working **16114** (min failing 16115). Both bisections are
the reporter's own, run with `viva-camctl stream`; removing the switch and
re-running is what separates a path limit from a camera clamp
([#112](https://github.com/VitalyVorobyev/viva-genicam/issues/112)). Whether the
SR-13 probe *lands* on that ceiling on this camera is not established — see
**Evidence standing** below.

**What the library does today (0.4.1, plus SR-13 on `main`).**

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

Older READMEs still showed `--auto` as if it existed, for a release in which the
CLI did not offer it. This ADR is also what makes those examples true again.

## Decision

Adopt a three-way packet-size policy, implemented in
[#118](https://github.com/VitalyVorobyev/viva-genicam/pull/118) (backlog SR-14).

| Mode | Meaning | Write `GevSCPSPacketSize`? | Path probe (SR-13)? |
|------|---------|----------------------------|---------------------|
| **Default** (no flag) | Use the camera’s current value | **Not upward.** Read only, unless the probe finds the path cannot carry what the device holds | **Yes** |
| **`--auto`** (explicit) | Start from `best_packet_size(nic_mtu)`, then negotiate the path | **Yes** | **Yes** — try the NIC MTU size first; if the test packet fails, **bisect** downward |
| **`--packet-size N`** | Caller-chosen ceiling | **Yes** (to N, after clamps) | **Yes** (never raises above N); mutually exclusive with `--auto` |

### Why preserve still probes

The obvious reading — preserve means *touch nothing* — was the first draft of
this ADR, and it is wrong on the hardware that prompted it. The reporter's raw
bootstrap read gives `0x0D04 = 0x00003EF2` — **16114 already in the register**.
Whatever put it there, a literal preserve hands that value straight back and
reproduces `frames=0` through the PoE4002, with the one mechanism that could
rescue it switched off.

Keeping the probe costs nothing that preserve is trying to protect, because **the
probe only ever lowers**. It cannot override an operator's choice upward; it can
only decline to stream at a size the path demonstrably drops — a fact no register
read can reach, since both endpoints accept it. So preserve's real promise is
*we will not raise what you set*, and that survives intact. `probe(false)` is
the escape for callers who want preserve to be literal.

The probe writes the register when it bisects. That is not an exception to the
policy but the mechanism of it: on this transport there is no way to ask without
telling, since the test-packet request and the size share one register.

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
- Preserve is not literal: the probe may lower the register. Callers who need
  the register untouched must say so with `probe(false)`.

### Evidence standing

The failing configuration is **first-hand**: the reporter measured the ceiling by
bisecting `viva-camctl stream` by hand, then isolated the switch by removing it
and re-running the same bisection (16114 direct, ≥9199 failing through the
PoE4002). Both numbers are theirs, not our inference from a log.

What is **not** confirmed on hardware is that the SR-13 probe helps *that*
camera, because it is unknown whether the FS-3200T answers GVSP test packets at
all. If it does not, the probe correctly changes nothing and `--packet-size`
remains the workaround on that link. This ADR does not depend on the answer —
preserve-by-default stands on the overwrite-on-every-Start complaint alone — but
the Default row's probe column does, and should be revisited if the answer is
no.
