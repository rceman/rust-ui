# Native Control Boundary

Status: architecture concept only. All signatures are proposed API exercises;
no platform implementation exists yet. Native peer choices below are design
targets justified by geometry and behaviour requirements — they are not a
port of Mascot code.

## Text contract

The app owns committed text; the native peer owns everything else.

```rust
#[derive(Default)]
pub struct TextValue {
    _private: (),
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct TextRevision(u64);

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct BindingToken(u64);

pub enum EditOrigin {
    NativePeer,
    Programmatic,
}

pub struct TextEdit {
    _private: (),
}
impl TextEdit {
    pub fn text(&self) -> &str;
    pub fn base_revision(&self) -> TextRevision;
    pub fn result_revision(&self) -> TextRevision;
    pub fn origin(&self) -> EditOrigin;
    pub fn binding(&self) -> BindingToken;
}

pub struct TextConflict {
    pub rejected_revision: TextRevision,
    pub rejected_text: String,
    pub committed_revision: TextRevision,
    pub committed_text: String,
    pub binding: BindingToken,
}

pub enum AcceptOutcome {
    Applied,
    IgnoredStale,
    IgnoredOrigin,
}

impl TextValue {
    pub fn new(text: impl Into<String>) -> Self;
    pub fn text(&self) -> &str;
    pub fn revision(&self) -> TextRevision;
    pub fn is_empty(&self) -> bool;
    pub fn accept(&mut self, edit: TextEdit) -> AcceptOutcome;
    pub fn replace(&mut self, text: impl Into<String>) -> TextRevision;
    pub fn clear(&mut self) -> TextRevision;
    pub fn keep_native(&mut self, conflict: TextConflict) -> AcceptOutcome;
}
```

`TextValue` holds the last acknowledged committed UTF-8 text, its revision, a
binding identity, and at most one pending programmatic replacement proposal
`(base_revision, requested_revision, requested_text)`:

- `replace`/`clear` queue or overwrite that proposal and return the requested
  revision — `text()` keeps reporting the last acknowledged committed text
  until the peer acknowledges the proposal. `TextValue::new` sets initial
  content; `Default` is an empty value. A pending proposal on an unmounted
  editor waits for the next mount.
- One `TextValue` binds to at most one mounted editable peer; binding it to
  two is an explicit error. There is no arbitrary `Clone` and no unsafe
  widget sharing.
- `accept` acknowledges the peer's newest in-order committed snapshot —
  whether `EditOrigin::NativePeer` or a `Programmatic` acknowledgement of a
  requested replace. Older or foreign-binding edits are ignored
  diagnostically (reported without text contents, never applied);
  acknowledged text is never resent to the peer, so acceptance emits no
  feedback loop. Accepting a `Programmatic` acknowledgement clears only the
  matching pending proposal — no text-reset echo.
- Native edits keep updating the committed snapshot while a programmatic
  proposal is pending, right up until the proposal is applied or conflicts.
- `keep_native(conflict)` reconciles against the conflict's committed
  snapshot (rejecting a stale or foreign snapshot), discards only the
  matching losing proposal, and tombstones the request so `view` does not
  resend it. A conflict from an older generation cannot clear a newer queued
  intent.

Origin tokens are opaque and identify the binding plus peer generation — they
carry more than the public `EditOrigin` enum.

### What the native peer owns

Selection, marked/composition text, undo stack, caret, internal scroll —
never mirrored into app state. Ordinary composition is never converted into a
destructive set-text round trip, and the core never overwrites marked text.
`TextValue::text()` exposes committed text only; in-progress composition is
not exposed as if it were committed. Native edit and conflict notifications
arrive ordered and nonreentrant.

### Programmatic edits versus composition

- A `replace`/`clear` during active composition is queued with its original
  base revision and applied only when composition ends and the base is still
  current.
- If the base has diverged by then, the peer emits `TextConflict` — carrying
  the rejected requested revision and text plus the latest committed native
  snapshot, revision and opaque binding — surfaced through `.on_conflict`.
  Nothing is silently overwritten, there is no automatic force-replace, and
  `update` never re-applies a failed intent on its own: the owner decides
  (`keep_native` plus a notice, or a deliberate retry).
- `.on_conflict` is mandatory whenever an app issues programmatic
  replacements: an unhandled rejected proposal produces a structured
  diagnostic and is suppressed — never retried in a loop.
- Unmounting a peer runs the native cancellation policy — which may finish a
  pending commit on some platforms. v0.1 removal semantics: the app retains
  its last *acknowledged* committed snapshot; any native final callback
  arriving during destruction is fenced and dropped — a removed app child is
  never mutated after removal. Loss of uncommitted in-progress composition
  on unmount is documented and tested, not claimed lossless.
- `Visibility::Hidden` keeps the peer and model alive; a completed native
  commit is routed before focus relocation where the native input policy
  allows it.
- Programmatic replace may reset the peer's undo stack; native
  acknowledgements and peer rebuilds must not.

### Submit policy

```rust
pub enum SubmitPolicy {
    Enter,
    ModifierEnter,
    None,
}
```

Submit fires only after native key interpretation confirms no active
composition (Enter consumed by an IME candidate window is not submit).
`ModifierEnter` is Ctrl+Enter on Windows and Command+Enter on macOS. On a
single-line input, plain Enter submits and produces no newline even with
Shift; on a `TextArea` under `Enter`, Shift+Enter inserts the newline.
`None` disables the chord. There is no custom editing, undo or spelling
engine in scope.

### Selection

`TextSelection` uses UTF-8 byte offsets plus the `TextRevision` it was
observed at, with separate `anchor`/`focus`. During active composition a
native selection may point into the peer's temporary marked-text buffer —
offsets that cannot be validated against committed text — so v0.1 emits
selection-changed only for committed-revision snapshots and suppresses
interim marked-selection notifications; IME itself stays fully native.
Platform adapters convert and validate native UTF-16 indices with checked
boundary math — byte offsets, UTF-16 code units and grapheme clusters are
never conflated. The conformance suite must cover emoji, combining
characters, RTL and IME paths. Selection is observable by the owner; there is
no global mirrored undo.

### Builder surface

`TextInput` and `TextArea` share the peer contract; `TextArea` adds multiline:

```rust
ui.text_input(&self.name)
    .label("Name")
    .placeholder("Ada Lovelace")
    .on_edit(Msg::Edited)
    .on_submit(|| Msg::Submit)
    .on_conflict(Msg::Conflict)
    .read_only(false)
    .disabled(false);

ui.text_area(&self.draft)
    .max_lines(6)
    .submit_policy(SubmitPolicy::Enter);
```

`.read_only(true)` blocks editing but keeps selection/focus; `.disabled(true)`
blocks activation entirely. They are independent.

## Geometry and layering

- Native peers occupy rectangular, axis-aligned bounds keyed to a stable
  `NodeId` + generation. There are no arbitrary affine transforms, opacity or
  rounded masks on live text: animate the chrome around a text peer, never
  the peer's contents.
- Rectangular clip only; dp→physical conversion happens at the backend.
  Min/max line measurement, keyboard focus and IME candidate-window positions
  follow layout.
- Portable layer rule: painted content may not arbitrarily overlap native
  controls. Tooltips and future popups live in top-level platform overlay
  surfaces where clip, hit-test and accessibility agree — there are no
  fake z-index promises.
- Focus loss or node removal moves focus to the next eligible node or back to
  the invoker — never to a destroyed peer.
- Accessibility imports the native text subtree (real caret, selection,
  editing announcements); it never receives a duplicate painted editable
  label.

## Peer styling

Native text peers accept one capability-limited patch type,
`TextInputStylePatch` — a chrome `BoxStylePatch` (background/border/radii/
padding of the painted frame) plus an optional `foreground: Color` routed to
the peer through a typed adapter:

- chrome maps only to the painted frame; `foreground` is subject to normal
  OS adjustment, and v0.1 requires peer backing and editable foreground to
  resolve fully opaque;
- the rectangular peer sits inside a mechanical safety inset — border +
  padding + the paired authored corner radius per side, exact rule in
  [STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md) — never
  rounded-clipped or alpha-composited; too-small constraints are
  `InvalidLayout`, nonopaque backing/foreground `Unsupported`;
- selection/caret/IME colors and fonts stay primarily OS-owned — no public
  `TextStyle` knobs on editable peers, system fonts and text scaling remain;
- theme/style/geometry updates never remount the peer, `set_text`, or reset
  undo/selection/composition — resolved-value equality skips redundant
  `apply`, and a style update during active composition is never converted
  into a text replace. Backends validate adapter capability before any
  native commit.

The full patch vocabulary and merge/equality rules live in
[STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md).

## Platform peers

### Windows — proposed: windowless RichEdit

Target the documented windowless RichEdit host: `ITextHost` supplies window
services, `ITextServices` renders/measures, with `ITextServices2::TxDrawD2D`
(Windows 8+ documented API) for composition into the Direct2D target.
Windowless is selected because *this* design composes text and painted layers
in one target — it is not the claim that transparency requires windowless;
an architecture using windowed controls could meet different constraints.
This is a new implementation that requires approval and spike validation
before it ships.

The host must supply input routing, caret/timer services, invalidation, and
UIA accessibility embedding — "native" does not mean accessibility is solved
for free. Two declared spike risks: whether a suitable UIA provider ships
with the RichEdit build we target, and the exact platform version coverage of
`TxDrawD2D`. Generic ActiveX host guidance is not proof either exists.

HWND RichEdit is the recorded alternative; it is not a silent fallback
because it changes the layering/transparent-surface capabilities the
requirement depends on.

### macOS — proposed: NSTextView

Plain `NSTextView` inside `NSScrollView` where scrolling is needed, using the
AppKit text system, first-responder semantics and `NSAccessibility` — never a
hand-rolled `NSTextInputClient` implementation. Main-thread lifetime, native
marked text.

The shared rectangular/axis-aligned geometry limits apply on macOS too, even
though the Windows windowless path could technically do more. A transparent
borderless window hosting a native text view alongside painted content is a
spike requirement — it must be demonstrated before Mascot-style surfaces are
declared eligible.

## Custom rendering extension

```rust
pub trait CustomRender {
    fn measure(&self, available: Constraints) -> Size;
    fn paint(&self, canvas: &mut dyn Canvas, bounds: Rect);
    fn hit_test(&self, local: Point, bounds: Rect) -> bool;
    fn semantics(&self) -> Semantics;
}

pub struct Semantics {
    pub role: Role,
    pub label: String,
    pub actions: Vec<SemanticsAction>,
}

pub trait Canvas {
    fn path(&mut self, path: &Path2d, paint: Paint);
    fn image(&mut self, image: &ImageSource, rect: Rect);
    fn text(&mut self, run: &TextRun, origin: Point);
}
```

`Semantics` carries a typed role (e.g. `Role::Button`), an accessible name
and a list of actions. `Canvas` is a deliberately small platform-neutral op
set — path, image, text — exposing no raw OS or GPU handles; a CPU raster
backend can implement all of it. `Paint::fill_role(ColorRole)` resolves the
role through the canvas's current theme, so a `ui.theme` change repaints a
custom node correctly. `Constraints::constrain(Size) -> Size` clamps a
desired size to the available box — `measure` must return a size within it.

Contract:

- The retained node holds an immutable `Rc<dyn CustomRender>` snapshot; no
  borrowed view data is retained, and UI-only `!Send` snapshots are fine.
- `ui.custom(self.frame.clone()).frame_events(self.playing).on_frame(Msg::Frame)`
  — frame events are typed `FrameTime`; the app's `update` advances the
  animation, installs a fresh snapshot, and clears `playing` when done.
- The scheduler only runs while the node is mounted, visible, and has
  `frame_events` enabled; node generation rejects late callbacks. There is no
  permanent default loop — the product's rig engine stays outside rust-ui.
- `frame_events` is decorative frame demand, also gated by the effective
  reduced-motion policy: under `Reduce` no frame events are delivered and the
  node retains its static snapshot. Resuming after hidden or reduced-motion
  suspension resets the `FrameTime` delta clock; suspended time is not
  delivered as a giant first delta. Product async progress/text updates are
  unaffected; essential real-time media clocks are outside v0.1.
- Paint callbacks must not block, mutate app state, or request uncontrolled
  loops.
- `semantics()` returns typed role/label/actions — never a fake native text
  tree. Custom actionable content wires the same `.on_press` keyboard and
  accessibility activation as a real button.
- `ImageSource::decode_png(bytes)` decodes once — before `App::run` or on a
  worker — never inside `view`; failures surface as `AssetError` via the
  `UiResult` conversion.
- Layout rule: a native `TextArea` sits beside custom imagery in its own
  rectangle, not alpha-composited over it.
