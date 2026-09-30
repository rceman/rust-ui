# API Examples

Status: architecture concept only. Every block below is a proposed API
exercise — consumer-shaped Rust, not implemented or compiler-verified.
Fragments assume `use rust_ui::prelude::*;` plus the std imports shown;
nothing here is independently runnable because the crate does not exist yet.
Examples 5/6 and 10 build on each other as named; each exercise still shows
concrete consumer code.

## 1. Counter

The canonical minimal app: plain state, one message type, `update`/`view`,
`App::new(...).title(...).run()`.

```rust
use rust_ui::prelude::*;

#[derive(Default)]
struct Counter {
    value: i32,
}

enum Msg {
    Increment,
}

impl Counter {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::Increment => self.value += 1,
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Sm), |ui| {
            ui.label(self.value.to_string());
            ui.button("Increment").on_press(|| Msg::Increment);
        });
    }
}

fn main() -> UiResult {
    App::new(Counter::default(), Counter::update, Counter::view)
        .title("Counter")
        .run()
}
```

## 2. Form / text input

Committed text lives in `TextValue` fields; the native peer owns IME,
selection and undo. `accept` acknowledges in-order edits; a rejected
programmatic replace arrives as `Msg::*Conflict`, and the owner resolves it
with `keep_native` plus a user-visible notice — it never retries the failed
intent silently. Submit is also validated in `update`, not only by the
disabled button.

```rust
use rust_ui::prelude::*;

#[derive(Default)]
struct Form {
    name: TextValue,
    notes: TextValue,
    saved: Option<String>,
    notice: Option<String>,
}

enum Msg {
    NameEdited(TextEdit),
    NotesEdited(TextEdit),
    NameConflict(TextConflict),
    NotesConflict(TextConflict),
    Submit,
}

impl Form {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::NameEdited(edit) => {
                self.name.accept(edit);
            }
            Msg::NotesEdited(edit) => {
                self.notes.accept(edit);
            }
            Msg::NameConflict(conflict) => {
                self.name.keep_native(conflict);
                self.notice =
                    Some("Text changed during editing; review before clearing.".into());
            }
            Msg::NotesConflict(conflict) => {
                self.notes.keep_native(conflict);
                self.notice =
                    Some("Text changed during editing; review before clearing.".into());
            }
            Msg::Submit => {
                if self.name.text().trim().is_empty() {
                    self.notice = Some("Name is required.".into());
                    return;
                }
                self.saved = Some(self.name.text().to_string());
                self.notes.clear();
                self.notice = None;
            }
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.surface(Surface::new().padding(Space::Md), |ui| {
            ui.column(Column::new().gap(Space::Md), |ui| {
                ui.text_input(&self.name)
                    .label("Name")
                    .placeholder("Ada Lovelace")
                    .on_edit(Msg::NameEdited)
                    .on_submit(|| Msg::Submit)
                    .on_conflict(Msg::NameConflict);
                ui.text_area(&self.notes)
                    .label("Notes")
                    .placeholder("Anything else?")
                    .max_lines(6)
                    .submit_policy(SubmitPolicy::ModifierEnter)
                    .on_edit(Msg::NotesEdited)
                    .on_submit(|| Msg::Submit)
                    .on_conflict(Msg::NotesConflict);
                ui.row(Row::new().gap(Space::Sm).align(Align::Center), |ui| {
                    ui.button("Submit")
                        .variant(ButtonVariant::Primary)
                        .disabled(self.name.is_empty())
                        .on_press(|| Msg::Submit);
                    ui.group("saved", |ui| {
                        if let Some(saved) = &self.saved {
                            ui.label(format!("Saved: {saved}")).color_role(ColorRole::MutedForeground);
                        }
                    });
                });
                ui.group("notice", |ui| {
                    if let Some(notice) = &self.notice {
                        ui.label(notice.as_str()).color_role(ColorRole::Destructive);
                    }
                });
            });
        });
    }
}
```

## 3. Conditional panel

Conditionals are wrapped in a stable `ui.group` so the region keeps identity
while its contents appear/disappear. Disappearing drops the runtime node and
its native peer; app data survives.

```rust
#[derive(Default)]
struct Inspector {
    detail: TextValue,
    open: bool,
}

enum Msg {
    Edited(TextEdit),
    Toggle,
}

impl Inspector {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::Edited(edit) => {
                self.detail.accept(edit);
            }
            Msg::Toggle => self.open = !self.open,
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Sm), |ui| {
            ui.button(if self.open { "Hide details" } else { "Show details" })
                .variant(ButtonVariant::Secondary)
                .on_press(|| Msg::Toggle);
            ui.group("details-panel", |ui| {
                if self.open {
                    ui.text_area(&self.detail)
                        .label("Details")
                        .max_lines(4)
                        .on_edit(Msg::Edited);
                }
            });
            ui.label("Summary stays mounted")
                .visibility(if self.open { Visibility::Hidden } else { Visibility::Visible });
        });
    }
}
```

The label contrasts with the conditional panel: `Visibility::Hidden` keeps
the node's slot and layout footprint (and a text peer's undo state) while
dropping hit, focus and accessibility; the `if` inside the group fully
unmounts the `TextArea` and reclaims its layout space.

## 4. Keyed list

`ui.keyed` groups rows by a stable business key (`Entry.id`), never the index.
Item IDs are copied into `move` factories — closures never borrow state.
Reordering preserves each row's native peer, undo and focus; removal drops
the peer and releases node resources.

```rust
struct Entry {
    id: u64,
    name: TextValue,
}

#[derive(Default)]
struct Roster {
    entries: Vec<Entry>,
    next_id: u64,
}

enum Msg {
    Add,
    Remove(u64),
    Promote(u64),
    Renamed(u64, TextEdit),
}

impl Roster {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::Add => {
                self.entries.push(Entry { id: self.next_id, name: TextValue::default() });
                self.next_id += 1;
            }
            Msg::Remove(id) => self.entries.retain(|entry| entry.id != id),
            Msg::Promote(id) => {
                if let Some(i) = self.entries.iter().position(|entry| entry.id == id) {
                    if i > 0 {
                        self.entries.swap(i - 1, i);
                    }
                }
            }
            Msg::Renamed(id, edit) => {
                if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
                    entry.name.accept(edit);
                }
            }
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Sm), |ui| {
            ui.keyed(&self.entries, |entry| entry.id, |ui, entry| {
                let id = entry.id;
                ui.row(Row::new().gap(Space::Sm).align(Align::Center), |ui| {
                    ui.text_input(&entry.name)
                        .label("Entry name")
                        .width(Length::Fill(1))
                        .on_edit(move |edit| Msg::Renamed(id, edit));
                    ui.icon_button(Icon::ArrowUp)
                        .label("Move up")
                        .on_press(move || Msg::Promote(id));
                    ui.icon_button(Icon::X)
                        .label("Remove")
                        .on_press(move || Msg::Remove(id));
                });
            });
            ui.button("Add row").on_press(|| Msg::Add);
        });
    }
}
```

## 5. Async Send/Cancel

`ReplyService` is the example's consumer-side contract — the app supplies the
service and the `Executor`; rust-ui supplies task keys, generation fencing
and the bounded `TaskSender` mailbox.

```rust
use std::sync::Arc;
use rust_ui::prelude::*;

pub trait ReplyService: Send + Sync {
    fn reply(
        &self,
        prompt: String,
        cancel: CancelToken,
        chunks: TaskSender<String>,
    ) -> BoxFuture<Result<(), String>>;
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
enum Task {
    Reply,
}

struct Chat {
    service: Arc<dyn ReplyService>,
    draft: TextValue,
    output: String,
    busy: bool,
    error: Option<String>,
}

enum Msg {
    DraftEdited(TextEdit),
    Send,
    Stop,
    Chunk(String),
    Finished(Result<(), String>),
}

const MAX_CHUNK_BYTES: usize = 4096;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

impl Chat {
    fn new(service: Arc<dyn ReplyService>) -> Self {
        Self { service, draft: TextValue::default(), output: String::new(), busy: false, error: None }
    }

    fn update(&mut self, msg: Msg, cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::DraftEdited(edit) => {
                self.draft.accept(edit);
            }
            Msg::Send if self.busy || self.draft.text().trim().is_empty() => {}
            Msg::Send => {
                self.busy = false;
                self.output.clear();
                self.error = None;
                let prompt = self.draft.text().to_string();
                let service = self.service.clone();
                match cx.spawn(Task::Reply, move |job| async move {
                    let result = service
                        .reply(prompt, job.cancellation(), job.sender().map(Msg::Chunk))
                        .await;
                    let _ = job.send(Msg::Finished(result)).await;
                }) {
                    Ok(()) => self.busy = true,
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            Msg::Stop => {
                cx.cancel(Task::Reply);
                self.busy = false;
            }
            Msg::Chunk(text) => {
                if self.busy {
                    if text.len() > MAX_CHUNK_BYTES
                        || text.len() > MAX_RESPONSE_BYTES.saturating_sub(self.output.len())
                    {
                        cx.cancel(Task::Reply);
                        self.busy = false;
                        self.error = Some("Response limit reached".into());
                    } else {
                        self.output.push_str(&text);
                    }
                }
            }
            Msg::Finished(result) => {
                self.busy = false;
                if let Err(e) = result {
                    self.error = Some(e);
                }
            }
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Md), |ui| {
            ui.surface(Surface::new().padding(Space::Sm), |ui| {
                ui.label(self.output.as_str()).wrap(true);
            });
            ui.text_area(&self.draft)
                .label("Message")
                .placeholder("Ask something")
                .max_lines(6)
                .submit_policy(SubmitPolicy::Enter)
                .on_edit(Msg::DraftEdited)
                .on_submit(|| Msg::Send);
            ui.group("action", |ui| {
                if self.busy {
                    ui.button("Stop")
                        .icon(Icon::Square)
                        .variant(ButtonVariant::Destructive)
                        .on_press(|| Msg::Stop);
                } else {
                    ui.button("Send")
                        .icon(Icon::ArrowUp)
                        .variant(ButtonVariant::Primary)
                        .disabled(self.draft.is_empty())
                        .on_press(|| Msg::Send);
                }
            });
            ui.group("error", |ui| {
                if let Some(error) = &self.error {
                    ui.label(error.as_str()).color_role(ColorRole::Destructive);
                }
            });
        });
    }
}

fn run_chat(service: Arc<dyn ReplyService>, executor: Arc<dyn Executor>) -> UiResult {
    App::new(Chat::new(service), Chat::update, Chat::view)
        .title("Chat")
        .executor(executor)
        .run()
}
```

Busy is set only on accepted start and reset on `Finished`, cancellation or
launch error. The `Msg::Send` guard covers both the disabled button and the
keyboard `on_submit` path — they route through the same arm. A conforming
`ReplyService` produces complete UTF-8 chunks of at most `MAX_CHUNK_BYTES`
each and treats a failed `send` or a cancelled `CancelToken` as
end-of-stream; the byte cap is product policy enforced in `update`, not
mailbox magic. The task key belongs to the app scope — swapping the Send/Stop
button never cancels the task it launched.

## 6. Streamed response

Extends Example 5 (same `Task::Reply`, same `cx.spawn` shape, same guarded
`Msg::Chunk` arm — chunks stay bounded by `MAX_CHUNK_BYTES`/
`MAX_RESPONSE_BYTES`). The additions: two `Chat` fields — `turns:
Vec<(u64, String)>` and `next_turn: u64`, initialized in `Chat::new` as
`turns: Vec::new(), next_turn: 0` — plus two product-side bounds, and a
`Finished` arm that archives the completed reply:

```rust
const MAX_TURNS: usize = 32;
const MAX_TRANSCRIPT_BYTES: usize = 256 * 1024;
```

```rust
Msg::Finished(Ok(())) => {
    self.busy = false;
    self.turns.push((self.next_turn, std::mem::take(&mut self.output)));
    self.next_turn += 1;
    while self.turns.len() > MAX_TURNS
        || self.turns.iter().map(|(_, t)| t.len()).sum::<usize>() > MAX_TRANSCRIPT_BYTES
    {
        self.turns.remove(0);
    }
}
Msg::Finished(Err(e)) => {
    self.busy = false;
    self.error = Some(e);
}
```

And in `view`, above the composer:

```rust
ui.keyed(&self.turns, |(id, _)| *id, |ui, (_, text)| {
    ui.surface(Surface::new().padding(Space::Sm), |ui| {
        ui.label(text.as_str()).wrap(true);
    });
});
```

Chunks arrive in order and are never coalesced. The transcript cap bounds
both count and bytes — even empty turns cannot accumulate without limit —
and it is the product's policy; the task queue bound is a count, not bytes.

A stale chunk or completion from the previous generation is dropped at send
and again at dequeue — cancelling then re-sending cannot contaminate the new
stream. `job.send` returning `Err` (closed/cancelled) ends the service loop;
there is no polling.

## 7. Tooltip

A tooltip is runtime hover/focus chrome on an otherwise ordinary control; it
emits no message, never substitutes for the accessible `.label(...)`. A bare
`ui.icon` is decorative (no semantics emitted); an `icon_button` carries a
mandatory accessible name. The `separator` is a one-line leaf. This is an
alternative view fragment for the `Settings` state of example 8.

```rust
fn view(&self, ui: &mut Ui<Msg>) {
    ui.row(Row::new().gap(Space::Sm).align(Align::Center), |ui| {
        ui.icon(Icon::Sparkles);
        ui.label("Appearance:").color_role(ColorRole::MutedForeground);
        ui.icon_button(Icon::Sun)
            .label("Use light theme")
            .tooltip("Switch appearance")
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Sm)
            .on_press(|| Msg::SetTheme(ThemeMode::Light));
        ui.separator();
        ui.badge("Beta");
    });
}
```

## 8. Theme switching

Theme is app data resolved once at the window root — `ui.theme` consumes the
`Theme` value before children build; it is not a per-node cascade.
`ThemeMode::System` follows the OS appearance through the same single call:
`Theme::resolve` maps the mode against `ui.appearance()`, and the runtime
re-runs `view` when the OS appearance changes.

```rust
#[derive(Default)]
struct Settings {
    mode: ThemeMode,
}

enum Msg {
    SetTheme(ThemeMode),
}

impl Settings {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::SetTheme(mode) => self.mode = mode,
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.theme(Theme::resolve(self.mode, ui.appearance()));
        ui.row(Row::new().gap(Space::Sm).align(Align::Center), |ui| {
            ui.icon_button(Icon::Sun)
                .label("Use light theme")
                .on_press(|| Msg::SetTheme(ThemeMode::Light));
            ui.icon_button(Icon::Moon)
                .label("Use dark theme")
                .on_press(|| Msg::SetTheme(ThemeMode::Dark));
            ui.button("System")
                .variant(ButtonVariant::Secondary)
                .on_press(|| Msg::SetTheme(ThemeMode::System));
        });
    }
}
```

## 9. Image and custom render node

`ImageSource::decode_png` runs once outside `view` on bytes the application
supplies; a decode failure is an `AssetError` converted to `UiError`. The
animated tile is a `CustomRender` snapshot the app replaces per frame —
`.on_press` gives the custom node the same keyboard/accessibility activation
as a real button.

```rust
use std::rc::Rc;
use std::time::Duration;

fn tile_path(bounds: Rect, frame: u32) -> Path2d {
    Path2d::rounded_rect(bounds.inset(dp(8.0 + (frame % 4) as f32)), Radius::Md)
}

struct Tile {
    frame: u32,
}

impl CustomRender for Tile {
    fn measure(&self, available: Constraints) -> Size {
        available.constrain(Size::new(dp(96.0), dp(96.0)))
    }
    fn paint(&self, canvas: &mut dyn Canvas, bounds: Rect) {
        canvas.path(&tile_path(bounds, self.frame), Paint::fill_role(ColorRole::Accent));
    }
    fn hit_test(&self, local: Point, bounds: Rect) -> bool {
        bounds.contains_local(local)
    }
    fn semantics(&self) -> Semantics {
        Semantics {
            role: Role::Button,
            label: "Demo tile".into(),
            actions: Vec::from([SemanticsAction::Press]),
        }
    }
}

struct Demo {
    tile: Rc<dyn CustomRender>,
    image: ImageSource,
    playing: bool,
    elapsed: Duration,
}

enum Msg {
    Frame(FrameTime),
    Toggle,
}

impl Demo {
    fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::Frame(time) => {
                if !self.playing {
                    return;
                }
                self.elapsed += time.delta;
                let frame = (self.elapsed.as_millis() / 33).min(240) as u32;
                self.tile = Rc::new(Tile { frame });
                if frame >= 240 {
                    self.playing = false;
                }
            }
            Msg::Toggle => {
                self.playing = !self.playing;
                if self.playing {
                    self.elapsed = Duration::ZERO;
                    self.tile = Rc::new(Tile { frame: 0 });
                }
            }
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.row(Row::new().gap(Space::Md).align(Align::Start), |ui| {
            ui.image(self.image.clone()).width(Length::Fixed(dp(96.0)));
            ui.custom(self.tile.clone())
                .frame_events(self.playing)
                .on_frame(Msg::Frame)
                .on_press(|| Msg::Toggle);
        });
    }
}

fn run_image(image: ImageSource) -> UiResult {
    let demo = Demo {
        tile: Rc::new(Tile { frame: 0 }),
        image,
        playing: false,
        elapsed: Duration::ZERO,
    };
    App::new(demo, Demo::update, Demo::view).title("Media").run()
}

fn run_png(bytes: &[u8]) -> UiResult {
    let image = ImageSource::decode_png(bytes)?;
    run_image(image)
}
```

`elapsed` accumulates real `Duration`, so a 16 ms frame is never truncated to
zero and the fractional remainder survives successive frame ticks; a `Toggle`
restart deliberately resets `elapsed`. `Paint::fill_role` resolves the role through the canvas's current theme,
so a `ui.theme` switch repaints the tile correctly. A native `TextArea`
layered beside the tile sits in its own rectangle — painted content never
alpha-overlaps a live text peer.

## 10. Composer as a scoped child component

A child is a plain struct with `update`/`view`; the parent owns its state,
adapts messages with `ui.scope` for view and `cx.scope` for update, and can
observe semantic child messages while forwarding them.

```rust
#[derive(Default)]
struct Composer {
    draft: TextValue,
    notice: Option<String>,
}

enum ComposerMsg {
    Edited(TextEdit),
    SubmitRequested,
    Conflict(TextConflict),
}

impl Composer {
    fn update(&mut self, msg: ComposerMsg, _cx: &mut UpdateCtx<ComposerMsg>) {
        match msg {
            ComposerMsg::Edited(edit) => {
                self.draft.accept(edit);
            }
            ComposerMsg::SubmitRequested => {}
            ComposerMsg::Conflict(conflict) => {
                self.draft.keep_native(conflict);
                self.notice =
                    Some("Text changed during editing; review before clearing.".into());
            }
        }
    }

    fn view(&self, ui: &mut Ui<ComposerMsg>) {
        ui.column(Column::new().gap(Space::Sm), |ui| {
            ui.text_area(&self.draft)
                .label("Composer")
                .placeholder("Message")
                .max_lines(6)
                .submit_policy(SubmitPolicy::Enter)
                .on_edit(ComposerMsg::Edited)
                .on_submit(|| ComposerMsg::SubmitRequested)
                .on_conflict(ComposerMsg::Conflict);
            ui.group("composer-notice", |ui| {
                if let Some(notice) = &self.notice {
                    ui.label(notice.as_str()).color_role(ColorRole::Destructive);
                }
            });
        });
    }
}

struct Inbox {
    composer: Composer,
    sent: Vec<String>,
}

enum Msg {
    Composer(ComposerMsg),
}

impl Inbox {
    fn update(&mut self, msg: Msg, cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::Composer(child) => {
                if matches!(child, ComposerMsg::SubmitRequested) {
                    self.sent.push(self.composer.draft.text().to_string());
                    self.composer.draft.clear();
                }
                self.composer.update(child, &mut cx.scope("composer", Msg::Composer));
            }
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.column(Column::new().gap(Space::Md), |ui| {
            for text in &self.sent {
                ui.label(text.as_str());
            }
            ui.scope("composer", Msg::Composer, |ui| self.composer.view(ui));
        });
    }
}
```

`cx.scope("composer", Msg::Composer)` is the update-side counterpart of
`ui.scope`: it uses the same stable key namespace, and child spawns/sends
through the scoped context arrive at the parent's `update` already wrapped —
a second composer on a different key could spawn the same `Task::Reply`
without collisions. Removing `self.composer` drops its state; any task it
spawned is cancelled explicitly through that same scope first.

This `Inbox` is a composition demo only — a production transcript applies the
count+byte bounding and keyed records of example 6 rather than an unbounded
`Vec`.

## Future controls (not v0.1)

Same core, no redesign — each is a typed props struct plus a typed
`.on_change(Fn(T) -> M)` / `.on_open_change(Fn(bool) -> M)` — declared future
events, not existing v0.1 handlers:

```rust
ui.checkbox(self.enabled).label("Enabled").on_change(Msg::EnabledChanged);
ui.switch(self.power).label("Power").on_change(Msg::PowerChanged);
ui.radio_group(&self.choice, &[Choice::A, Choice::B]).on_change(Msg::ChoiceChanged);
ui.select(&self.kind, &[Kind::X, Kind::Y]).on_change(Msg::KindChanged).on_open_change(Msg::KindOpen);
ui.popover(&self.info_open, |ui| { ui.label("Detail"); }).on_open_change(Msg::InfoOpen);
ui.scroll_area(|ui| {
    for text in &self.rows {
        ui.label(text.as_str());
    }
})
.on_scroll(Msg::Scrolled);
ui.dialog(&self.confirm_open).title("Discard?").on_open_change(Msg::ConfirmOpen);
```
