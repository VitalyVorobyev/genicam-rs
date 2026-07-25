# ADR-0005: Full SFNC pixel format coverage

**Status:** Accepted
**Date:** 2026-03-06

## Context

The current streamer only handles `Mono8`. Industrial cameras produce a wide range of pixel formats defined in the SFNC (Standard Features Naming Convention).

## Decision

The streamer and Image Viewer will support the **full SFNC pixel format set**, implemented incrementally:

### Phase 1 (MVP)
- Mono8

### Phase 2
- Mono10, Mono12, Mono16 (bit-shift to 8-bit or 16-bit display)
- BayerRG8, BayerGR8, BayerBG8, BayerGB8 (debayer to RGB8)

### Phase 3
- RGB8, BGR8, RGBa8
- BayerXX10, BayerXX12, BayerXX16

### Phase 4
- YCbCr422_8, YCbCr8
- Coord3D_C16 (depth maps)
- Packed formats (Mono10p, Mono12p)

## Implementation

- The camera service declares the active `PixelFormat` in the Zenoh image metadata.
- The streamer negotiates format at session start and selects the appropriate encoder.
- The Image Viewer adapts its rendering pipeline based on the pixel format.
- Debayering and color conversion happen in Rust (streamer or service), not in the browser/UI.

## Consequences

- The image Zenoh key payload format must include metadata (or a separate metadata key) for pixel format, width, height.
- The BMP encoder in the streamer must be extended or replaced with a format-aware encoder.
- The WebSocket protocol between streamer and viewer needs a header/info frame describing the format.
