# Backend Architecture

Status: architecture concept only. No implementation crates or platform
source files are
created by this task; every signature below is a proposed internal contract.

## Packaging recommendation

One eventual public library crate — `rust-ui` — with private modules:

```text
rust-ui
  runtime/     # App, UpdateCtx, Ui, arena, transaction, dispatch, tasks
  layout/      # containers, Length, measure/distribute/clip
  theme/       # Theme, ColorRole, Space, Radius, MotionToken, recipes
  widgets/     # label, button, icon_button, badge, separator, text peers, image, custom
  icons/       # generated Lucide Icon enum + path data (offline codegen)
  platform/
    win32/
    macos/
```

A `gallery` example lives behind it later. Six premature crates buy nothing:
the only consumers are rust-ui itself and, eventually, Mascot. Splitting
backends into their own crates is justified only by an independent
release/testing need, and must never create cycles or force public
OS-handle traits into the API.

Platform dependencies are optional and isolated behind `#[cfg]` imports.
Core, generic tests — model, tree, layout, theme, message flow — must compile
and run headless on Linux even though a Linux backend is deferred.

## Internal backend contract surface

Private traits (never public, never exposing raw OS handles):

```rust
trait EventLoopBackend {
    fn run(&self, handler: &mut dyn FnMut(LoopEvent));
    fn set_deadline(&self, at: Option<Instant>);
    fn wake_handle(&self) -> WakeHandle;
}

#[derive(Clone)]
struct WakeHandle {
    _private: (),
}
impl WakeHandle {
    fn wake(&self);
}

trait WindowBackend {
    fn set_title(&self, title: &str);
    fn size(&self) -> Size;
    fn scale_factor(&self) -> f32;
    fn set_theme_mode(&self, mode: ThemeMode);
    fn appearance(&self) -> Appearance;
    fn invalidate(&self, damage: Rect);
    fn present(&self, canvas: &mut dyn CanvasBackend) -> Result<(), PlatformError>;
}

trait CanvasBackend {
    fn path(&mut self, path: &Path2d, paint: Paint);
    fn image(&mut self, image: &ImageSource, rect: Rect);
    fn text(&mut self, run: &TextRun, origin: Point);
}

trait TextMeasureBackend {
    fn shape_label(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
    ) -> Result<TextMetrics, PlatformError>;
    fn invalidate_measure_cache(&mut self, reason: MeasureCacheInvalidate);
}

struct TextPeerUpdate {
    bounds: Rect,
    clip: Rect,
    scale: f32,
    theme: TextPeerTheme,
    read_only: bool,
    disabled: bool,
    visible: bool,
    max_lines: Option<u32>,
    submit_policy: SubmitPolicy,
    pending_patch: Option<TextPatch>,
}

trait TextPeerFactory {
    fn mount(
        &self,
        window: &dyn WindowBackend,
        update: TextPeerUpdate,
    ) -> Result<Box<dyn TextPeer>, PlatformError>;
}

trait TextPeer {
    fn measure(&self, constraints: Constraints) -> Result<TextMetrics, PlatformError>;
    fn apply(&mut self, update: TextPeerUpdate) -> Result<(), PlatformError>;
    fn drain_events(&mut self, out: &mut Vec<TextPeerEvent>);
    fn accessibility_handle(&self) -> A11yHandle;
    fn unmount(self: Box<Self>);
}

trait ClipboardBackend {
    fn get_text(&self) -> Option<String>;
    fn set_text(&self, text: &str);
}

trait FocusBackend {
    fn focused(&self) -> Option<NodeId>;
    fn request(&self, node: NodeId);
}

trait AccessibilityBackend {
    fn sync_subtree(&self, root: NodeId, tree: &SemanticsTree);
}
```

Roles:

- `EventLoopBackend` methods are UI-thread-only — `run` is the blocking
  platform loop, `set_deadline` drives the single earliest-deadline timer.
  Worker-side waking goes through the cloneable `Send + Sync` `WakeHandle`;
  the loop object itself never crosses threads.
- `WindowBackend` is one native window: title, size, scale factor, theme
  mode, current `Appearance`, damage invalidation and `present` — which
  returns a typed `PlatformError` on failure rather than dropping a frame
  silently.
- `CanvasBackend` is the paint target backing the small `Canvas` op set with
  dp snapping at this boundary. Static-label shaping and measurement are a
  separate `TextMeasureBackend` (or a `CanvasBackend` method) whose cache is
  invalidated on DPI, font or theme change — measured text is not implicitly
  stable across those.
- `TextPeer` is the editable native peer, created through `TextPeerFactory`
  and addressed through opaque internal handles — no raw native objects
  cross the boundary or a thread. `apply` pushes a whole `TextPeerUpdate`
  (bounds, clip, scale, theme/text style, read_only, disabled, visible,
  max_lines, submit_policy, pending text patch) — acknowledgement-aware:
  property and acknowledgement updates never reset the peer's undo stack,
  while an explicitly accepted programmatic replacement may, per the native
  contract. `set_bounds`-equivalent moves also reposition caret and
  IME candidate UI. `measure` feeds layout min/max lines.
  `drain_events` runs only after commit/native notifications — never periodic
  polling — and yields edits, selection, submit and conflict events.
  `unmount` retains the last acknowledged committed snapshot and fences any
  teardown callbacks — it does not promise to save in-progress composition.
  Mount, `apply` and `measure` return typed errors: a failed peer is
  reported, not an invisible missing text area.
- `ClipboardBackend` and `FocusBackend` are minimal;
  `AccessibilityBackend` syncs the semantics tree and imports the native
  text peers' own accessibility subtrees via `A11yHandle`.

Supporting shapes — `PlatformError`, `TextMetrics`, `TextPatch`,
`TextPeerTheme`, `TextStyle`, `MeasureCacheInvalidate`, `A11yHandle`,
`SemanticsTree`, `LoopEvent`, `TextPeerEvent` — are internal typed
structs/enums local to the platform modules; none exposes a raw OS handle.
`TextPatch` distinguishes its variants: initialization (the current committed
value, revision and binding), a native-acknowledgement marker, and a
programmatic proposal with its base revision — so `mount` always receives the
real committed value rather than accidentally attaching an empty peer, and
`apply` carries the same patch on proposal state changes. There is no second
editor state object.

## Platform adapters — conceptual flow

### Windows (`platform/win32`)

- Owning UI thread runs a COM-initialised Win32 message loop; `EventLoopBackend`
  posts a thread message for `wake` and a timer for `set_deadline`. No busy
  poll.
- Baseline raster path: Direct2D + DirectWrite in a software-compatible
  configuration is the proposed spike target — the supplied Mascot evidence
  establishes Direct2D/DirectWrite/DirectComposition generally, not the
  software path's measured properties. No permanently initialised GPU stack
  is required for a simple opaque window.
- Acceleration and DirectComposition for layered/transparent surfaces are
  optional and lazy, enabled only where the window actually needs them.
- Text peers are windowless RichEdit hosts (`ITextHost`/`ITextServices`,
  `TxDrawD2D` target where available) per [NATIVE_CONTROL_BOUNDARY.md](NATIVE_CONTROL_BOUNDARY.md).
- Flow: `msg loop -> dispatch -> update batch -> view -> commit ->
  invalidate(damage) -> present`. Native peer bounds resolve at commit, before
  notifications dispatch.

### macOS (`platform/macos`)

- Main thread runs `NSApplication`'s loop; `wake` posts to the main queue and
  `set_deadline` drives a `CFRunLoopTimer`/`NSTimer`. No busy poll.
- Baseline raster path: AppKit + Quartz/CoreText drawing; `CALayer`
  acceleration is optional.
- Text peers are `NSTextView` instances with first-responder semantics.
- Flow matches Windows: platform event -> dispatch -> update -> view ->
  commit -> invalidate -> present; AppKit peers follow the same rectangular
  island rules.

## Failure and lifecycle contracts

- Unsupported capabilities (e.g. transparent composition on a platform build
  that cannot provide it) are reported as typed `UiError` variants — never
  silent degradation. No all-native compositing guarantee is made until the
  spike proves it.
- GPU/render-target failure must not take down the app model: recreate device
  resources, mark full damage once, and preserve native text undo/selection
  wherever the peer survives.
- Window destruction is ordered: fence queued tasks and pending node events
  -> focus/composition teardown -> accessibility peers -> text peers ->
  surfaces/paint targets -> model drop.
- `M` deliveries, task generations and node generations are all fenced before
  teardown begins, so no after-free callback can reach the app.

## What this deliberately does not include

- No port of Mascot's painter, window or text code — backend traits are
  designed from the contracts above, not transplanted.
- No public OS-handle API; no consumer ever sees `HWND`, `NSView`, COM
  interfaces or GPU objects.
- No Linux backend in v0.1; the headless-test requirement exists so the core
  never becomes accidentally platform-coupled.
