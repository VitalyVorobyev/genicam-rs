# ADR-0006: Progressive disclosure for Image Viewer controls

**Status:** Accepted
**Date:** 2026-03-06

## Context

The Image Viewer needs camera controls (exposure, gain, ROI, trigger, etc.). Different users need different levels of detail. A rigid tier system (Simple/Standard/Advanced) forces users into predefined modes.

## Decision

Use **progressive disclosure** in the Image Viewer: a single view with collapsible sections. Each section corresponds to an SFNC feature group. Sections start collapsed except for the essential ones (Acquisition, Exposure/Gain). Users expand what they need.

### Default sections (expanded)
- Acquisition Control (Start/Stop, mode)
- Exposure & Gain

### Available sections (collapsed)
- Image Format (Width, Height, OffsetX/Y, PixelFormat, Binning)
- Trigger Configuration
- Transport Layer (GigE packet size, stream channel)
- Auto Functions (auto-exposure, auto-gain, auto-white-balance)
- Color Processing (white balance, gamma, LUT)
- Digital I/O
- Counter & Timer
- Custom (user-defined groups from Feature Browser)

### Section state persistence
Section expand/collapse state is saved in localStorage per camera model.

## Consequences

- No mode switching UI needed.
- Users see exactly what they need; power users expand all sections.
- Section mapping to SFNC nodes is runtime-configured (see ADR-007).
- Sections that have no matching nodes on the connected camera are hidden.
