# Astra Architecture Report

Provenance: branch `agent/architecture-concept-astra`, baseline
`1f28aac4e1aedd69e860102aff8642ff94121dbe`, 2026-09-30.

Style review update: branch `agent/style-customization-model-review`,
baseline `768eb172509a45ed15cdf5156fe57268ccaf8b08` — the authoritative
styling definition now lives in
[STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md). Rationale in
brief: the owner's three workflows (FAST/CUSTOM/SURGICAL) are preserved over
one shared box/text style representation with no hidden styling system or
private renderer; component customization is capability-limited typed
patches rather than full replacement; independent border sides, corner
radii and padding are accepted; group/node opacity is deferred.

Shadow correction on the same branch over committed `2aa9a54962a7e12cfaa50d14588d0faf67f4f755`:
the earlier "all shadows deferred" stance was narrowed — a basic one-`Shadow`
outer box shadow is recommended for v0.1 after re-review found a cached
native shadow path in owner-supplied Mascot context (inspected read-only at
snapshot `14e576ff`; evidence of implementability, not a runtime validation
and no port). Only spread/inset/shadow lists stay deferred.

Optional-frontend correction over `0bf0add83fa3a7e4052d058414b55d458deb2f8d`:
the independent review's **APPROVE WITH ARCHITECTURE CHANGES** is addressed
by the authoritative reserved frontend boundary in
[STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md#future-optional-authoring-frontends-reserved).
Rust builders remain first; optional future CSS/generated authoring lowers
to the existing typed style/layout inputs, without replacing components,
layout algorithms, retained arena, resolved-style family, renderer, damage
or native-peer contracts. No generic property/value IR is introduced.
CSS parser/selectors/specificity/cascade remain rejected core dependencies,
out of scope for v0.1, but supported as optional future authoring. DOM/browser
layout, browser-relative sizing, CSS-dependent rendering and core dynamic
property bags remain rejected.

FAST/CUSTOM/SURGICAL and basic outer `Shadow` are accepted. The corrected
style architecture is ready for owner approval of the Windows spike, not
permission to begin it. The future-only stylesheet slot precedes inline Rust;
state matching/cascade finishes in its frontend, with no duplicate state
tracking. Recoverable external candidate publication, metadata-only semantics
and frontend-owned provenance are reserved, not implemented. Rust-only cost
is unchanged by design; CSS-enabled dynamic matching is not promised free.
Static explicit bindings may avoid a general selector engine, whereas general
selectors may still require compiled matching after build-time parsing.
Future CSS profile/interfaces/implementation require separate approval and
are not blockers for the current native-contract spike.

Status: architecture concept only — documentation, not final approval, not a
performance result, not native proof. All referenced Rust is proposed API
exercise material, unimplemented and unverified by a compiler.

## Recommended model

**F — a deliberately narrow hybrid**: typed message/update (model B), ordinary
fluent property builders, and event-driven declarative `Ui<M>` traversal
(model C) over one retained arena of native-capable widget records with keyed
structural reconciliation.

- App: plain `S` plus `fn update(&mut S, M, &mut UpdateCtx<M>)` and
  `fn view(&S, &mut Ui<M>)`; `App::new(state, update, view).title(..).run()`.
- Events are `Fn(...) -> M` factories; all mutation happens in `update`.
- Keyed reconciliation with parent-local, type-namespaced, equality-checked
  keys and generational `NodeId`s; stale node and task deliveries are fenced.
- Native text is a real platform peer behind `TextValue`/`TextEdit`
  revisions; custom content is an immutable `Rc<dyn CustomRender>` snapshot.
- Async is `cx.spawn(key, factory)` on a supplied `Executor`, generation-
  fenced cancellation, bounded `TaskSender`, plus `UiProxy` for thread
  workers. No mandated runtime.

## Tradeoffs accepted

- Whole-view rerun per update batch (worst-case O(N) traversed nodes) in
  exchange for zero subscription machinery; paint/layout/accessibility damage
  stays narrow, app computation is not signal-granular. Fits small product
  UI; enormous lists are explicitly deferred.
- Boxed `'static` message factories — allocation churn per view pass is
  accepted and measured by the spike, not promised away.
- `M: 'static` normally, `M: Send` for task/worker APIs — a documented bound,
  not erased.
- Erased internal node storage and message factories with no unsafe
  downcasting on the public surface.
- Mutation-in-update is a programming contract, not a type proof (interior
  mutability escapes exist).
- Rectangular, axis-aligned native peers only — no arbitrary transformed,
  translucent or masked live text.

## Rejected alternatives (detail in [ARCHITECTURE_OPTIONS.md](ARCHITECTURE_OPTIONS.md))

- **A builder+listeners**: behaviour lives on scattered listeners — even the
  strongest context-injected variant (`on_click(|state: &mut _| ...)`) has no
  single named place where all event behaviour is read. Declined for
  inspectability, not for capability.
- **B message+view values**: nearly identical programming model — same typed
  messages, `Fn` factories, no `Clone` needed; declined only on
  representation (a returned `View<Msg>` tree to reconcile vs. staged
  descriptors), not because it is heavier — allocation is the same class.
- **C immediate**: declined for implicit dispatch timing inside traversal,
  not inability — it can retain native peers and idle event-driven.
- **D signals**: declined for the implicit dependency graph and subscription
  ownership; explicit commands mean it is not "async by effects only".
- **E macro/DSL**: a syntax overlay over any backing model; declined for
  macro hygiene and generated-code diagnostics, not expressiveness.

F is chosen for named inspectable events, one update path, retained native
peers and a small runtime — subject to owner agreement and the spike — and is
not the shortest candidate (C and D are smaller; B slightly).

## Risk register

| # | Risk | Handling |
|---|---|---|
| R1 | UIA provider availability for windowless RichEdit unproven (generic host guidance is not proof) | dedicated spike case; HWND alternative changes layering and is not a silent fallback |
| R2 | `TxDrawD2D` exact platform/version coverage | documented-API target; owner sets minimum versions |
| R3 | Transparent borderless window with adjacent native text on macOS | spike requirement before Mascot eligibility |
| R4 | RichEdit host duties (input, caret/timer, invalidation, UIA embedding) underestimated | spike builds one real TextArea |
| R5 | Required DPI/retina displays unavailable | entries reported blocked, never simulated |
| R6 | O(N) traversal inadequate for large lists | declared tradeoff; 100/1000-row measurement; virtual lists deferred |
| R7 | Boxed-closure allocation churn | accepted; measured, not assumed |
| R8 | Generational fencing gaps (stale node/task deliveries) | deterministic duplicate-key/stale-node/cancel-resend tests |
| R9 | Interior mutability bypasses the update contract | documented contract; review discipline |
| R10 | Mailbox bounds/backpressure tuning | bounded counts recorded in spike; payload bytes stay product-owned |
| R11 | "Native" does not auto-solve accessibility (UIA/NSAccessibility embedding) | Narrator/VoiceOver spike cases with no duplicate text nodes |
| R12 | `M: Send` bound blocks !Send message types in task paths | documented split: app path only needs `'static` |

## Owner questions (with recommended answers)

1. Accept typed-`Msg` boilerplate and whole-view root traversal instead of a
   signal/DSL model? — **Recommended: yes.**
2. Accept rectangular, axis-aligned native peers (no arbitrarily transformed
   controls)? — **Recommended: yes.**
3. Approve a windowless-RichEdit (`ITextHost`/`ITextServices`, targeting
   `TxDrawD2D`) integration spike, with UIA support unproven? —
   **Recommended: approve the spike, not the outcome.**
4. Minimum supported Windows/macOS versions? — unset without owner;
   **recommended: currently maintained desktop versions, validated on actual
   machines.**
5. Approve one crate with private modules initially? — **Recommended:
   approve.**
6. Accept deferred large lists and deferred future controls? —
   **Recommended: accept.**
7. Approve the spike below only — explicitly not Mascot migration? —
   **Recommended: approve.**

## Proposed v0.1 inventory

**Surface**: `App` (`new`/`title`/`executor`/`proxy`/`run`), `Ui<'ui, M>`
containers (`row`, `column`, `stack`, `surface`), leaf builders (`label`,
`button`, `icon`, `icon_button`, `badge`, `separator`, `text_input`,
`text_area`, `image`, `custom`), primitives (`box_`, `text`, `action`),
`group`/`scope`/`keyed`, `theme`, `appearance`, `visibility`,
`UiResult`/`UiError`/`UiDiagnostic` (incl. `InvalidStyle`,
`InvalidComposition`).

**State/events**: `UpdateCtx` (`spawn`, `cancel`, `scope`), the v0.1 rows of
the event table in [STATE_AND_EVENTS.md](STATE_AND_EVENTS.md) (press, pointer enter/leave/down/up,
focus/blur, key, edit, submit, selection-changed, conflict, frame).

**Text**: `TextValue`, `TextEdit`, `TextRevision`, `BindingToken`,
`EditOrigin`, `AcceptOutcome`, `TextSelection`, `TextConflict` +
`keep_native`, `SubmitPolicy`.

**Layout/style**: `Length`, `dp`, `Align`, `Justify`, `Space`, `Visibility`, `Theme` +
`ThemeMode` + `Theme::resolve` + `Appearance`, `ColorRole` (incl.
`DestructiveForeground`, `Shadow`), `Radius`, `MotionToken`, `ReducedMotion`,
`ButtonVariant`, `ControlSize`, `Icon` enum. Style vocabulary per
[STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md): `Color`,
`Insets`, `CornerRadii`, `Border`/`BorderSide`, `BoxStyle`, `TextStyle`,
`TextSize`/`TextWeight`, `VisualStyle`, `BoxProps`, `Action`/`ActionStyle`,
`StateStyles`, `Shadow`, and the patch family `BoxStylePatch`/`BorderPatch`/
`CornerRadiiPatch`/`InsetsPatch`/`TextStylePatch`/`VisualStylePatch`/
`ButtonStylePatch`/`TextInputStylePatch`/`ShadowPatch` — internally resolved
as `ResolvedBoxStyle`/`ResolvedTextStyle`/`ResolvedVisualStyle`/
`ResolvedShadow`.

**Async/custom**: `Executor`, `BoxFuture`, `CancelToken`, `TaskSender`, `Job`,
`UiProxy`, typed errors; `CustomRender`, `Canvas`, `Semantics`,
`ImageSource`/`AssetError`.

**Deferred**: Checkbox, Switch, RadioGroup, Select, Popover, ScrollArea,
Dialog, `on_open_change`/`on_scroll`/`on_change`, Activity component,
baseline alignment, group/node opacity and shadow spread/inset/lists
(deferred, not rejected), Linux backend,
virtualized large lists, GPU-required rendering, gallery example, effects
test harness (recording `UpdateCtx`/virtual clock), any Mascot migration.
Optional CSS parser/matching/cascade, watcher/hot reload and selector metadata/
APIs are separately deferred implementation, not core dependencies or
Windows-spike work; their authoring boundary is reserved above.

## Bounded spike: Native Composer Contract Spike

Only after owner approval, and **phased**: the immediate next step is a
Windows-only phase — headless resolver/patch regression tests plus one
opaque native Windows proof window containing a baseline `Primary` button, a
surgically patched variant, a custom `ui.action` built from public
primitives, and one native `TextArea`, exercising the styling gates in
[STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md#required-windows-spike-tests-planned--none-executed).
That first opaque window is a starter harness, not completion of the phase:
all the Windows-relevant native/DPI/IME/UIA/identity/task/cancellation/
resource and transparent-borderless gates below remain necessary. The macOS
eligibility requirements — including every previously deferred proof — are
unchanged and remain a separately approved, later cross-platform validation
phase, not part of the immediate action and not started now.

Contents: a Label, a Button, one native TextArea, a conditional panel, three
keyed editable rows, a PNG/custom-painted tile, a tooltip, a deterministic
in-process stream with cancellation, light/dark switching, reduced-motion
honoring. A fake service — no providers, no network, no Mascot migration, no
entire catalog, no GPU requirement, no macro/signal experiment.

Acceptance cases:

- edit/undo/select/IME preserved across rerender, theme switch and DPI
  changes;
- Windows 100/125/150/200% DPI and macOS 1x/2x on actual displays —
  unavailable matrix entries are reported blocked, not simulated;
- emoji, combining characters, RTL, CJK IME paths;
- reorder keeps focus on the stable row key; removal releases the native
  peer and cancels node events;
- cancel-then-resend: stale stream chunks and completion cannot modify the
  new output;
- two scoped children on the same task key do not cancel each other;
- a queued replace applies only if its base survives composition; a changed
  committed revision emits `TextConflict` and `keep_native` preserves native
  input; removal during composition retains the last acknowledged text and
  fences teardown callbacks (in-progress composition loss is recorded, not
  promised lossless);
- same-value acknowledgements and repeated view passes do not duplicate or
  echo native edits;
- window close with queued work produces no after-free callback;
- screen-reader keyboard traversal/activation plus native text semantics on
  Narrator and VoiceOver, without duplicate text nodes;
- typed unsupported-composition diagnostics where layering is impossible;
- transparent borderless layout with an adjacent native editor — required
  before claiming Mascot eligibility.

Resource/idle gates:

- with the window exposed, editor unfocused, no active task or motion:
  30 seconds of measurement shows zero rust-ui animation callbacks and zero
  requested redraws after initial settle (OS expose and other external events
  recorded separately; a focused native caret is an allowed separate case,
  not a violation);
- after 100 mount/reorder/remove cycles, live node/peer/timer/task counts
  return to baseline; caches reported separately — RSS is not equated to
  leaks;
- duplicate-key, stale-node and stale-task tests are deterministic.

Recorded, never fabricated: release binary size, dependency graph, cold
startup, redraw counts, allocations for an unchanged traversal and for one
text append, and update/layout/paint times at 100 and 1000 repeated display
rows. The 1000-row number exposes the root-traversal limit — it is not a
virtual-list promise. No performance thresholds are inferred from prior
Mascot results.

Gate order: every mechanical native/identity/text/safety case passes before
any efficiency claim. If accessibility, IME or layering fails, stop and
revise the peer contract — do not write a custom editor and do not expand
the framework.

## Document coverage matrix

| Task requirement | Covered in |
|---|---|
| Candidate models A-F comparison | [ARCHITECTURE_OPTIONS.md](ARCHITECTURE_OPTIONS.md) |
| Existing-systems study + references | [ARCHITECTURE_OPTIONS.md](ARCHITECTURE_OPTIONS.md) (References) |
| Ten required API exercises | [API_EXAMPLES.md](API_EXAMPLES.md) 1-10 |
| Basic layout vocabulary | [LAYOUT_STYLE_MOTION.md](LAYOUT_STYLE_MOTION.md) (Layout primitives) |
| Component vocabulary + future fit | [API_EXAMPLES.md](API_EXAMPLES.md) (Future controls), v0.1 inventory above |
| Event ownership/syntax table | [STATE_AND_EVENTS.md](STATE_AND_EVENTS.md) (Events) |
| Dynamic UI (conditionals, keyed, theme, send/stop, stream) | [API_EXAMPLES.md](API_EXAMPLES.md) 3-8 |
| State ownership/lifecycle | [STATE_AND_EVENTS.md](STATE_AND_EVENTS.md), [PROGRAMMING_MODEL.md](PROGRAMMING_MODEL.md) |
| Native text boundary | [NATIVE_CONTROL_BOUNDARY.md](NATIVE_CONTROL_BOUNDARY.md) |
| Custom rendering extension | [NATIVE_CONTROL_BOUNDARY.md](NATIVE_CONTROL_BOUNDARY.md), [API_EXAMPLES.md](API_EXAMPLES.md) 9 |
| Styling/theme model | [LAYOUT_STYLE_MOTION.md](LAYOUT_STYLE_MOTION.md), [STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md) (authoritative) |
| Motion model + idle invariant | [LAYOUT_STYLE_MOTION.md](LAYOUT_STYLE_MOTION.md) (Motion) |
| Async model | [STATE_AND_EVENTS.md](STATE_AND_EVENTS.md) (Async), [API_EXAMPLES.md](API_EXAMPLES.md) 5-6 |
| Runtime/tree model | [PROGRAMMING_MODEL.md](PROGRAMMING_MODEL.md) |
| Backend/crate boundary | [BACKEND_ARCHITECTURE.md](BACKEND_ARCHITECTURE.md) |
| Efficiency criteria | [PROGRAMMING_MODEL.md](PROGRAMMING_MODEL.md) (Invalidation), spike metrics above |
| Agent-friendliness comparison | [ARCHITECTURE_OPTIONS.md](ARCHITECTURE_OPTIONS.md) (+ source-volume section) |
| Mascot migration path | [MASCOT_MIGRATION_PLAN.md](MASCOT_MIGRATION_PLAN.md) |
| Stop condition | docs only — no crates, CI or code files added |

RUST_UI_ARCHITECTURE_CONCEPT_COMPLETE
