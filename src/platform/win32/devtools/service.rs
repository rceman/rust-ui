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
    pub(crate) fn service_devtools(&mut self) -> UiResult {
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
                if *permit == d::server::Admission::Ready {
                    *permit = d::server::Admission::Executing;
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
    /// the `epoch` probe payload — paint epoch + live transition and
    /// animation counters so motion scheduling is observable end-to-end
    pub(crate) fn handshake_json(&self) -> Result<Value> {
        let s = self.dev_snapshot()?;
        Ok(
            json!({"hwnd":self.hwnd.0 as usize,"pid":std::process::id(),"paint_epoch":self.paint_epoch.get(),"counters":{"native_timer_fires":self.counters.native_timer_fires.load(std::sync::atomic::Ordering::Relaxed),"requested_redraws":self.counters.requested_redraws.load(std::sync::atomic::Ordering::Relaxed),"animation_timer_fires":self.counters.animation_timer_fires.load(std::sync::atomic::Ordering::Relaxed),"animation_callbacks":self.counters.animation_callbacks.load(std::sync::atomic::Ordering::Relaxed)},"transitions":self.trans.borrow().len(),"trans_detail":self.trans_probe(),"reduced":self.rt.sched.reduced,"geist":{"sans":crate::platform::win32::fonts::geist().sans,"mono":crate::platform::win32::fonts::geist().mono},"scale_factor":s.scale_factor,"client_dp":s.client_dp,"client_px":s.client_px,"protocol":d::PROTOCOL}),
        )
    }
    pub(crate) fn dev_idle_state(&self) -> (bool, Value) {
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
    pub(crate) fn complete_devtools_idle(&mut self) {
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
