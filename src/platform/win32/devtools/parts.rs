use super::super::*;
use crate::devtools::{self as d, Error, Node, Rect, Result, Snapshot};
use crate::platform::win32::devtools::{platform, toggle_part_rects};
use crate::runtime::UpdateCtx;
use crate::{NodeId, UiResult};
use serde_json::{Value, json};

impl<S, M, U, V> Backend<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut crate::Ui<'_, '_, M>),
{
    /// chains compose `a.b[0]` keys; a nested automation_id bounds its
    /// own part scope and is not descended into.
    pub(super) fn collect_parts(
        &self,
        id: NodeId,
        prefix: &mut Vec<String>,
        out: &mut Vec<d::PartNode>,
    ) {
        let Some(n) = self.rt.arena.get(id) else {
            return;
        };
        for slot in &n.children {
            let cid = NodeId {
                slot: *slot,
                generation: self.rt.arena.generation_of(*slot),
            };
            let Some(c) = self.rt.arena.get(cid) else {
                continue;
            };
            // nested owner starts a fresh part scope — its parts belong
            // to itself, not to this node
            if c.automation_id.is_some() {
                continue;
            }
            let mut descend = c.part_key.is_none();
            if let Some(pk) = &c.part_key {
                prefix.push(pk.to_string());
                let r = self.rects.get(&cid).copied().unwrap_or_default();
                let p = r.physical(self.peer_ctx.scale.get());
                out.push(d::PartNode {
                    key: prefix.join("."),
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
                    // parts inherit their owner node's viewport clip —
                    // a clipped-out node can't carry on-screen parts
                    visible: c.visibility == crate::Visibility::Visible
                        && !self.clip_fully_excludes(cid, r),
                    reason: None,
                });
                descend = true;
            }
            // unnamed Toggle — its intrinsic parts emit under the prefix
            // (e.g. `thumb` inside `field.horizontal`, or
            // `item[k].indicator` under a part-marked radio item)
            if let NodeData::Toggle {
                kind,
                size,
                checked,
                label,
                ..
            } = &c.data
            {
                let r = self.rects.get(&cid).copied().unwrap_or_default();
                for (key, pr) in
                    toggle_part_rects(self, kind, *size, *checked, label.as_deref(), &r)
                {
                    let pp = pr.physical(self.peer_ctx.scale.get());
                    let full = {
                        prefix.push(key.into());
                        prefix.join(".")
                    };
                    out.push(d::PartNode {
                        key: full,
                        rect_dp: Rect {
                            x: pr.x as f64,
                            y: pr.y as f64,
                            width: pr.width as f64,
                            height: pr.height as f64,
                        },
                        rect_px: Rect {
                            x: pp.left as f64,
                            y: pp.top as f64,
                            width: (pp.right - pp.left) as f64,
                            height: (pp.bottom - pp.top) as f64,
                        },
                        visible: c.visibility == crate::Visibility::Visible
                            && !self.clip_fully_excludes(cid, pr),
                        reason: None,
                    });
                    prefix.pop();
                }
            }
            if descend {
                self.collect_parts(cid, prefix, out);
            }
            if c.part_key.is_some() {
                prefix.pop();
            }
        }
    }
    pub(super) fn dev_target(&self, args: &Value) -> Result<NodeId> {
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
    pub(super) fn dev_point(&self, id: NodeId) -> Result<space::ClientPhysicalPoint> {
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
    pub(crate) fn dev_command(&mut self, command: &str, args: &Value) -> Result<Value> {
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
                self.focus_and_settle(Some(id)).map_err(platform)?;
                Ok(json!({"focused":args["id"],"modality":"keyboard"}))
            }
            "press-at" => {
                // raw client-space press (DIP coords) — outside-click /
                // light-dismiss testing; same routing as a real LMB
                // down+up at the point
                let coord = |k: &str| -> Result<f32> {
                    let v = args[k]
                        .as_f64()
                        .or_else(|| args[k].as_str().and_then(|s| s.parse().ok()))
                        .ok_or_else(|| Error::new("INVALID_ARGUMENT", format!("{k} required")))?;
                    if !v.is_finite() {
                        return Err(Error::new("INVALID_ARGUMENT", format!("{k} not finite")));
                    }
                    Ok(v as f32)
                };
                let (x, y) = (coord("x")?, coord("y")?);
                let sc = self.peer_ctx.scale.get();
                let p = space::ClientPhysicalPoint(crate::geom::PhysicalPoint {
                    x: sc.to_physical(x),
                    y: sc.to_physical(y),
                });
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
                Ok(json!({"performed":"press-at","x":x,"y":y}))
            }
            "leave" => {
                self.leave().map_err(platform)?;
                Ok(json!({"performed":"leave"}))
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
            "resize" => {
                // programmatic client-size change — SetWindowPos fires the
                // SAME synchronous WM_SIZE chain a drag step does, so perf
                // counters observe the real path (window size is sized on
                // the outer frame; reported client rect may differ by the
                // non-client border)
                let num = |k: &str| -> Result<i32> {
                    let v = args[k]
                        .as_i64()
                        .or_else(|| args[k].as_str().and_then(|s| s.parse().ok()))
                        .ok_or_else(|| Error::new("INVALID_ARGUMENT", format!("{k} required")))?;
                    i32::try_from(v)
                        .map_err(|_| Error::new("INVALID_ARGUMENT", format!("{k} out of range")))
                };
                let (w, h) = (num("width")?, num("height")?);
                if !(64..=16384).contains(&w) || !(64..=16384).contains(&h) {
                    return Err(Error::new(
                        "INVALID_ARGUMENT",
                        "width/height must be 64..=16384 px",
                    ));
                }
                unsafe {
                    SetWindowPos(
                        self.hwnd,
                        None,
                        0,
                        0,
                        w,
                        h,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                    .map_err(platform)?;
                }
                let mut c = RECT::default();
                unsafe {
                    let _ = GetClientRect(self.hwnd, &mut c);
                }
                Ok(json!({"resized":[w,h],"client_px":[c.left,c.top,c.right,c.bottom]}))
            }
            "scroll" => {
                // inject a scroll DELTA into the scroll viewport's own
                // factories — the app still owns the offset; this only
                // drives the same NodeEvent wheel routing produces
                let id = self.dev_target(args)?;
                let dy = args["dy"]
                    .as_f64()
                    .or_else(|| args["dy"].as_str().and_then(|s| s.parse().ok()))
                    .ok_or_else(|| Error::new("INVALID_ARGUMENT", "dy required"))?;
                if !dy.is_finite() {
                    return Err(Error::new("INVALID_ARGUMENT", "dy not finite"));
                }
                let n = self
                    .rt
                    .arena
                    .get(id)
                    .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "stale node"))?;
                if !matches!(&n.data, NodeData::Container { kind, .. } if *kind == crate::node::KIND_SCROLL)
                {
                    return Err(Error::new(
                        "NOT_SUPPORTED",
                        "target is not a scroll viewport",
                    ));
                }
                self.push_input(
                    id,
                    NodeEvent::Scroll(crate::event::ScrollOffset {
                        x: 0.,
                        y: dy as f32,
                    }),
                )
                .map_err(platform)?;
                self.turn().map_err(platform)?;
                Ok(json!({"scrolled":dy,"id":args["id"]}))
            }
            "scroll-into-view" => {
                // the delta needed to bring a descendant into its NEAREST
                // scroll ancestor's viewport inner rect — injected into
                // the scroll node's factories (app-owned offset preserved)
                let id = self.dev_target(args)?;
                let Some(target) = self.rects.get(&id).copied() else {
                    return Err(Error::new("TARGET_NOT_VISIBLE", "no layout"));
                };
                // nearest KIND_SCROLL ancestor via the retained parent chain
                let mut cur = self.rt.arena.get(id).and_then(|n| n.parent);
                let mut scr = None;
                while let Some(slot) = cur {
                    match self.rt.arena.slot(slot) {
                        Some(pn) => {
                            if matches!(&pn.data, NodeData::Container { kind, .. } if *kind == crate::node::KIND_SCROLL)
                            {
                                scr = Some(NodeId {
                                    slot,
                                    generation: self.rt.arena.generation_of(slot),
                                });
                                break;
                            }
                            cur = pn.parent;
                        }
                        None => break,
                    }
                }
                let Some(scr) = scr else {
                    return Ok(json!({"scrolled":0.0,"id":args["id"]}));
                };
                // the viewport clip the scroll node applies = its inner
                // rect — the same value its descendants carry in clips
                let vp = {
                    let sn = self.rt.arena.get(scr).unwrap();
                    let ins = crate::layout::node_insets(
                        scr,
                        sn,
                        &crate::layout::Interaction {
                            dark: self.rt.theme.dark,
                            ..Default::default()
                        },
                    );
                    let r = self.rects.get(&scr).copied().unwrap_or_default();
                    LogicalRect {
                        x: r.x + ins.left.0,
                        y: r.y + ins.top.0,
                        width: (r.width - ins.left.0 - ins.right.0).max(0.),
                        height: (r.height - ins.top.0 - ins.bottom.0).max(0.),
                    }
                };
                let dy = if target.y < vp.y {
                    target.y - vp.y
                } else if target.y + target.height > vp.y + vp.height {
                    target.y + target.height - (vp.y + vp.height)
                } else {
                    0.
                };
                if dy != 0. {
                    self.push_input(
                        scr,
                        NodeEvent::Scroll(crate::event::ScrollOffset { x: 0., y: dy }),
                    )
                    .map_err(platform)?;
                    self.turn().map_err(platform)?;
                }
                Ok(json!({"scrolled":dy,"id":args["id"]}))
            }
            "perf" => Ok(super::RESIZE.snapshot()),
            "perf-reset" => {
                super::RESIZE.reset();
                Ok(json!({"reset":true}))
            }
            "shutdown" => Ok(json!({"shutdown":true})),
            _ => Err(Error::new("NOT_SUPPORTED", command)),
        }
    }
}
