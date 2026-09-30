# Architecture Options

Status: architecture concept only. Every code block below is a proposed API
exercise — normalized sketches of what each model would look like, not calls
into real framework APIs and not compiler-verified.

This document compares the six candidate programming models from
[RUST_UI_ARCHITECTURE_CONCEPT_TASK.md](RUST_UI_ARCHITECTURE_CONCEPT_TASK.md)
(A-F), then summarizes what relevant existing systems actually do (verified
facts) versus what we infer their model would cost us (our analysis).

## The same counter, six ways

Each block is one counter under that model — initial state, handlers, one
label and one button, and the launch call. Imports are omitted consistently
across all six.

### A. Fluent builder + listeners

```rust
fn counter() -> View {
    let count = Rc::new(Cell::new(0_i32));
    let label = Label::new("0");
    let label_ref = label.downgrade();
    let button = Button::new("Increment").on_click(move || {
        let next = count.get() + 1;
        count.set(next);
        if let Some(label) = label_ref.upgrade() {
            label.set_text(next.to_string());
        }
    });
    Column::new()
        .gap(Space::Sm)
        .child(label)
        .child(button)
        .into()
}

fn main() -> UiResult {
    App::from_view(counter()).run()
}
```

The snippet above is the handle-oriented variant of A: the listener mutates
shared state and pokes a peer widget through a weak handle. A stronger
variant exists — a context-supplied `on_click(|state: &mut Counter| ...)`
callback can mutate the model directly without any `Rc`/weak graph — and is
a legitimate runner-up shape. Either way, event behavior lives on individual
listeners rather than in one inspectable place, and cross-widget reads keep
needing some ownership plumbing the type system does not name.

### B. Typed message/update (view tree values)

```rust
#[derive(Default)]
struct Counter {
    value: i32,
}

enum Msg {
    Increment,
}

fn update(state: &mut Counter, msg: Msg) {
    match msg {
        Msg::Increment => state.value += 1,
    }
}

fn view(state: &Counter) -> View<Msg> {
    View::column()
        .gap(Space::Sm)
        .child(View::label(state.value.to_string()))
        .child(View::button("Increment").on_press(|| Msg::Increment))
}

fn main() -> UiResult {
    App::new(Counter::default(), update, view).run()
}
```

B shares almost the whole programming model with F: typed messages, one
`update`, `Fn` factories (no `Clone` bound needed — a factory like
`|| Msg::Increment` constructs the message at dispatch). The difference is
representation: `view` returns a `View<Msg>` value tree which the runtime
reconciles into retained widgets, where F writes staged descriptors into the
retained arena directly. View values can be stack-allocated or reused; a
returned tree is not inherently a second heap-allocated browser DOM — but it
is still a second owned tree of descriptors that must be diffed.

### C. Immediate declarative traversal

```rust
fn main() -> UiResult {
    ImmediateApp::new(0_i32, |count, ui| {
        ui.column(Column::new().gap(Space::Sm), |ui| {
            ui.label(count.to_string());
            if ui.button("Increment").clicked() {
                *count += 1;
                ui.request_repaint();
            }
        });
    })
    .run()
}
```

The fewest physical lines in this normalized exercise. `.clicked()` reads
naturally, and an immediate facade
can sit over retained native peers with event-driven idle — neither is
contradicted by the syntax. The cost is semantic, not mechanical: mutation
and event dispatch interleave inside traversal, so "when does this run" and
"what can this event touch" stop being explicit, and event order becomes an
artifact of widget visitation order.

### D. Fine-grained reactive signals

```rust
fn main() -> UiResult {
    ReactiveApp::new(|| {
        let value = signal(0_i32);
        column(
            Space::Sm,
            (
                label(move || value.get().to_string()),
                button("Increment").on_press(move || value.update(|n| *n += 1)),
            ),
        )
    })
    .run()
}
```

Invalidation is precise — only `label` re-reads `value` — and async work can
be launched as explicit commands, not only as effects; callbacks are not
inherently untestable. The cost is the implicit dependency graph: `label`
subscribes to `value` because a closure happened to call `get()`. Ownership
and disposal of those subscriptions live in scope machinery outside the
visible code, and debugging means inspecting a runtime graph.

### E. Declarative macro/DSL

```rust
#[derive(Default)]
struct Counter {
    value: i32,
}

enum Msg {
    Increment,
}

fn update(state: &mut Counter, msg: Msg) {
    match msg {
        Msg::Increment => state.value += 1,
    }
}

fn view(state: &Counter) -> View<Msg> {
    view! {
        Column(gap: Space::Sm) {
            Label(text: state.value.to_string())
            Button(text: "Increment", on_press: || Msg::Increment)
        }
    }
}

fn main() -> UiResult {
    App::new(Counter::default(), update, view).run()
}
```

E is a syntax overlay: the `Msg`/`update` (or whichever) backing model still
exists underneath, so the overlay is *in addition to* a model, not a
replacement for one. Its real costs are macro hygiene, rustfmt and IDE gaps,
and diagnostics that surface inside generated code rather than at the call
site.

### F. Minimal hybrid (recommended)

```rust
#[derive(Default)]
struct Counter {
    value: i32,
}

enum Msg {
    Increment,
}

fn update(state: &mut Counter, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
    match msg {
        Msg::Increment => state.value += 1,
    }
}

fn view(state: &Counter, ui: &mut Ui<Msg>) {
    ui.column(Column::new().gap(Space::Sm), |ui| {
        ui.label(state.value.to_string());
        ui.button("Increment").on_press(|| Msg::Increment);
    });
}

fn main() -> UiResult {
    App::new(Counter::default(), update, view).run()
}
```

F keeps B's typed messages and single mutation point, C's `ui.*` traversal
over a retained node arena, and ordinary fluent property builders — while
staging property/structural descriptors into transaction buffers rather than
building a returned value tree.

## Dimension comparison

| Dimension | A builder+listeners | B message/update | C immediate | D signals | E macro/DSL | F minimal hybrid |
|---|---|---|---|---|---|---|
| Syntax | fluent, verbose at scale | enum + match boilerplate | shortest | short, hidden wiring | terse block | fluent leaves + factories |
| State / invalidation | manual per-listener | whole-view rebuild | whole-view rerun | implicit dep graph, fine-grained | inherits backing model | whole-view rerun, narrow damage |
| Event ownership | scattered listeners | single `update` | inline in traversal | closures mutate signals | routes to backing model | single `update` |
| Tree / renderer | builder-owned widgets | returned view values | immediate calls | reactive node graph | overlay syntax | retained arena + keyed reconcile |
| Native peers / focus / a11y | direct but manual | via reconciliation | retainable behind facade | via runtime graph | via backing layer | retained peers, generational ids |
| Async | ad-hoc per listener | commands/messages | callback + queue | commands or effects | inherits backing | `cx.spawn` + typed `M` delivery |
| Compile/runtime cost | low compile; cycles in handle variant | staged descriptors; moderate monomorph. | low | runtime graph + subscriptions | macro expansion | modest monomorph., boxed closures |
| Human/agent ergonomics | several patterns | one pattern, three sites per action | one pattern, ambiguous timing | high concept count | extra language | one pattern, three sites per action |

Where the cells are honest rather than flattering:

- **Invalidation.** D offers automatic read-tracked fine-grained
  invalidation; A can target updates manually, and B/C/F can add explicit
  memoization — which F deliberately defers. F accepts an O(N) traversal per
  update batch and narrows damage (layout/paint/accessibility) instead of
  narrowing app computation.
- **Event ownership.** B and F funnel every event into one `update` — the
  property that makes the flow greppable and auditable. A spreads behavior
  across listeners even in its context-injected strongest form; C interleaves
  dispatch with traversal; D's closures mutate signals directly; E delegates
  to whichever backing model it targets.
- **Allocation parity.** F's staged transaction buffers and B's returned
  `View<Msg>` descriptors are the same cost class — F staging descriptors is
  not evidence of less allocation. The difference is that F's descriptors are
  diffed into a retained arena with generational node identity, which is what
  keeps native text peers, focus and accessibility stable across re-runs;
  B reconciles equivalent identity through the returned tree instead.

## Form and async implications

- **Form editing.** Under A each field is a handle plus listener wiring
  (weakest in the handle variant, lighter with context-injected state). Under
  D each field is a signal; write loops are a real hazard. Under F a
  `TextValue` field plus `.on_edit(Msg::Edited)` keeps ownership in app state
  while the native peer owns IME/undo (see
  [NATIVE_CONTROL_BOUNDARY.md](NATIVE_CONTROL_BOUNDARY.md)).
- **Async.** A's listeners can launch work but need ad-hoc proxies to return
  results; B/F route every result through `update` as a message, so
  cancellation, busy state and stale replies are ordinary match arms (see
  [STATE_AND_EVENTS.md](STATE_AND_EVENTS.md)). D can use explicit commands
  equally well; folding async into tracked effects instead would re-derive
  the invisible-graph problem.
- **Composite components.** Under A a child is handles plus registration;
  under F a child is an ordinary struct plus `ui.scope(key, map, draw)` —
  no extra machinery.

## What to borrow / what not to import

"Borrow" items are our inference about what transfers to F, not features we
would copy. Facts about each system are in References below.

| System | What to borrow | What not to import |
|---|---|---|
| Vue | props/computed clarity; compiled-template ergonomics goal | proxy dependency tracking; cascade; browser/VDOM layer |
| Svelte | rune/template brevity as an ergonomic bar | the compiler-as-language toolchain |
| Solid | targeted invalidation as inspiration for narrow damage | dynamic subscription graph at runtime |
| Leptos | typed reactive composition within Rust | signal ownership/capture ergonomics |
| Dioxus | typed components, tooling ambition | returned VDOM tree values; its backend split |
| Iced | typed `Message` + `update`/`view`, Task/Subscription determinism | the returned generic view tree — deliberately not chosen |
| egui | traversal ergonomics; `request_repaint` event-driven idle | `.clicked()` direct dispatch semantics |
| Floem | native reactive composition example | signals, Taffy flex/grid scope, style breadth |
| Xilem | strongly typed Rust view composition | view+element state reconciliation layering |
| GPUI | scoped ownership/actions, custom element extension | entity/context breadth, GPU-first orientation |
| Slint | property/callback declaration clarity | separate DSL, compiler and toolchain |

## Human and agent ergonomics

- In B/E/F a new action is declared in three places — the enum variant, the
  `update` match arm, and the `.on_*` binding — which is exactly what makes
  it greppable: `Msg::Send`, `fn update` and the call site are all named
  locations. A and C have fewer declarations but no single point where event
  behavior can be read end to end.
- Pure helpers (`fn draw_panel(ui, props)`) and child structs are ordinary
  Rust — reuse costs nothing extra; there is no component-registration
  machinery for an agent to get subtly wrong.
- `.on_*` factories report capture mistakes as normal Rust errors at the
  call site. A DSL can push the same mistakes through generated code — macros
  may produce indirect diagnostics or require extra span tooling to point
  back at the author's line.
- Surgical edits are easy: adding a button or a message touches one spot at
  a time. The realistic agent failure mode is mixing paradigms — reaching for
  signals or listener handles mid-file — which the single-pattern rule and
  absence of those APIs prevent.

## Recommendation

F, as defined in [PROGRAMMING_MODEL.md](PROGRAMMING_MODEL.md). It is not the
shortest source — C and D are shorter and B is slightly shorter in the frozen
measurement below. F is chosen because it trades that brevity for named,
inspectable events, one update path, retained native peers and a small
runtime — predictable edits and ownership over minimal characters — subject
to owner agreement and the validation spike.

## Source-volume comparison

Normalized snippets above, measured over the fenced contents exactly as
displayed: splitlines for physical lines, nonblank lines for LOC, UTF-8 byte
count including one trailing newline, and `round(char_count / 4)` as a rough
token proxy.

| Model | Physical lines | Nonblank LOC | UTF-8 bytes | Approx. char/4 token proxy |
|---|---|---|---|---|
| A | 21 | 20 | 543 | 136 |
| B | 25 | 21 | 484 | 121 |
| C | 12 | 12 | 326 | 82 |
| D | 13 | 13 | 320 | 80 |
| E | 27 | 23 | 495 | 124 |
| F | 25 | 21 | 511 | 128 |

Method and caveats: no imports, dependency manifests, backend or framework
internals counted for any model; the snippets are ASCII so chars equal bytes.
The proxy is not a tokenizer measurement or an agent-cost benchmark, and
formatting changes the figures. Model A shown is the handle-oriented variant,
not a universal lower bound; context-injected variants can avoid the
displayed handle plumbing — none was measured here. Full forms and tasks
would add lifetime/error/scope code to every model — a toy counter does not
prove overall productivity.

## References

Accessed 2026-09-30. These are mutable, versioned documentation pages;
descriptions below record what they documented at that date, not a guarantee
about later releases.

- Vue — <https://vuejs.org/guide/extras/reactivity-in-depth.html> — Proxy-based
  dependency tracking plus a traditional virtual DOM. Not all Vue rendering
  uses the VDOM: the docs discuss the Vapor mode direction. (external fact)
- Svelte 5 — <https://svelte.dev/docs/svelte/$state/> and
  <https://svelte.dev/docs/svelte/$effect> — compiler runes ($state/$effect)
  with runtime dependency tracking; reads after `await` in an effect are not
  tracked, so it is not compile-time-only tracking. (external fact)
- Solid — <https://docs.solidjs.com/advanced-concepts/fine-grained-reactivity>
  — signals/observers with dynamic dependencies and targeted DOM mutation.
  (external fact)
- Leptos — <https://book.leptos.dev/reactivity/interlude_functions.html> —
  components run setup once; reactive closures/signals carry ownership and
  subscription costs; Rust closure capture adds ergonomic complexity.
  (external fact)
- Dioxus — <https://docs.rs/dioxus/0.7.10/dioxus/prelude/struct.VirtualDom.html>
  — virtual tree diff; signal adoption does not remove the VDOM. Do not claim
  all targets require a WebView — current docs list dioxus-native.
  (external fact)
- Iced — <https://docs.iced.rs/iced/> (observed 0.15.0-dev) — typed
  Message + update + view with Task/Subscription; deterministic data flow.
  Boxed callbacks are a cost we note, not a benchmarked claim. (external fact)
- egui — <https://docs.rs/egui/latest/egui/struct.Context.html> —
  `request_repaint`/`request_repaint_after` show immediate mode can be
  event-driven idle; immediate does not mean a mandatory 60 Hz loop.
  (external fact)
- Floem — <https://docs.rs/crate/floem/0.2.0> — native fine-grained signals,
  Taffy flex/grid layout, GPU plus CPU fallback (not GPU-only). This is a
  historical version page; no claim about latest head. (external fact)
- Xilem — <https://github.com/linebender/xilem/blob/main/xilem/ARCHITECTURE.md>
  — strongly typed view tree rebuilt and diffed into a retained element tree;
  no DSL required. Generic type/compile-size cost is inference, not measured.
  (external fact + our inference)
- GPUI — <https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md>
  — entities, render/elements hybrid, context/executor, GPU acceleration.
  Tailwind-like Rust styling is not automatically a CSS runtime.
  (external fact)
- Slint — <https://docs.slint.dev/latest/docs/slint/guide/language/coding/functions-and-callbacks/>
  and <https://docs.slint.dev/latest/docs/rust/slint/> — separate language
  compiled to components with props/callbacks; pages warn they are
  unreleased/current mutable docs. More tooling/bindings cost; it does not
  force a browser runtime. (external fact)
- Windows windowless RichEdit —
  <https://learn.microsoft.com/en-us/windows/win32/controls/about-windowless-rich-edit-controls>
  (ITextHost supplies window services),
  <https://learn.microsoft.com/en-us/windows/win32/api/textserv/nf-textserv-itextservices2-txdrawd2d>
  (ITextServices2::TxDrawD2D documented for Windows 8+ — a documented API, not
  a minimum-OS choice), and
  <https://learn.microsoft.com/en-us/windows/win32/winauto/host-a-ui-automation-windowless-activex-control>
  (generic UIA host guidance — not proof a RichEdit UIA provider is
  available). (external facts; integration is untested design)
- macOS text —
  <https://developer.apple.com/library/archive/documentation/TextFonts/Conceptual/CocoaTextArchitecture/TextEditing/TextEditing.html>
  (first responder / marked text editing) and
  <https://developer.apple.com/documentation/appkit/nstextinputclient>
  (native NSTextView vs implementing NSTextInputClient yourself).
  (external facts)

Separately: [MASCOT_PROVEN_CONTEXT.md](MASCOT_PROVEN_CONTEXT.md) is supplied
product evidence (Windows-native rendering, windowless RichEdit, transparent
composition and event-driven idle are proven there); it is a requirements
source, not proof that the proposed API below works.
