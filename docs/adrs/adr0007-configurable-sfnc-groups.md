# ADR-0007: Configurable SFNC feature groups in Image Viewer

**Status:** Accepted
**Date:** 2026-03-06

## Context

Different cameras expose different subsets of SFNC features. The Image Viewer should show purpose-built controls for common features, but the exact mapping between UI sections and GenICam node names must be flexible.

## Decision

SFNC feature groups are defined in a **configuration file** (`sfnc-groups.json`) that ships with the app. Users can customize it. The configuration maps group names to arrays of GenICam node names with UI hints.

```json
{
  "groups": [
    {
      "id": "exposure_gain",
      "label": "Exposure & Gain",
      "icon": "sun",
      "default_expanded": true,
      "features": [
        { "node": "ExposureTime", "widget": "slider", "unit_override": null },
        { "node": "ExposureAuto", "widget": "enum_toggle" },
        { "node": "Gain", "widget": "slider" },
        { "node": "GainAuto", "widget": "enum_toggle" }
      ]
    }
  ]
}
```

### Runtime behavior
1. On connect, the app loads the SFNC groups config.
2. For each group, it checks which listed nodes exist in the camera's UiGraph.
3. Groups with zero matching nodes are hidden.
4. Matching nodes are rendered with the specified widget type.
5. Widget types: `slider` (numeric with range), `enum_toggle` (dropdown or button group), `checkbox` (boolean), `text_input` (string), `readonly` (display only).

## Consequences

- Image Viewer adapts to any camera without code changes.
- Power users can add custom groups for vendor-specific features.
- Default config covers the most common SFNC features.
- Widget rendering reuses the existing editor components from Feature Browser where possible.
