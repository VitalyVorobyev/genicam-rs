# ADR-0018: GenApi Conformance over Convenient Approximations

**Status:** Accepted
**Date:** 2026-07-29

## Context

Issue #35 began as one Hikrobot MV-CS050-10GC that would not open. The
reporter attached a `dbg!` dump of the parsed model — 3 232 nodes, 314
formulas — which, checked against the 30-document vendor corpus in
`fixtures/vendor-xml/`, turned a single-camera report into a measurable
audit. The result was not a Hikrobot quirk:

| What we had | What GenICam specifies | Corpus reach |
|---|---|---|
| `==` / `!=` for equality | `=` and `<>` | 27 of 30 documents rejected outright |
| Every formula in `f64` | `<IntSwissKnife>`/`<IntConverter>` evaluate in `i64` | 3 125 of 3 489 knives |
| `<Output>` decides the knife type | the element name does; `<Output>` is not a GenApi element | 0 occurrences of `<Output>` in the corpus |
| One address per register | address = **sum** of `<Address>`, `<pAddress>`, `<pIndex>` | FLIR 454, PGR 418, Hikrobot 42 registers |
| `<pIndex>` ignored | index × stride, added | 197 occurrences |
| `<StructReg>` keeps `<Address>` only | shares all its address terms | 860 Hikrobot nodes at address 0 |
| Always sign-extend | unsigned unless `<Sign>Signed</Sign>` | every register with the top bit set |
| Reads use `<FormulaTo>` | reads use `<FormulaFrom>` | every non-identity converter |

Three of these stopped a camera from opening. The rest were worse: the
camera opened and quietly read the wrong registers, or evaluated
predicates against sign-extended garbage, with no error anywhere.

The common thread is that each was a *reasonable-looking approximation*.
A C-like expression grammar is a sensible default if you have not read the
GenApi formula grammar. One address per register is a sensible data model
if every document you have seen declares one. Sign-extending is a sensible
default if your integers are `i64`. Each was defensible in isolation, and
each was wrong.

They survived because our test suite was built from fixtures we wrote
ourselves. A fake camera can only exercise constructs we already thought
of, so it validated our approximations rather than challenging them. The
vendor corpus test that did challenge them stopped at `viva_genapi_xml::parse`
— one layer below where all eight of these defects lived.

## Decision

**Where the GenICam specification is explicit, we implement the
specification, and we verify against the reference implementation rather
than against our own reading of it.**

Concretely:

1. The formula language follows the GenApi grammar — `=`, `<>`, the
   standard function set including `LG`, the `E`/`PI` constants,
   `<Constant>` and named `<Expression>` bindings — with the C-like
   spellings kept only as tolerated aliases. Operator precedence and the
   integer/float promotion rules are cross-checked against aravis
   (`src/arvevaluator.c`), which is cited in the source.
2. A node's type comes from its element name, not from a non-standard
   hint element.
3. `Addressing` models the address as a **sum of terms**
   (`Addressing::Sum { terms, len }`), because that is what the standard
   describes. There is no "primary" term.
4. Where the standard defines a default (`<Sign>` is unsigned, `<AccessMode>`
   absent is RW), we use that default rather than one inferred from
   surrounding metadata.
5. **Conformance is measured, not asserted.** `crates/viva-genapi/tests/vendor_corpus.rs`
   builds a `NodeMap` from every corpus document and evaluates every node
   in it against a stub transport. A document that parses but cannot be
   used is a failure.

Where the standard is genuinely silent — the sign of a scaled `<Float>`
register, of an `<Enumeration>` payload — we keep existing behaviour and
say so in a comment, rather than inventing a rule.

## Consequences

### Positive

- All 30 corpus documents now build a nodemap and evaluate: 21 785 nodes,
  up from 3 documents that got that far.
- The conformance gate sits at the layer where these bugs live. Any future
  approximation in the formula engine, the address model or the numeric
  codecs fails the weekly corpus run instead of reaching a user.
- Failures are now visible: an unrepresentable node is recorded in
  `NodeMap::skipped` and logged, rather than either being dropped silently
  or taking the whole camera down.
- `viva-genicam` no longer panics on remote input (backlog SR-01).

### Negative

- Breaking changes for downstream consumers. `Addressing::Fixed` and
  `Addressing::Indirect` are gone, replaced by `Addressing::Sum`;
  `NodeMap::from` is replaced by `TryFrom`/`try_from_xml`;
  `bytes_to_i64`/`i64_to_bytes` take a `Sign`. Viva Studio needs the
  matching update.
- Behaviour changes that are correct but observable. An `<IntSwissKnife>`
  that used to round `5 / 3` to 2 now truncates to 1; a register whose top
  bit is set now reads positive; a converter now reads through the other
  formula. Anything calibrated against the old behaviour will move.
- Keeping up with the reference implementation is now an ongoing
  obligation: `../aravis` is cited in the source as the tiebreaker for
  formula semantics, so divergence there is a bug in ours.
- The corpus test costs a full nodemap build and evaluation per document.
  It stays out of PR CI — the corpus is fetched from third-party
  repositories — so a regression is caught weekly rather than on merge.
