# rust-ui Architecture Concept Task

**Phase:** architecture/design only  
**Implementation:** intentionally deferred  
**Primary question:** what programming model should rust-ui expose so native Rust UI is as easy to compose as modern Vue/Svelte-style application UI while remaining small, strongly typed, native and efficient?

## 1. Objective

Design the public architecture and programming model for a reusable native Rust UI component library.

The target relationship is:

```text
rust-ui
  = generic native UI components/runtime/backends

mascot
  = product
  = animated mascot + agent/chat logic + providers
  = consumer of rust-ui
```

rust-ui must not know about:

- Mascot rigs/bones/clips;
- Codex/Devin/providers;
- agent/session semantics;
- product response protocols.

A product-specific animated mascot should appear to rust-ui as an image/custom render node plus events/hit geometry.

## 2. Design goals

The API should be:

- concise and pleasant to write;
- low-boilerplate;
- strongly typed;
- easy for both humans and coding agents;
- dynamic/reactive;
- composable;
- deterministic and testable;
- efficient at runtime;
- event-driven when idle;
- compatible with native controls;
- portable across Windows/macOS and later Linux;
- understandable without framework magic.

The ergonomics should feel closer to Vue/Svelte than manual Win32/AppKit wiring.

Do not achieve that by importing browser architecture.

## 3. Hard constraints

Do not require:

- JS/TypeScript;
- DOM;
- browser/WebView;
- CSS parser/cascade/selectors;
- Tailwind runtime;
- React-style virtual DOM by default;
- permanent 60/120 Hz render loop;
- Tokio as a mandatory runtime;
- custom text editing merely to keep abstractions pure;
- a universal GUI framework larger than Mascot actually needs.

No implementation migration from Mascot belongs in this task.

## 4. Candidate programming models to compare

Astra must compare at least these models.

### A. Fluent builder + listeners

```rust
Button::new("Send")
    .icon(Icon::ArrowUp)
    .variant(ButtonVariant::Primary)
    .disabled(state.busy)
    .on_click(on_send)
```

Evaluate callback ownership/lifetimes, async work, composition, agent token cost and testability.

### B. Typed message/update model

```rust
fn view(state: &State) -> View<Message> { ... }

fn update(state: &mut State, msg: Message) { ... }
```

Evaluate Elm/Iced-like determinism versus verbosity.

### C. Immediate declarative builder

```rust
ui.column(|ui| {
    ui.label("Ready");
    if ui.button("Send").clicked() {
        // ...
    }
});
```

Evaluate native controls, focus, retained animation state, accessibility and idle behavior.

### D. Fine-grained reactive signals

```rust
let busy = signal(false);
let can_send = derived(|| !busy.get() && !text.get().is_empty());
```

Evaluate invalidation precision, ownership, debugging, async events and hidden runtime complexity.

### E. Declarative macro/DSL

```rust
view! {
    Column(gap: 8) {
        TextInput(value: state.text)
        Button(
            text: "Send",
            icon: Icon::ArrowUp,
            disabled: state.busy,
            on_click: Message::Send,
        )
    }
}
```

Evaluate rustfmt/IDE/error quality, macro complexity and agent friendliness.

### F. Minimal hybrid

Explicitly test whether the best model is a narrowly defined hybrid, for example:

- declarative component tree;
- typed messages/actions;
- small reactive state primitives;
- native platform controls behind shared contracts;
- targeted retained state for focus/motion;
- no generalized VDOM.

Do not combine every model merely because each has one attractive property.

## 5. Existing systems to study for ideas, not dependencies

Study the programming-model strengths/weaknesses of relevant systems, including where useful:

- Vue;
- Svelte;
- Solid;
- Leptos;
- Dioxus;
- Iced;
- egui;
- Floem;
- Xilem;
- GPUI;
- Slint.

The deliverable must distinguish:

- syntax ergonomics;
- state model;
- invalidation model;
- event model;
- renderer/tree ownership;
- native-control interoperability;
- async integration;
- accessibility implications;
- compile-time/runtime cost.

Do not select a framework merely because its syntax is attractive.

## 6. Required API exercises

The recommended model must show concise consumer code for all of these.

### Basic layout

- Row
- Column
- Stack/Overlay
- padding/gap/alignment
- fixed/content/fill/min/max sizing
- reusable nested components

Avoid recreating full CSS/Flexbox unless a concrete need makes a smaller model insufficient.

### Current component vocabulary

- Surface
- Label/Text
- Icon
- Image
- Button
- IconButton
- Tooltip
- Badge
- Separator
- TextInput/TextArea
- content/response surface

Also show how future:

- Checkbox
- Switch
- RadioGroup
- Select
- Popover
- ScrollArea
- Dialog

fit without redesigning the core.

### Events

Show the ownership and syntax for:

- click;
- pointer enter/leave;
- pointer down/up;
- focus/blur;
- keyboard events;
- text changed;
- submit;
- selection changed;
- open/close;
- custom semantic events.

Avoid callback jungle.

Astra must make a concrete recommendation on whether the primary app-facing model is based on:

- closures;
- typed messages;
- commands/actions;
- subscriptions;
- signals;
- or a deliberate combination.

### Dynamic UI

Show:

- if/else conditional UI;
- dynamic visibility;
- enabled/disabled state;
- keyed lists;
- changing labels/icons;
- theme switching;
- Send -> busy/Stop -> Send;
- streamed response updates;
- async completion/cancellation.

### State ownership

Define:

- application state;
- local component state;
- ephemeral interaction state;
- derived/computed state;
- immutable vs mutable updates;
- state preservation/removal rules;
- invalidation/damage propagation.

### Native text control boundary

Public API should be shared while implementation remains native.

Example concept:

```rust
TextInput {
    value,
    placeholder,
    multiline,
    max_lines,
    ...
}
```

Backend examples:

```text
Windows -> RichEdit
macOS   -> NSTextView
```

Explain layout, focus, clipping, z-order, theme, accessibility, events and transitions around embedded native controls.

Do not design a custom text editor.

### Custom rendering extension

Show a narrow extension point so a consumer can:

- display a PNG;
- provide custom animated content;
- receive pointer/hit events;
- layer native components around custom rendered content.

rust-ui must not become an animation engine.

## 7. Styling and theme model

Current design direction is shadcn-first, but rust-ui should represent the design system natively.

Design typed support for:

- theme tokens;
- component variants;
- component sizes;
- light/dark;
- interaction states;
- platform font choices;
- Lucide icons;
- motion tokens.

Do not add stringly typed CSS-like properties.

Compare likely forms such as:

```rust
ButtonVariant::Primary
Size::Sm
ThemeToken::Foreground
```

versus typed style structs and decide where each belongs.

## 8. Motion model

The model must support event-driven motion such as:

- hover/focus colour transition;
- tooltip fade/scale;
- active spinner when a product legitimately uses one;
- reduced-motion policy.

Invariant:

```text
no active transition -> no animation timer -> no continuous redraw
```

Explain animation scheduling and invalidation.

## 9. Async model

Show an ergonomic pattern for:

- click -> command/future;
- cancellation;
- completion -> UI update;
- streamed events;
- UI-thread dispatch;
- background task integration.

rust-ui should integrate with application runtimes rather than mandate Tokio unless a strong reason exists.

## 10. Runtime/tree model

Astra must explicitly decide:

- retained tree vs immediate reconstruction;
- whether reconciliation exists;
- identity/key semantics;
- component instance lifetime;
- focus ownership;
- state slots;
- native child ownership;
- event routing;
- hit testing;
- damage/invalidation;
- accessibility-tree projection.

If a tree is rebuilt declaratively, explain how rust-ui avoids unnecessary allocation/work.

If a retained tree is mutated, explain how consumer code stays ergonomic.

## 11. Cross-platform backend boundary

Propose package/crate ownership.

One plausible shape to evaluate:

```text
crates/
  rust-ui-core/
  rust-ui-components/
  rust-ui-icons/
  rust-ui-win32/
  rust-ui-macos/
  rust-ui-lab/
```

Do not accept this structure merely because it is suggested.

The backend boundary should allow platform-native implementation differences without leaking them through every app component.

## 12. Efficiency criteria

The architecture should aim for:

- effectively zero idle rendering work;
- bounded component/tree state;
- narrow invalidation;
- no runtime SVG parser;
- no browser engine;
- no permanently initialized GPU stack required for simple UI;
- low allocation churn during common state updates;
- compact release dependency surface;
- minimal source/token boilerplate for consumers.

Astra should discuss likely performance implications of each programming model, but this task does not benchmark an implementation.

## 13. Agent-friendliness

This is an explicit design criterion.

Compare example implementations by:

- LOC;
- approximate source/token volume;
- duplicated boilerplate;
- number of concepts required for a simple component;
- ease of locating event/state ownership;
- compiler diagnostics;
- ability to make surgical edits;
- likelihood of agents inventing inconsistent patterns.

Prefer one obvious way to express common UI.

## 14. Deliverables

Create architecture documentation only.

Required:

```text
docs/
  ARCHITECTURE_OPTIONS.md
  PROGRAMMING_MODEL.md
  STATE_AND_EVENTS.md
  LAYOUT_STYLE_MOTION.md
  NATIVE_CONTROL_BOUNDARY.md
  BACKEND_ARCHITECTURE.md
  API_EXAMPLES.md
  MASCOT_MIGRATION_PLAN.md
  ASTRA_ARCHITECTURE_REPORT.md
```

### ARCHITECTURE_OPTIONS.md

Compare candidate models A-F and relevant existing frameworks.

### PROGRAMMING_MODEL.md

Define the recommended public model and why.

### STATE_AND_EVENTS.md

Define state ownership, events/messages/listeners, async integration and lifecycle.

### LAYOUT_STYLE_MOTION.md

Define minimal layout primitives, typed styling/theme and event-driven motion.

### NATIVE_CONTROL_BOUNDARY.md

Define RichEdit/NSTextView integration and generic custom-render extension points.

### BACKEND_ARCHITECTURE.md

Define crate ownership and Windows/macOS backend boundaries.

### API_EXAMPLES.md

Show complete concise examples:

1. counter/button;
2. form/text input;
3. conditional panel;
4. keyed list;
5. async Send/Cancel;
6. streaming text;
7. tooltip;
8. theme switch;
9. custom image/render node;
10. small chat/composer composition.

For each major example, include enough code to judge ergonomics honestly.

### MASCOT_MIGRATION_PLAN.md

Describe how the existing Mascot UI work could later be extracted/mapped into rust-ui **after architecture approval**.

Do not migrate code now.

### ASTRA_ARCHITECTURE_REPORT.md

Summarize:

- recommended model;
- rejected alternatives and why;
- unresolved risks;
- smallest validation spike needed before implementation;
- proposed v0.1 public API surface;
- exact questions requiring owner approval.

## 15. Decision quality

Do not optimize for novelty.

A successful design should let a consumer write common UI with code that feels roughly as straightforward as modern Svelte/Vue composition while preserving:

- native performance;
- Rust type safety;
- explicit ownership;
- native-control quality;
- deterministic event/state flow.

If this cannot be achieved without a large runtime/framework, say so clearly and propose the simplest compromise.

## 16. Stop condition

Do not begin implementation.

Do not port existing Mascot crates.

Do not copy current Win32 painter architecture into rust-ui as a fait accompli.

The task ends at an architecture recommendation plus a bounded implementation-spike plan.
