# Programming Model

Status: architecture concept only. All Rust signatures below are a proposed
API exercise, not implemented or compiler-verified code.

## The model in one paragraph

An app is a plain Rust state value `S` plus two functions: `update`, the only
place `S` mutates, and `view`, a declarative traversal that describes the UI
through a `Ui<M>` visitor over one retained arena of native-capable widget
records. Events produce typed messages `M`; messages flow into `update`; the
root view reruns after each update batch. `M` is the message type parameter,
so composition is a typed adapter (`ui.scope`), never a global event bus.

## Canonical signatures

```rust
pub struct Ui<'ui, M>;

pub struct UpdateCtx<'cx, M>;

pub enum UiError {
    Platform(String),
    Asset(AssetError),
    Unsupported(&'static str),
    InvalidUi(UiDiagnostic),
}

pub enum UiDiagnostic {
    DuplicateKey,
    DuplicateTextBinding,
    InvalidLayout,
    InvalidStyle,
    InvalidComposition,
}

pub type UiResult = Result<(), UiError>;

pub struct App<S, M, U, V>;

impl<S: 'static, M: 'static, U, V> App<S, M, U, V>
where
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut Ui<'_, M>),
{
    pub fn new(state: S, update: U, view: V) -> Self;
    pub fn title(self, title: &str) -> Self;
    pub fn executor(self, executor: Arc<dyn Executor>) -> Self;
    pub fn proxy(&self) -> UiProxy<M>
    where
        M: Send;
    pub fn run(self) -> UiResult;
}
```

Notes on the signature set:

- There is no `Application` trait to implement; two functions are the whole
  contract. `App::new(state, update, view)` accepts `fn` items, free
  functions, or `impl` methods — all the same shapes.
- `S` lives on the UI thread for the whole run. State may be `!Send`; `M`
  needs only `'static` in ordinary use. Task and worker-facing APIs
  additionally require `M: Send`. Native handles never cross threads.
- `Ui<'ui, M>` carries a borrow lifetime. Examples write `&mut Ui<M>` with the
  lifetime elided; `Ui<'ui, M>` is the declared form and `Ui<M>` in parameter
  position is the same type.
- `UiResult` is the one app-level result; platform/asset failures convert into
  typed `UiError` variants rather than panicking.
- A builder that detects an invalid build (duplicate key, a `TextValue` bound
  to two mounted peers, a `Fill` on an unbounded axis, a malformed numeric
  style, an actionable/focusable node nested inside an `ui.action`) stages a
  typed `UiDiagnostic` with non-sensitive context — a key fingerprint and
  node kind, never user text. A fatal invalid build aborts the transaction
  before any native mutation and `run` returns `Err` after orderly teardown.
  This is for structural errors only: a `TextConflict` is an event, not a
  fatal error.

## Traversal, staging, commit

```rust
impl<'ui, M> Ui<'ui, M> {
    pub fn column(&mut self, props: Column, draw: impl FnOnce(&mut Ui<M>));
    pub fn row(&mut self, props: Row, draw: impl FnOnce(&mut Ui<M>));
    pub fn stack(&mut self, props: Stack, draw: impl FnOnce(&mut Ui<M>));
    pub fn surface(&mut self, props: Surface, draw: impl FnOnce(&mut Ui<M>));

    pub fn label(&mut self, text: impl AsRef<str>) -> LabelBuilder<'_, M>;
    pub fn button(&mut self, text: &str) -> ButtonBuilder<'_, M>;
    pub fn icon(&mut self, icon: Icon) -> IconBuilder<'_, M>;
    pub fn icon_button(&mut self, icon: Icon) -> IconButtonBuilder<'_, M>;
    pub fn text_input(&mut self, value: &TextValue) -> TextInputBuilder<'_, M>;
    pub fn text_area(&mut self, value: &TextValue) -> TextAreaBuilder<'_, M>;
    pub fn image(&mut self, source: ImageSource) -> ImageBuilder<'_, M>;
    pub fn custom(&mut self, render: Rc<dyn CustomRender>) -> CustomBuilder<'_, M>;
    pub fn badge(&mut self, text: impl AsRef<str>) -> BadgeBuilder<'_, M>;
    pub fn separator(&mut self) -> SeparatorBuilder<'_, M>;

    pub fn box_(&mut self, props: BoxProps, draw: impl FnOnce(&mut Ui<M>));
    pub fn text(&mut self, text: impl AsRef<str>) -> TextBuilder<'_, M>;
    pub fn action(
        &mut self,
        props: Action,
        draw: impl FnOnce(&mut Ui<M>),
    ) -> ActionBuilder<'_, M>;

    pub fn group(&mut self, key: impl KeyId, draw: impl FnOnce(&mut Ui<M>));
    pub fn scope<ChildMsg: 'static>(
        &mut self,
        key: impl KeyId,
        map: impl Fn(ChildMsg) -> M + 'static,
        draw: impl FnOnce(&mut Ui<ChildMsg>),
    );
    pub fn keyed<T, K: Eq + std::hash::Hash + Clone + 'static>(
        &mut self,
        items: &[T],
        key: impl Fn(&T) -> K,
        draw: impl Fn(&mut Ui<M>, &T),
    );
    pub fn theme(&mut self, theme: Theme);
    pub fn appearance(&self) -> Appearance;
}
```

The traversal never mutates the arena directly. Property and structural
changes accumulate in reusable transaction buffers while `view` runs; after
`view` returns, one commit applies them to the arena and only then do native
notifications dispatch. Two consequences follow:

- A container call returns no long-lived borrow once its closure returns — the
  child `&mut Ui<M>` is scoped to the closure body.
- Same-value props detected during staging produce no downstream layout,
  paint, or accessibility work.
- `label`/`badge` take `impl AsRef<str>`: a borrowed `&str` is compared
  against the retained value and copied only when changed — `format!`-built
  strings still allocate as usual.
- `ui.box_` is the noninteractive painted box, `ui.text` the styled text
  leaf, and `ui.action` the semantic activation wrapper for bespoke
  controls — the public primitive vocabulary shared by the built-in
  recipes. The authored style types (`BoxStyle`, `TextStyle`, the `*Patch`
  types, capability-limited `.style` surfaces) and the shared
  resolver/equality/damage rules are defined in
  [STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md); the runtime
  responsibility is only that resolved-value equality, not input identity,
  decides downstream work.

This is keyed structural reconciliation over one retained arena — not a
browser DOM and not a second virtual view tree. Node storage and message
factories are type-erased internally, which is fine because the public
surface keeps `M` typed and no unsafe downcasting escapes to consumers. Closures
at call sites may box and rebind on every `view` pass; the model does not
promise zero allocation or fixed traversal cost.

## Identity and keys

```rust
pub trait KeyId: Eq + std::hash::Hash + Clone + 'static {}
impl<T: Eq + std::hash::Hash + Clone + 'static> KeyId for T {}
```

- Every node has a stable `NodeId` carrying a generation; identity is
  (parent scope, key type namespace, key equality) — a hash collision between
  unequal keys is not identity.
- Static siblings that emit no explicit key get ordinal+kind identity within
  their parent.
- Keys are parent-local. Moving a key to another parent remounts the node;
  reusing a key after deletion produces a new generation, so nothing stale
  can claim the slot.
- Emitting the same key twice under one parent is a duplicate-key diagnostic
  error in all builds — it is never silently resolved.
- Nodes not re-emitted in a pass are removed at commit: focus, capture,
  motion, native peers and queued messages for them are released; the app's
  own data is untouched.
- Queued and native-originated messages carry their emitting generation; late
  events from removed nodes are dropped. Async replies carry a separate task
  generation (see [STATE_AND_EVENTS.md](STATE_AND_EVENTS.md)).

Rules of thumb for authors: always wrap a conditional region in a stable
`ui.group(key, ...)`; use `ui.keyed(items, key_fn, ...)` for lists keyed by a
stable business key, never the index; use `ui.scope` when a child emits a
different message type. `ui.keyed` itself owns an implicit list scope at a
stable static ordinal — its item keys are local to that scope — so a list
whose location varies or is conditional still belongs inside an explicit
`ui.group`.

## Lifetimes and the view contract

- `view(&S, &mut Ui<M>)` may borrow state freely during traversal; retained
  changed text/assets are copied or shared into the arena before `view`
  returns, so no `&self` data can be captured by retained nodes.
- Event factories are `Fn() -> M + 'static` (or `Fn(E) -> M + 'static` for
  data events) — they manufacture messages on demand, so `M` never needs to
  be prebuilt or `Clone`. They never capture borrowed `&self`, never mutate
  shared state, and never produce side effects during `view`. Copy item IDs
  into `move` factories rather than capturing references.
- Event factory snapshots bind at dispatch time. Dispatch, `update` and
  `view` are nonreentrant: a message cannot re-enter `view`, and `view` cannot
  emit messages synchronously.
- Mutating `S` only inside `update` is a programming contract, not a type
  proof — interior mutability escapes exist and are the consumer's
  responsibility.

## Invalidation pipeline

```text
native event -> dispatch -> update batch (1..n messages)
             -> root view rerun -> staged transaction
             -> commit -> native notifications -> narrow damage
```

- Worst case, the root `view` traverses all N nodes after a batch; app
  computation is deliberately not signal-granular.
- Damage is narrow: unchanged props skip layout/paint/accessibility; only
  changed regions schedule work.
- Static widget-level changes that need no app message — focus ring, hover
  colour, caret blink, in-flight motion — are owned by the runtime and bypass
  `view` entirely.
- The model targets small product UI; it does not pretend to be the right
  tradeoff for enormous virtualized lists.

## Composition

Reusable UI is ordinary code, not a component system:

- pure draw functions `fn draw_panel(ui: &mut Ui<ChildMsg>, props: ...)`, or
- plain child structs with their own `update`/`view`, whose state the parent
  owns and whose `ChildMsg` the parent forwards explicitly via `ui.scope`.

There are no invisible component slots, no automatic memoization of absent
components, and no hidden caches of dropped children.
