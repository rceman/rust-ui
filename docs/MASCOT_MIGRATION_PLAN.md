# Mascot Migration Plan

Status: future work, gated on architecture approval and the validation spike
in [ASTRA_ARCHITECTURE_REPORT.md](ASTRA_ARCHITECTURE_REPORT.md). Nothing in this document authorizes or
schedules code migration now.

## Relationship

```text
Mascot existing UI  --(evidence / requirements)-->  rust-ui architecture
NOT
Mascot existing UI  --(mechanical extraction)----->  rust-ui
```

The Mascot prototype (`rceman/mascot`, branch
`agent/native-ui-component-gallery-v0.1-swe2`, reference HEAD
`50b2284c1f4e77e751fde75fed78651a4aa34bc7`) is a proof source, not the public
API. The list below maps proven requirements to the designed surfaces — it is
not a file-by-file port plan, and no reverse-engineering claims are made here.

## Conceptual mapping

| Proven Mascot capability | rust-ui surface (post-approval) | Owner after split |
|---|---|---|
| Surface/Bubble, content/response surface | `ui.surface`, `Surface` props | rust-ui |
| Typography/Label | `ui.label`, `ColorRole` styling | rust-ui |
| IconButton/Button | `ui.button`, `ui.icon_button`, `ButtonVariant`, `ControlSize` | rust-ui |
| Tooltip | `.tooltip` chrome + `MotionToken::Tooltip` | rust-ui |
| Badge/Status pill | `ui.badge`, non-colour-only status | rust-ui |
| Separator | `ui.separator` | rust-ui |
| Lucide icons | `Icon` enum, offline-generated path data | rust-ui |
| Light/dark token styling | `Theme` + `ColorRole` + `ui.theme` | rust-ui |
| Event-driven idle rendering | scheduler + damage model | rust-ui |
| DPI-validated rendering | backend dp snapping; validation is spike evidence | rust-ui |
| Windowless RichEdit composer | `TextValue`/`TextEdit` contract + `TextPeer` backend trait | rust-ui (new implementation) |
| Transparent borderless composition | layered-surface capability — spike-gated | rust-ui (spike) |
| Mascot rig/bones/clips, agent/chat/provider logic | `ui.custom` + `frame_events` + task APIs consumed by product | Mascot |

## What a later extraction would do, in stages

1. **Spike first.** The Native Composer Contract Spike must pass its
   mechanical gates before any Mascot surface is declared eligible —
   especially the transparent-borderless-native-text and accessibility cases.
2. **Stand up rust-ui core against the contracts** in
   [PROGRAMMING_MODEL.md](PROGRAMMING_MODEL.md)/[STATE_AND_EVENTS.md](STATE_AND_EVENTS.md) — new code, not lifted
   modules. Mascot code may be read as a requirements reference during
   implementation. Deliberate review and adaptation of isolated generic
   native helpers or assets — a RichEdit host routine, offline-generated
   icon path data — is permitted where contracts, licenses and tests fit;
   it is not a blanket mandate that every proven low-level detail be
   reimplemented blind. What does not carry over is the old public API
   surface and the window-painter architecture.
3. **Port product surface last.** Mascot consumes `rust-ui` for components,
   layout, theme, motion and backends; Mascot keeps the rig, agent/chat
   semantics and provider/session logic — exposed through `ui.custom`,
   `ReplyService`-style consumer traits and `UiProxy`/task APIs.

## Accidental APIs rejected on purpose

These prototype shapes are evidence of feasibility, not contracts:

- the current painter/window structure — backend traits in
  [BACKEND_ARCHITECTURE.md](BACKEND_ARCHITECTURE.md) are designed from contracts, not transplanted;
- any HWND-bound text assumptions — the windowless peer is a geometry
  requirement, not a compatibility promise;
- prototype naming, file layout, and gallery wiring — no legacy symbol names
  are carried forward by inertia;
- product-specific composition (chat transcript shape, provider plumbing) —
  stays in Mascot, never enters rust-ui.

## Evidence required before declaring a Mascot surface migrated

- spike acceptance cases passing for the analogous capability (text editing,
  layered composition, streamed output, keyed rows, cancellation);
- deterministic component gallery evidence for each migrated widget
  (replicating the prototype's deterministic evidence tooling);
- DPI matrix results on real displays, with unavailable entries reported
  blocked rather than simulated;
- accessibility verification of native text on Narrator (Windows) and
  VoiceOver (macOS) — no duplicate text nodes;
- resource-baseline counts (nodes, peers, timers, tasks) returning to
  baseline after mount/reorder/remove cycles.

## Explicit non-goals for the migration stage

- no reverse engineering of Mascot internals into rust-ui APIs;
- no behavioural parity promises beyond the designed contracts;
- no wholesale catalog port — vocabulary follows product need only;
- no start before owner approval of the architecture and the spike.
