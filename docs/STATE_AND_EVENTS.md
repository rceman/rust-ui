# State and Events

Status: architecture concept only. All signatures are proposed API exercises.

## State ownership tiers

| Tier | Owner | Lifetime | Example |
|---|---|---|---|
| Application state | the app's `S` struct | app lifetime | counter value, draft `TextValue`, theme choice |
| Child component state | parent struct, via ordinary fields | parent's own lifetime | child `Composer` struct inside `Chat` |
| Ephemeral interaction state | runtime node, not app | while node mounted | hover, focus ring, press, caret, selection, scroll, tooltip visibility |
| Derived state | computed in `view`/helpers | recomputed, never stored | `!self.draft.text().is_empty()` as `can_send` |

Derived state is a pure computation in `view`; there are no signals or cached
computations inside the framework.

Preservation and removal rules:

- Conditional disappearance drops the runtime node — its focus, pointer
  capture, motion, native peer and pending node events — but never app-owned
  data the parent still holds.
- Explicit removal of an item from a `Vec`/map drops that user state; tasks
  the app owns require explicit `cx.cancel` (tasks are not garbage-collected
  with nodes by implication — see "Task ownership").
- Reordering keyed nodes preserves native peers, undo state and focus.
- There is no automatic memory cache of absent components; a remounted
  subtree starts fresh unless the app kept its state.

The mutation contract: `S` changes only inside `update`. This is a programming
contract — `RefCell` can defeat it — not a type-level guarantee.

## Events

All events are typed factories bound at the emitting node. Ownership means:
the closure lives on the node, produces `M` (or `Option<M>`), and the runtime
routes the message into `update` after traversal.

| API | Payload | Produced by | Semantics / example |
|---|---|---|---|
| `.on_press(Fn() -> M)` | none | semantic activation: pointer release, Enter/Space on the focused control, accessibility Invoke | `ui.button("Send").on_press(|| Msg::Send)` |
| `.on_pointer_enter/leave(Fn(PointerEvent) -> M)` | `PointerEvent` | hit-test boundary transitions | raw chrome effects only; prefer `.on_press` |
| `.on_pointer_down/up(Fn(PointerEvent) -> M)` | `PointerEvent` | raw pointer phases | custom surfaces, drag starts |
| `.on_focus(Fn() -> M)` / `.on_blur(Fn() -> M)` | none | focus gained/lost | `.on_focus(|| Msg::EditorFocused)` |
| `.on_key(Fn(KeyEvent) -> Option<M>)` | `KeyEvent` | unhandled non-text keys after native peer | `None` leaves native handling intact |
| `.on_edit(Fn(TextEdit) -> M)` | `TextEdit` | native peer committed a text change | `.on_edit(Msg::Edited)` |
| `.on_submit(Fn() -> M)` | none | submit chord per `SubmitPolicy`, only when no composition is active | `.on_submit(|| Msg::Submit)` |
| `.on_selection_changed(Fn(TextSelection) -> M)` | `TextSelection` | native peer selection change | `.on_selection_changed(Msg::Sel)` |
| `.on_conflict(Fn(TextConflict) -> M)` | `TextConflict` | queued programmatic replace lost its base | `.on_conflict(Msg::Conflict)` |
| `.on_open_change(Fn(bool) -> M)` (future) | `bool` | Popover/Dialog open state | future controls only |
| `.on_frame(Fn(FrameTime) -> M)` | `FrameTime` | scheduled frame while `frame_events` on | `.on_frame(Msg::Frame)` |
| `.on_scroll(Fn(ScrollOffset) -> M)` (future) | `ScrollOffset` | ScrollArea position | future controls only |
| `.on_change(Fn(T) -> M)` (future) | typed value — `bool`, enum, etc. | Checkbox/Switch/RadioGroup/Select value change | future controls only; does not exist in v0.1 |
| custom semantic events | `M` variants | app-defined message variants | `Msg::RowToggled(row_id)` |

Event payload types:

```rust
pub struct PointerEvent {
    pub position: Point,
    pub button: Option<PointerButton>,
    pub modifiers: Modifiers,
}
pub struct KeyEvent { pub key: Key, pub modifiers: Modifiers }
pub struct FrameTime { pub delta: Duration, pub absolute: Instant }
pub struct TextSelection { pub revision: TextRevision, pub anchor: usize, pub focus: usize }
pub struct TextConflict {
    pub rejected_revision: TextRevision,
    pub rejected_text: String,
    pub committed_revision: TextRevision,
    pub committed_text: String,
    pub binding: BindingToken,
}
pub struct Modifiers { pub shift: bool, pub ctrl: bool, pub alt: bool, pub logo: bool }
pub struct ScrollOffset { pub x: f32, pub y: f32 }
```

`PointerEvent.button` is `None` for enter/leave — hover carries no button —
and `Some(..)` on down/up.

Routing order — there is no DOM bubbling/capture machinery:

1. hit-test target (or the capture owner while a pointer is captured);
2. the native text peer first for text/IME input;
3. the typed scope adapter upward (`ui.scope` map functions) to reach the
   app's `update`;
4. explicit window-level shortcut fallback last.

Focus traversal and activation are distinct: Tab moves focus without
activating, while Enter/Space on the focused control and the platform
accessibility Invoke action produce `.on_press` — a button that activates
under a pointer must activate under the keyboard and the screen reader
identically. Disabled controls cannot activate
and produce no messages. `.on_key` only sees unhandled non-text keys; global
shortcuts never steal active text composition. Tooltips emit no message by
default — hover/focus is runtime chrome, not app state.

Future controls inherit this table rather than bypassing it: Dialog/Popover
add focus scoping, Escape/dismissal, focus return to the invoker, a semantic
role and a top-level platform overlay; ScrollArea adds clipping/damage and an
offset that native peers must follow. The generic state/message/key model
supports all of that — but implementing it is real backend work, not free.

## Async

Proposed core contracts (no Tokio import; the executor is supplied):

```rust
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;

pub trait Executor: Send + Sync {
    fn spawn(&self, task: BoxFuture<()>) -> Result<(), TaskStartError>;
}

#[derive(Clone)]
pub struct CancelToken {
    _private: (),
}
impl CancelToken {
    pub fn is_cancelled(&self) -> bool;
    pub async fn cancelled(&self);
}

pub struct TaskSender<M>;
impl<M: Send + 'static> TaskSender<M> {
    pub async fn send(&self, msg: M) -> Result<(), SendError>;
    pub fn map<N: Send + 'static>(
        &self,
        f: impl Fn(N) -> M + Send + Sync + 'static,
    ) -> TaskSender<N>;
    pub fn is_closed(&self) -> bool;
}

pub struct Job<M>;
impl<M: Send + 'static> Job<M> {
    pub fn cancellation(&self) -> CancelToken;
    pub fn sender(&self) -> &TaskSender<M>;
    pub async fn send(&self, msg: M) -> Result<(), SendError>;
}

impl<'cx, M: 'static> UpdateCtx<'cx, M> {
    pub fn spawn<K, F, Fut>(&mut self, key: K, factory: F) -> Result<(), TaskStartError>
    where
        K: KeyId,
        F: FnOnce(Job<M>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
        M: Send;

    pub fn cancel(&mut self, key: impl KeyId);

    pub fn scope<ChildMsg: 'static>(
        &mut self,
        key: impl KeyId,
        adapter: impl Fn(ChildMsg) -> M + 'static,
    ) -> UpdateCtx<'_, ChildMsg>;
}
```

`CancelToken::cancelled` resolves when cancellation is fenced.
`TaskSender::send` is bounded and never blocks the UI thread. The factory
returns any `Send` future — the core boxes it once for the `Executor`, so no
`Box::pin` is needed at the call site.

`UpdateCtx::scope` is the update-side counterpart of `ui.scope`: it returns a
child-scoped context whose spawns and sends arrive at the parent's `update`
already wrapped by the adapter. The same stable key used in `ui.scope` and
`cx.scope` names one namespace: task identity is (window, explicit update
scope path, key type/equality, generation), so two sibling composers each on
`Task::Reply` do not cancel each other. Task `KeyId` values stay
UI-thread-side — worker envelopes carry opaque generation tokens, not `K`.
`UpdateCtx` adapters live on the UI thread and need not be `Send` — only the
`ChildMsg: Send` values crossing to a worker must be `Send`. No context
borrow is retained by a spawned task, and neither the factory nor the adapter
map re-enters `update`.

`TaskStartError`, `SendError` and `ProxySendError` are typed errors, not
strings.

Semantics:

- `cx.spawn(key, factory)` registers the key's task generation and hands the
  boxed future to the configured `Executor`. The factory is `FnOnce`, `Send`,
  `'static`, and captures `Arc` services and owned values — never borrows.
- `cx.cancel(key)` synchronously fences delivery by incrementing the scoped
  generation; the `CancelToken` cooperatively wakes the adapter/future and the
  wrapper stops polling and drops the future when possible. Cancel does not
  undo external side effects and cannot kill a blocking syscall.
- Re-spawning under the same key fences the previous generation. Generation
  checks happen at send and at dequeue, so a stale chunk or completion can
  never contaminate the replacement task's stream.
- `TaskSender::send` returns `Err` on a closed/cancelled channel; the mailbox
  is bounded, applies backpressure, and never blocks the UI thread. Stream
  order is preserved and chunks are never lossy-coalesced — batching applies
  to paint wakeups and state updates, not to data.
- The task queue's bound is an event-count bound; payload byte bounds are the
  product's responsibility (no generic `M` can enforce a byte limit).
- A failed or closed send exits the service loop — no spin, no polling.

Consumer-side contract used in examples (explicitly a consumer service, never
part of rust-ui core):

```rust
pub trait ReplyService: Send + Sync {
    fn reply(
        &self,
        prompt: String,
        cancel: CancelToken,
        chunks: TaskSender<String>,
    ) -> BoxFuture<Result<(), String>>;
}
```

Spawn usage, verbatim shape:

```rust
cx.spawn(Task::Reply, move |job| async move {
    let result = service
        .reply(prompt, job.cancellation(), job.sender().map(Msg::Chunk))
        .await;
    let _ = job.send(Msg::Finished(result)).await;
})
```

`job.sender().map(Msg::Chunk)` stores the mapping factory in the sender, so
each `String` chunk becomes a `Msg` at send time; this path requires
`Msg: Send`.

### External worker bridge

For a `std::thread` worker that cannot return a future, the bridge is
`UiProxy<M>` obtained via `App::proxy()` before `run()`:

```rust
impl<M: Send + 'static> UiProxy<M> {
    pub fn try_send(&self, msg: M) -> Result<(), ProxySendError>;
}
```

This is the same bounded mailbox feeding `update` — one delivery path, not a
second task API.

### Task ownership and lifecycle

- Tasks belong to the app/window scope, never to a transient button node — a
  control that toggles Send/Stop must not cancel itself by unmounting, and
  the runtime never infers task ownership from view visibility.
- Removing a stateful child does **not** auto-cancel its tasks: the owner
  explicitly opens the same scope (`cx.scope`) and calls `cx.cancel(key)`
  before dropping the child's state.
- Window close cancels every task scope and releases native resources in the
  destruction order defined in [BACKEND_ARCHITECTURE.md](BACKEND_ARCHITECTURE.md).
- Busy state is app data: set true only when `spawn` returns `Ok`, cleared on
  `Finished`, on cancellation, or on a launch error — a failed launch leaves
  the app not-busy. Stop pressed → `cx.cancel`, busy=false, partial output
  preserved; a new Send clears output.
- Effects testing (recording `UpdateCtx` against a virtual clock) is named as
  a future validation seam, not a shipped harness.

Lifecycle: normal future completion closes further sends but retains the
job's delivery registration until all accepted envelopes, including its
terminal message, have been dispatched. Completion alone must not fence valid
queued output. Cancellation, replacement or window close instead fences the
old generation immediately and may discard its queued envelopes. Registry
entries and adapters are reclaimed after the corresponding drain or fence;
closed mapping senders cannot retain the window or model. Unique task
IDs/generations avoid an ever-growing map of retired keys. Framework live
state stays proportional to mounted nodes + active jobs + bounded queued
messages + explicitly budgeted caches, not the count of keys ever used;
bounding the product's own data remains the consumer's job.
