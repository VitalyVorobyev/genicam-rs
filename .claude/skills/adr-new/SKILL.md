---
name: adr-new
description: Scaffold the next ADR in docs/adrs/ with the house template and index it
---

# New ADR

Argument: a short kebab-case title (e.g. `gvsp-resend-strategy`).

## Steps

1. Find the highest `adrNNNN` in `docs/adrs/` and take the next number
   (zero-padded to four digits).
2. Create `docs/adrs/adrNNNN-<kebab-title>.md` using the house template
   (documented in `docs/adrs/README.md`):

   ```markdown
   # ADR-NNNN: Title In Words

   **Status:** Proposed | Accepted
   **Date:** <today, YYYY-MM-DD>

   ## Context

   ## Decision

   ## Consequences

   ### Positive

   ### Negative
   ```

3. Write the content. Focus the **Context** on WHY — the forces at play
   and the alternatives that were considered and rejected. The Decision
   section states what was chosen; Consequences lists honest trade-offs
   (both Positive and Negative).
4. Add the new row to the index table in `docs/adrs/README.md`
   (`| [NNNN](adrNNNN-<kebab-title>.md) | Title | Status |`).

## Reminders

- Retrospective ADRs are welcome — recording a decision already made is
  better than leaving it undocumented.
- An ADR that reverses an earlier one must mark the old ADR
  **Superseded** (in its Status line and in the index) with a link to
  the new one.
