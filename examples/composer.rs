//! Checkpoint-2 composer spike — a REAL consumer of the public API.
//!
//! State: draft + A/B/C row `TextValue`s, fake streamed response, busy
//! guard, details toggle, theme/reduced-motion plain buttons, a neutral
//! geometric custom tile, programmatic replace, reorder and B remove/recreate
//! — all through the documented `App`/`Ui`/`UpdateCtx` surface.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Context;
use std::thread;
use std::time::Duration;

use rust_ui::*;

// ---------------------------------------------------------------------------
// app-owned executor — thread-per-job (max 8), Waker = park/unpark, no frame
// polling. The fake stream blocks the WORKER on finite deterministic delays.
// ---------------------------------------------------------------------------

struct Parker(thread::Thread);

impl std::task::Wake for Parker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// thread-per-job executor capped at 8 live workers — the worker blocks on
/// `thread::park` between polls and the waker unparks it; no frame polling.
struct ThreadExecutor {
    live: Arc<AtomicUsize>,
}

impl Executor for ThreadExecutor {
    fn spawn(&self, mut task: BoxFuture<()>) -> Result<(), TaskStartError> {
        if self.live.fetch_add(1, Ordering::SeqCst) >= 8 {
            self.live.fetch_sub(1, Ordering::SeqCst);
            return Err(TaskStartError::Rejected);
        }
        let live = self.live.clone();
        thread::spawn(move || {
            let waker = std::task::Waker::from(Arc::new(Parker(thread::current())));
            let mut cx = Context::from_waker(&waker);
            while task.as_mut().poll(&mut cx).is_pending() {
                thread::park();
            }
            live.fetch_sub(1, Ordering::SeqCst);
        });
        Ok(())
    }
}

impl ThreadExecutor {
    fn new() -> Arc<ThreadExecutor> {
        Arc::new(ThreadExecutor {
            live: Arc::new(AtomicUsize::new(0)),
        })
    }
}

// ---------------------------------------------------------------------------
// state + messages (documented surface)
// ---------------------------------------------------------------------------

struct Entry {
    id: u64,
    name: TextValue,
}

struct Spike {
    draft: TextValue,
    /// multiline composer body — real `text_area` peer (max-lines bound)
    body: TextValue,
    /// draft read-only toggle — read-only vs disabled evidence surface
    draft_ro: bool,
    rows: Vec<Entry>,
    response: String,
    busy: bool,
    notice: Option<String>,
    show_details: bool,
    theme_mode: ThemeMode,
    reduced: ReducedMotion,
    /// toggles the draft editor's surgical patch on/off live — exercises
    /// apply_text_style on a mounted peer without remount
    draft_bold: bool,
    tile: Rc<dyn CustomRender>,
    turn_counter: u64,
}

enum Msg {
    DraftEdited(TextEdit),
    DraftConflict(TextConflict),
    RowEdited(u64, TextEdit),
    RowConflict(u64, TextConflict),
    Send,
    Stop,
    Chunk(String),
    Finished,
    ToggleDetails,
    Reorder,
    ProgrammaticReplace,
    SetTheme(ThemeMode),
    SetReduced(ReducedMotion),
    ToggleDraftBold,
    ToggleDraftReadOnly,
    BodyEdited(TextEdit),
    RemoveB,
    RecreateB,
}

const CHUNKS: usize = 16;
const CHUNK_MS: u64 = 60;
const RESPONSE_CAP: usize = 64 * 1024;
const CHUNK_MAX: usize = 4096;

impl Spike {
    fn new() -> Self {
        // RUI_ROWS=<n> scales the keyed list for the 100/1000-node runs
        let n: u64 = std::env::var("RUI_ROWS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3)
            .clamp(1, 4096);
        Spike {
            draft: TextValue::new("draft-prefill"),
            body: TextValue::new(""),
            draft_ro: false,
            rows: (1..=n)
                .map(|i| Entry {
                    id: i,
                    name: TextValue::new(format!("row-{i}")),
                })
                .collect(),
            response: String::new(),
            busy: false,
            notice: None,
            show_details: false,
            theme_mode: ThemeMode::System,
            reduced: ReducedMotion::System,
            draft_bold: false,
            tile: Rc::new(Tile),
            turn_counter: 0,
        }
    }

    fn row(&mut self, id: u64) -> Option<&mut TextValue> {
        self.rows
            .iter_mut()
            .find(|e| e.id == id)
            .map(|e| &mut e.name)
    }

    // the deterministic fake stream — 16 chunks 60 ms apart, worker-side
    // finite blocking delay (not UI polling); cancellation checked before
    // and after every delay; ends on any failed send.
    async fn stream(job: Job<Msg>) {
        let cancel = job.cancellation();
        for i in 0..CHUNKS {
            if cancel.is_cancelled() {
                return;
            }
            thread::sleep(Duration::from_millis(CHUNK_MS));
            if cancel.is_cancelled() {
                return;
            }
            let chunk = format!("chunk-{:02} ", i);
            debug_assert!(chunk.len() <= CHUNK_MAX);
            if job.send(Msg::Chunk(chunk)).await.is_err() {
                return;
            }
        }
        let _ = job.send(Msg::Finished).await;
    }

    fn update(&mut self, msg: Msg, cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::DraftEdited(e) => {
                let _ = self.draft.accept(e);
            }
            Msg::DraftConflict(c) => {
                // keep the native peer's committed text; surface a notice
                let _ = self.draft.keep_native(c);
                self.notice = Some("draft edit rejected — kept native".into());
            }
            Msg::RowEdited(id, e) => {
                if let Some(v) = self.row(id) {
                    let _ = v.accept(e);
                }
            }
            Msg::RowConflict(id, c) => {
                if let Some(v) = self.row(id) {
                    let _ = v.keep_native(c);
                    self.notice = Some(format!("row {id} edit rejected — kept native"));
                }
            }
            Msg::Send => {
                if self.busy {
                    return;
                }
                // spawn BEFORE busy — a rejected/failed start must not
                // leave the UI stuck; the task key replaces any prior
                // registration deterministically
                match cx.spawn("stream", |job| async move { Self::stream(job).await }) {
                    Ok(()) => {
                        self.busy = true;
                        self.response.clear();
                        self.turn_counter += 1;
                    }
                    Err(e) => {
                        self.notice = Some(format!("send rejected: {e:?}"));
                    }
                }
            }
            Msg::Stop => {
                // cancel fences the registration — late chunks/completion
                // drop at mailbox resolve; busy clears so a resend is legal
                cx.cancel("stream");
                self.busy = false;
            }
            Msg::Chunk(c) => {
                if self.busy {
                    let room = RESPONSE_CAP.saturating_sub(self.response.len());
                    self.response.push_str(&c[..c.len().min(room)]);
                }
            }
            Msg::Finished => self.busy = false,
            Msg::ToggleDetails => self.show_details = !self.show_details,
            Msg::Reorder => self.rows.rotate_right(1), // A/B/C -> C/A/B
            Msg::ProgrammaticReplace => {
                let _ = self.draft.replace("replaced programmatically");
            }
            Msg::SetTheme(m) => self.theme_mode = m,
            Msg::SetReduced(r) => self.reduced = r,
            Msg::ToggleDraftBold => self.draft_bold = !self.draft_bold,
            Msg::ToggleDraftReadOnly => self.draft_ro = !self.draft_ro,
            Msg::BodyEdited(e) => {
                let _ = self.body.accept(e);
            }
            Msg::RemoveB => self.rows.retain(|e| e.id != 2),
            Msg::RecreateB => {
                if !self.rows.iter().any(|e| e.id == 2) {
                    self.rows.insert(
                        1,
                        Entry {
                            id: 2,
                            name: TextValue::new("B"),
                        },
                    );
                }
            }
        }
    }

    fn view(&self, ui: &mut Ui<Msg>) {
        ui.theme(Theme::resolve(self.theme_mode, ui.appearance()).reduced_motion(self.reduced));
        ui.column(Column::new().gap(Space::Sm).padding(Space::Md), |ui| {
            ui.label("composer spike");
            ui.group("draft", |ui| {
                // capability-limited editor patch — chrome + foreground
                // only; size/weight are OS-owned (approved boundary)
                ui.text_input(&self.draft)
                    .placeholder("draft")
                    .label("draft")
                    .style(if self.draft_bold {
                        TextInputStylePatch::new()
                            .foreground(Color::rgb(180, 60, 40))
                            .border_bottom_color(Color::rgb(180, 60, 40))
                            .border_bottom_width(dp(2.0))
                    } else {
                        TextInputStylePatch::new()
                    })
                    .read_only(self.draft_ro)
                    .on_edit(Msg::DraftEdited)
                    .on_conflict(Msg::DraftConflict)
                    .on_submit(|| Msg::Send);
                // multiline editor — max_lines bounds the native peer's
                // growth; read-only draft above is the inert-editor surface
                ui.text_area(&self.body)
                    .placeholder("body — multiline, 4-line cap")
                    .label("body")
                    .max_lines(4)
                    .on_edit(Msg::BodyEdited);
            });
            // conditional group placed BEFORE the keyed rows — toggling
            // details never remounts the sibling editors
            ui.group("details", |ui| {
                if self.show_details {
                    ui.surface(Surface::new().padding(Space::Sm), |ui| {
                        ui.label(format!(
                            "turn {} | busy {} | notice {}",
                            self.turn_counter,
                            self.busy,
                            self.notice.as_deref().unwrap_or("-"),
                        ));
                    });
                }
            });
            ui.row(Row::new().gap(Space::Sm), |ui| {
                ui.button(if self.show_details {
                    "hide details"
                } else {
                    "show details"
                })
                .variant(ButtonVariant::Secondary)
                .on_press(|| Msg::ToggleDetails);
            });
            ui.keyed(
                &self.rows,
                |e| e.id,
                |ui, e| {
                    let id = e.id;
                    ui.text_input(&e.name)
                        .label(&format!("row {}", id))
                        .on_edit(move |t| Msg::RowEdited(id, t))
                        .on_conflict(move |c| Msg::RowConflict(id, c));
                },
            );
            ui.row(Row::new().gap(Space::Sm), |ui| {
                // SURGICAL — the documented mandatory case: only the bottom
                // border changes as authored/resolved values
                ui.button("send")
                    .disabled(self.busy)
                    .style(
                        ButtonStylePatch::new()
                            .border_bottom_width(dp(2.0))
                            .border_bottom_color(Color::rgb(255, 0, 0)),
                    )
                    .on_press(|| Msg::Send);
                ui.button("stop")
                    .variant(ButtonVariant::Destructive)
                    .disabled(!self.busy)
                    .on_press(|| Msg::Stop);
                ui.button("reorder").on_press(|| Msg::Reorder);
                ui.button("replace").on_press(|| Msg::ProgrammaticReplace);
            });
            ui.row(Row::new().gap(Space::Sm), |ui| {
                ui.button("dark")
                    .variant(ButtonVariant::Secondary)
                    .on_press(|| Msg::SetTheme(ThemeMode::Dark));
                ui.button("light")
                    .variant(ButtonVariant::Secondary)
                    .on_press(|| Msg::SetTheme(ThemeMode::Light));
                ui.button("reduce")
                    .variant(ButtonVariant::Secondary)
                    .on_press(|| Msg::SetReduced(ReducedMotion::Reduce));
                ui.button("motion")
                    .variant(ButtonVariant::Secondary)
                    .on_press(|| Msg::SetReduced(ReducedMotion::NoPreference));
                ui.button(if self.draft_bold { "unbold" } else { "bold" })
                    .on_press(|| Msg::ToggleDraftBold);
                ui.button(if self.draft_ro { "ro off" } else { "ro on" })
                    .on_press(|| Msg::ToggleDraftReadOnly);
                ui.button("rm B").on_press(|| Msg::RemoveB);
                ui.button("mk B").on_press(|| Msg::RecreateB);
            });
            ui.label(self.response.as_str()).wrap(true);
            // CUSTOM — a semantic action built only from public primitives:
            // Action props + box_/text children; activates like a button
            ui.action(
                Action::new().label("custom tile action").style(
                    ActionStyle::new(
                        BoxStyle::new()
                            .background(Color::role(ColorRole::Muted))
                            .radii(CornerRadii::all(dp(8.0)))
                            .padding(Insets::all(dp(10.0)))
                            .shadow(Some(Shadow {
                                color: Color::role(ColorRole::Shadow),
                                offset_x: dp(0.0),
                                offset_y: dp(3.0),
                                blur_sigma: dp(6.0),
                            })),
                    )
                    .hover(BoxStylePatch::new().background(Color::role(ColorRole::Border))),
                ),
                |ui| {
                    ui.row(Row::new().gap(Space::Sm), |ui| {
                        ui.label("custom action").color_role(ColorRole::Foreground);
                        ui.custom(self.tile.clone());
                    });
                },
            )
            .on_press(|| Msg::ToggleDetails);
        });
    }
}

/// The neutral geometric tile — static, role Image, no frame demand.
struct Tile;

impl CustomRender for Tile {
    fn measure(&self, _c: Constraints) -> Size {
        Size {
            width: 96.0,
            height: 48.0,
        }
    }
    fn paint(&self, canvas: &mut dyn Canvas, bounds: Rect) {
        let mut p = Path2d::default();
        p.ops.push(PathOp::MoveTo(Point {
            x: bounds.x,
            y: bounds.y,
        }));
        p.ops.push(PathOp::LineTo(Point {
            x: bounds.x + bounds.width,
            y: bounds.y,
        }));
        p.ops.push(PathOp::LineTo(Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height,
        }));
        p.ops.push(PathOp::Close);
        canvas.path(&p, Paint::fill_role(ColorRole::Accent));
    }
    fn semantics(&self) -> Semantics {
        Semantics {
            role: Role::Image,
            label: "decorative tile".into(),
            actions: Vec::new(),
        }
    }
}

fn main() -> UiResult {
    let app = App::new(Spike::new(), Spike::update, Spike::view)
        .title("rust-ui composer")
        .executor(ThreadExecutor::new());
    app.run()
}
