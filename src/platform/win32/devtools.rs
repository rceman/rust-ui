//! Retained-tree authority. All mutations enter under Backend::guarded_turn.
use super::*;
use crate::devtools::{self as d, Error, Node, Rect, Result, Snapshot};
use serde_json::{Value, json};
fn platform(e: impl ToString) -> Error {
    Error::new("UI_ERROR", e)
}
impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    fn dev_snapshot(&self) -> Result<Snapshot> {
        let scale = self.peer_ctx.scale.get();
        let mut client = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut client).map_err(platform)?;
        }
        let physical = Rect {
            x: 0.,
            y: 0.,
            width: f64::from(client.right - client.left),
            height: f64::from(client.bottom - client.top),
        };
        let mut count = 0;
        let root = self.dev_node(self.rt.root, 0, &mut count)?;
        let snapshot = Snapshot {
            schema: d::SNAPSHOT_SCHEMA.into(),
            scale_factor: scale.0 as f64,
            client_dp: Rect {
                x: 0.,
                y: 0.,
                width: f64::from(scale.to_logical(client.right - client.left)),
                height: f64::from(scale.to_logical(client.bottom - client.top)),
            },
            client_px: physical,
            root,
        };
        snapshot.index()?;
        Ok(snapshot)
    }
    fn dev_node(&self, id: NodeId, depth: usize, count: &mut usize) -> Result<Node> {
        *count += 1;
        if depth > 128 || *count > 8192 {
            return Err(Error::new("MESSAGE_TOO_LARGE", "semantic tree limit"));
        }
        let n = self
            .rt
            .arena
            .get(id)
            .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "stale semantic node"))?;
        let r = if id == self.rt.root {
            let mut c = RECT::default();
            unsafe {
                GetClientRect(self.hwnd, &mut c).map_err(platform)?;
            }
            LogicalRect {
                x: 0.,
                y: 0.,
                width: self.peer_ctx.scale.get().to_logical(c.right - c.left),
                height: self.peer_ctx.scale.get().to_logical(c.bottom - c.top),
            }
        } else {
            self.rects.get(&id).copied().unwrap_or_default()
        };
        let p = r.physical(self.peer_ctx.scale.get());
        let (kind, label, enabled) = match &n.data {
            NodeData::Empty => ("root", String::new(), true),
            NodeData::Container { .. } => ("container", String::new(), true),
            NodeData::Label { text, .. } => ("label", text.chars().take(1024).collect(), true),
            NodeData::Button { text, disabled, .. } => {
                ("button", text.chars().take(1024).collect(), !*disabled)
            }
            NodeData::Action {
                label, disabled, ..
            } => ("action", label.chars().take(1024).collect(), !*disabled),
            NodeData::Editor {
                snapshot, disabled, ..
            } => (
                "text-input",
                snapshot.committed.chars().take(1024).collect(),
                !*disabled,
            ),
            NodeData::Custom { .. } => ("custom", String::new(), true),
        };
        let children = n
            .children
            .iter()
            .map(|slot| {
                self.dev_node(
                    NodeId {
                        slot: *slot,
                        generation: self.rt.arena.generation_of(*slot),
                    },
                    depth + 1,
                    count,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Node {
            automation_id: n.automation_id.as_deref().map(str::to_owned),
            kind: kind.into(),
            label,
            rect_dp: Rect {
                x: r.x as f64,
                y: r.y as f64,
                width: r.width as f64,
                height: r.height as f64,
            },
            rect_px: Rect {
                x: p.left as f64,
                y: p.top as f64,
                width: (p.right - p.left) as f64,
                height: (p.bottom - p.top) as f64,
            },
            visible: n.visibility == crate::Visibility::Visible && r.width > 0. && r.height > 0.,
            enabled,
            focused: self.focus == Some(id),
            focus_visible: self.focus == Some(id),
            hovered: self.hot == Some(id),
            pressed: self.pressed == Some(id),
            children,
        })
    }
    fn dev_target(&self, args: &Value) -> Result<NodeId> {
        let wanted = args["id"]
            .as_str()
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "id required"))?;
        let mut found = None;
        for slot in 0..self.rt.arena.slot_len() {
            if self
                .rt
                .arena
                .slot(slot as u32)
                .is_some_and(|n| n.automation_id.as_deref() == Some(wanted))
            {
                if found.is_some() {
                    return Err(Error::new("DUPLICATE_ID", wanted));
                }
                found = Some(NodeId {
                    slot: slot as u32,
                    generation: self.rt.arena.generation_of(slot as u32),
                })
            }
        }
        found.ok_or_else(|| Error::new("TARGET_NOT_FOUND", wanted))
    }
    fn dev_point(&self, id: NodeId) -> Result<space::ClientPhysicalPoint> {
        let snap = self.dev_snapshot()?;
        let n = self.rt.arena.get(id).unwrap();
        if !n.interactive() {
            return Err(Error::new(
                "NOT_SUPPORTED",
                "target is not interactive/enabled",
            ));
        }
        let r = self
            .rects
            .get(&id)
            .ok_or_else(|| Error::new("TARGET_NOT_VISIBLE", "no layout"))?;
        let px = r.physical(self.peer_ctx.scale.get());
        if r.width <= 0.
            || r.height <= 0.
            || px.left < 0
            || px.top < 0
            || px.right as f64 > snap.client_px.width
            || px.bottom as f64 > snap.client_px.height
        {
            return Err(Error::new("TARGET_NOT_VISIBLE", "target outside client"));
        }
        let logical = crate::Point {
            x: r.x + r.width / 2.,
            y: r.y + r.height / 2.,
        };
        if self.hit_test(logical) != Some(id) {
            return Err(Error::new(
                "TARGET_NOT_VISIBLE",
                "target center occluded or not hit owner",
            ));
        }
        Ok(space::ClientPhysicalPoint(crate::geom::PhysicalPoint {
            x: self.peer_ctx.scale.get().to_physical(logical.x),
            y: self.peer_ctx.scale.get().to_physical(logical.y),
        }))
    }
    fn dev_command(&mut self, command: &str, args: &Value) -> Result<Value> {
        match command {
            "handshake" => {
                let s = self.dev_snapshot()?;
                Ok(
                    json!({"hwnd":self.hwnd.0 as usize,"pid":std::process::id(),"paint_epoch":self.paint_epoch.get(),"counters":{"native_timer_fires":self.counters.native_timer_fires.load(Ordering::Relaxed),"requested_redraws":self.counters.requested_redraws.load(Ordering::Relaxed)},"scale_factor":s.scale_factor,"client_dp":s.client_dp,"client_px":s.client_px,"protocol":d::PROTOCOL}),
                )
            }
            "tree" | "snapshot-layout" => {
                serde_json::to_value(self.dev_snapshot()?).map_err(d::ioerr)
            }
            "rect" => {
                let id = self.dev_target(args)?;
                let snapshot = self.dev_snapshot()?;
                let n = snapshot.index()?[self
                    .rt
                    .arena
                    .get(id)
                    .unwrap()
                    .automation_id
                    .as_deref()
                    .unwrap()]
                .0;
                Ok(
                    json!({"id":n.automation_id,"rect_dp":n.rect_dp,"rect_px":n.rect_px,"scale_factor":snapshot.scale_factor,"coordinate_space":"client-relative","visible":n.visible,"enabled":n.enabled}),
                )
            }
            "hover" | "press" | "release" | "click" => {
                let id = self.dev_target(args)?;
                let p = self.dev_point(id)?;
                match command {
                    "hover" => self.hover(p, 0).map_err(platform)?,
                    "press" => self
                        .pointer(
                            crate::node::PointerPhase::Down,
                            p,
                            Some(PointerButton::Primary),
                            1,
                        )
                        .map_err(platform)?,
                    "release" => self
                        .pointer(
                            crate::node::PointerPhase::Up,
                            p,
                            Some(PointerButton::Primary),
                            0,
                        )
                        .map_err(platform)?,
                    _ => {
                        self.pointer(
                            crate::node::PointerPhase::Down,
                            p,
                            Some(PointerButton::Primary),
                            1,
                        )
                        .map_err(platform)?;
                        self.pointer(
                            crate::node::PointerPhase::Up,
                            p,
                            Some(PointerButton::Primary),
                            0,
                        )
                        .map_err(platform)?;
                    }
                }
                Ok(json!({"performed":command,"id":args["id"]}))
            }
            "focus" => {
                let id = self.dev_target(args)?;
                self.dev_point(id)?;
                if args
                    .get("modality")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s != "keyboard")
                {
                    return Err(Error::new(
                        "NOT_SUPPORTED",
                        "v0.1 supports keyboard focus modality",
                    ));
                }
                self.set_focus(Some(id)).map_err(platform)?;
                self.turn().map_err(platform)?;
                Ok(json!({"focused":args["id"],"modality":"keyboard"}))
            }
            "key" => {
                let name = args["key"]
                    .as_str()
                    .ok_or_else(|| Error::new("INVALID_ARGUMENT", "key required"))?;
                let vk = match name {
                    "Enter" => 13,
                    "Space" => 32,
                    "Tab" => 9,
                    "Escape" => 27,
                    "Left" => 37,
                    "Up" => 38,
                    "Right" => 39,
                    "Down" => 40,
                    "Backspace" => 8,
                    "Delete" => 46,
                    "Home" => 36,
                    "End" => 35,
                    _ => {
                        return Err(Error::new(
                            "NOT_SUPPORTED",
                            "key subset: Enter Space Tab Escape arrows Backspace Delete Home End",
                        ));
                    }
                };
                self.key_msg(WM_KEYDOWN, vk, 0).map_err(platform)?;
                self.key_msg(WM_KEYUP, vk, 0).map_err(platform)?;
                Ok(json!({"key":name,"delivered":true}))
            }
            "shutdown" => Ok(json!({"shutdown":true})),
            _ => Err(Error::new("NOT_SUPPORTED", command)),
        }
    }
    pub(super) fn service_devtools(&mut self) -> UiResult {
        let Some(mut server) = self.devtools.take() else {
            return Ok(());
        };
        let fatal = server.shared.fatal.lock().unwrap().take();
        if let Some(e) = fatal {
            let text = e.to_string();
            self.devtools = Some(server);
            return Err(UiError::Platform(text));
        }
        if server.shared.close_requested.load(Ordering::Acquire) {
            self.devtools = Some(server);
            unsafe {
                DestroyWindow(self.hwnd).map_err(|e| UiError::Platform(e.to_string()))?;
            }
            return Ok(());
        }
        for _ in 0..16 {
            let Ok(e) = server.queue.try_recv() else {
                break;
            };
            let allowed = {
                let mut permit = e.admission.lock().unwrap();
                if *permit == d::client::Admission::Ready {
                    *permit = d::client::Admission::Executing;
                    true
                } else {
                    false
                }
            };
            if !allowed {
                let _ = e.reply.send(d::Response::from_result(
                    e.request.id,
                    Err(Error::new(
                        "SESSION_CLOSED",
                        "request not admitted by wake authority",
                    )),
                ));
                continue;
            }
            if std::time::Instant::now() >= e.deadline {
                let _ = e.reply.send(d::Response::from_result(
                    e.request.id,
                    Err(Error::new("TIMEOUT", "request expired before UI dispatch")),
                ));
                continue;
            }
            if e.request.command == "wait-idle" {
                if server
                    .idle
                    .as_ref()
                    .is_some_and(|e| std::time::Instant::now() >= e.deadline)
                {
                    server.idle.take();
                }
                if server.idle.is_some() {
                    let _ = e.reply.send(d::Response::from_result(
                        e.request.id,
                        Err(Error::new("BUSY", "idle barrier already pending")),
                    ));
                } else {
                    server.idle = Some(e);
                }
                continue;
            }
            let result = self.dev_command(&e.request.command, &e.request.arguments);
            let fatal = result
                .as_ref()
                .err()
                .filter(|e| e.code == "UI_ERROR")
                .map(ToString::to_string);
            let _ = e.reply.send(d::Response::from_result(e.request.id, result));
            if let Some(text) = fatal {
                self.devtools = Some(server);
                return Err(UiError::Platform(text));
            }
        }
        self.devtools = Some(server);
        Ok(())
    }
    fn dev_idle_state(&self) -> (bool, Value) {
        let dirty = (0..self.rt.arena.slot_len()).any(|i| {
            self.rt.arena.slot(i as u32).is_some_and(|n| {
                n.dirty
                    & (crate::runtime::Runtime::<S, M, U, V>::DIRTY_LAYOUT
                        | crate::runtime::Runtime::<S, M, U, V>::DIRTY_PAINT)
                    != 0
            })
        });
        let pending_paint =
            unsafe { windows::Win32::Graphics::Gdi::GetUpdateRect(self.hwnd, None, false) }
                .as_bool();
        let (mailbox, events, reentrant, deferred, native) = (
            self.rt.mailbox.queue_len(),
            !self.rt.events.is_empty(),
            self.reentrant_queue.borrow().len(),
            self.deferred_native.len(),
            self.peer_ctx.sink.lock().unwrap().len(),
        );
        let (damage, deadline, pump, state) = (
            self.damage.get().is_some(),
            self.rt.next_deadline().is_some(),
            self.pump_queued.get(),
            self.state_dirty.get(),
        );
        let diagnostics = json!({"mailbox":mailbox,"events":events,"reentrant":reentrant,"deferred_native":deferred,"native_notifications":native,"state_dirty":state,"dirty_layout_or_paint":dirty,"pending_paint":pending_paint,"damage":damage,"scheduled_deadline":deadline,"pump_queued":pump});
        (
            mailbox == 0
                && !events
                && reentrant == 0
                && deferred == 0
                && native == 0
                && state == 0
                && !dirty
                && !pending_paint
                && !damage
                && !deadline
                && !pump,
            diagnostics,
        )
    }
    pub(super) fn complete_devtools_idle(&mut self) {
        if self.devtools.as_ref().is_none_or(|s| s.idle.is_none()) {
            return;
        }
        // Presentation and native queries are a native Backend entry too.
        // Synchronous callbacks queue; they cannot form an aliasing &mut.
        let previous = self.in_dispatch.replace(true);
        let (idle, diagnostics) = self.dev_idle_state();
        if let Some(server) = self.devtools.as_ref() {
            *server.shared.diagnostics.lock().unwrap() = diagnostics;
        }
        let barrier =
            idle.then(|| unsafe { windows::Win32::Graphics::Dwm::DwmFlush() }.map_err(platform));
        self.in_dispatch.set(previous);
        if !previous {
            if let Err(e) = self.drain_reentrant() {
                self.mark_fatal(e);
                return;
            }
        }
        self.in_dispatch.set(true);
        let (still_idle, diagnostics) = self.dev_idle_state();
        self.in_dispatch.set(previous);
        if let Some(server) = self.devtools.as_ref() {
            *server.shared.diagnostics.lock().unwrap() = diagnostics;
        }
        if let Some(barrier) = barrier {
            if barrier.is_err() || still_idle {
                let result=barrier.map(|_|json!({"idle":true,"mailbox":0,"events":0,"damage":false,"scheduled_deadline":false,"present_barrier":"DwmFlush"}));
                if let Some(e) = self.devtools.as_mut().and_then(|s| s.idle.take()) {
                    let _ = e.reply.send(d::Response::from_result(e.request.id, result));
                }
            }
        }
    }
}
