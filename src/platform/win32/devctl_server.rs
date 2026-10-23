use crate::devtools::{self as D, Error, MAX_MESSAGE, PROTOCOL, Request, Response, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::Path,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::*,
        Security::Cryptography::*,
        Storage::FileSystem::*,
        System::{IO::*, Pipes::*, Threading::*},
    },
    core::*,
};

use super::*;
use crate::devtools::client::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Admission {
    Waking,
    Ready,
    Executing,
    Rejected,
}
pub(crate) struct Envelope {
    pub request: Request,
    pub reply: mpsc::SyncSender<Response>,
    pub deadline: Instant,
    pub admission: Arc<std::sync::Mutex<Admission>>,
}
fn enqueue_request(
    tx: &mpsc::SyncSender<Envelope>,
    mailbox: &crate::tasks::Mailbox,
    envelope: Envelope,
) -> Result<()> {
    let admission = envelope.admission.clone();
    let mut permit = admission.lock().unwrap();
    // UI cannot consume an unconfirmed request while the installed wake seam
    // establishes progress. This permit is request admission, not a second event.
    if tx.try_send(envelope).is_err() {
        *permit = Admission::Rejected;
        return Err(Error::new("BUSY", "request bound"));
    }
    if !mailbox.poke_ui() {
        *permit = Admission::Rejected;
        return Err(Error::new("SESSION_CLOSED", "UI wake failed"));
    }
    *permit = Admission::Ready;
    Ok(())
}
pub(crate) struct Shared {
    pub diagnostics: std::sync::Mutex<Value>,
    pub close_requested: std::sync::atomic::AtomicBool,
    pub fatal: std::sync::Mutex<Option<Error>>,
}
pub(crate) struct Server {
    pub queue: mpsc::Receiver<Envelope>,
    pub idle: Option<Envelope>,
    pub shared: Arc<Shared>,
    stop: Arc<Handle>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub(crate) fn start(mailbox: Arc<crate::tasks::Mailbox>, _hwnd: usize) -> Result<Option<Self>> {
        let Ok(id) = std::env::var("RUST_UI_DEVCTL_SESSION") else {
            return Ok(None);
        };
        let token = std::env::var("RUST_UI_DEVCTL_TOKEN").map_err(err)?;
        let s = Session {
            protocol: PROTOCOL.into(),
            session_id: id.clone(),
            token: token.clone(),
            pid: std::process::id(),
            endpoint: format!("\\\\.\\pipe\\rust-ui-devctl-{id}"),
            window: Value::Null,
        };
        valid_session(&s)?;
        let initial = pipe(&s.endpoint)?;
        let stop = Arc::new(event()?);
        let halted = stop.clone();
        let (tx, queue) = mpsc::sync_channel(16);
        let shared = Arc::new(Shared {
            diagnostics: std::sync::Mutex::new(Value::Null),
            close_requested: std::sync::atomic::AtomicBool::new(false),
            fatal: std::sync::Mutex::new(None),
        });
        let state = shared.clone();
        let thread = std::thread::Builder::new()
            .name("rust-ui-devtools-pipe".into())
            .spawn(move || {
                let mut listener = initial;
                loop {
                    if connect(listener.0, halted.0).is_err() {
                        break;
                    }
                    let mut shutting = false;
                    let response = match read::<Request>(listener.0, halted.0) {
                        Err(e) => Response::from_result(0, Err(e)),
                        Ok(req) => {
                            let id = req.id;
                            let command = req.command.clone();
                            if !req.arguments.is_object() && !req.arguments.is_null() {
                                Response::from_result(
                                    id,
                                    Err(Error::new(
                                        "PROTOCOL_ERROR",
                                        "arguments must be object/null",
                                    )),
                                )
                            } else if req.command.len() > 64 {
                                Response::from_result(
                                    id,
                                    Err(Error::new("INVALID_ARGUMENT", "command length limit")),
                                )
                            } else if req.protocol != PROTOCOL || req.token != token {
                                Response::from_result(
                                    id,
                                    Err(Error::new("PROTOCOL_MISMATCH", "version/token")),
                                )
                            } else {
                                let ms = req
                                    .arguments
                                    .get("timeout_ms")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(4000)
                                    .clamp(1, 30000);
                                let timeout = Duration::from_millis(ms);
                                let (reply, rx) = mpsc::sync_channel(1);
                                let admission = Arc::new(std::sync::Mutex::new(Admission::Waking));
                                let accepted = enqueue_request(
                                    &tx,
                                    &mailbox,
                                    Envelope {
                                        request: req,
                                        reply,
                                        deadline: Instant::now() + timeout,
                                        admission: admission.clone(),
                                    },
                                );
                                if let Err(error) = accepted {
                                    Response::from_result(id, Err(error))
                                } else {
                                    let response = rx.recv_timeout(timeout).unwrap_or_else(|_| {
                                        let mut permit = admission.lock().unwrap();
                                        if *permit == Admission::Ready {
                                            *permit = Admission::Rejected;
                                        }
                                        drop(permit);
                                        Response::from_result(
                                            id,
                                            Err(Error::new(
                                                "TIMEOUT",
                                                "UI quiescence/command deadline",
                                            )
                                            .with_details(
                                                state.diagnostics.lock().unwrap().clone(),
                                            )),
                                        )
                                    });
                                    shutting = command == "shutdown" && response.ok;
                                    response
                                }
                            }
                        }
                    };
                    let delivered = finish(listener.0, halted.0, &response).is_ok();
                    if shutting && delivered {
                        state
                            .close_requested
                            .store(true, std::sync::atomic::Ordering::Release);
                        if !mailbox.poke_ui() {
                            eprintln!("devctl close wake failed: terminal mailbox");
                        }
                        break;
                    }
                    unsafe {
                        let _ = DisconnectNamedPipe(listener.0);
                    }
                    drop(listener);
                    if unsafe { WaitForSingleObject(halted.0, 0) } == WAIT_OBJECT_0 {
                        break;
                    }
                    match pipe(&s.endpoint) {
                        Ok(p) => listener = p,
                        Err(e) => {
                            *state.fatal.lock().unwrap() = Some(e);
                            let _ = mailbox.poke_ui();
                            break;
                        }
                    }
                }
            })
            .map_err(err)?;
        Ok(Some(Self {
            queue,
            idle: None,
            shared,
            stop,
            thread: Some(thread),
        }))
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        unsafe {
            let _ = SetEvent(self.stop.0);
        }
        self.idle.take();
        while let Ok(e) = self.queue.try_recv() {
            let _ = e.reply.send(Response::from_result(
                e.request.id,
                Err(Error::new("SESSION_CLOSED", "shutdown")),
            ));
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
#[path = "devctl_capture.rs"]
mod capture;
pub use capture::{CaptureRuntime, capture_client};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_request_cannot_execute_after_wake_failure() {
        let mailbox = crate::tasks::Mailbox::new();
        mailbox.install_wake(Arc::new(|| false)).unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        let (reply, _) = mpsc::sync_channel(1);
        let permit = Arc::new(std::sync::Mutex::new(Admission::Waking));
        let e = Envelope {
            request: Request {
                protocol: PROTOCOL.into(),
                id: 1,
                token: "test".into(),
                command: "click".into(),
                arguments: Value::Null,
            },
            reply,
            deadline: Instant::now() + Duration::from_secs(1),
            admission: permit.clone(),
        };
        assert_eq!(
            enqueue_request(&tx, &mailbox, e).unwrap_err().code,
            "SESSION_CLOSED"
        );
        assert_eq!(
            *rx.recv().unwrap().admission.lock().unwrap(),
            Admission::Rejected
        );
        assert_eq!(*permit.lock().unwrap(), Admission::Rejected);
    }
    #[test]
    fn session_metadata_strict_local_identity() {
        let id = "1".repeat(64);
        let mut s = Session {
            protocol: PROTOCOL.into(),
            session_id: id.clone(),
            token: "2".repeat(64),
            pid: 1,
            endpoint: format!("\\\\.\\pipe\\rust-ui-devctl-{id}"),
            window: Value::Null,
        };
        valid_session(&s).unwrap();
        s.endpoint = format!("\\\\remote\\pipe\\rust-ui-devctl-{id}");
        assert!(valid_session(&s).is_err());
        s.endpoint = format!("\\\\.\\pipe\\rust-ui-devctl-{id}");
        s.token = "wrong".into();
        assert!(valid_session(&s).is_err());
    }
    #[test]
    fn launch_quoting_is_full_argument_not_shell() {
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg("a\\\"b"), "\"a\\\\\\\"b\"");
        assert_eq!(quote_arg("W:\\folder\\"), "\"W:\\folder\\\\\"");
    }
    #[test]
    fn native_scale_uses_platform_authority() {
        for (dpi, ratio) in [(96, 1.), (120, 1.25), (144, 1.5), (192, 2.)] {
            let s = crate::platform::win32::space::scale_from_dpi(dpi);
            assert_eq!(s.0, ratio);
            assert_eq!(s.to_physical(32.), (32. * ratio) as i32);
        }
    }
    #[test]
    fn native_frame_size_is_rejected_before_payload_allocation() {
        let id = nonce().unwrap();
        let token = nonce().unwrap();
        let s = Session {
            protocol: PROTOCOL.into(),
            session_id: id.clone(),
            token,
            pid: std::process::id(),
            endpoint: format!("\\\\.\\pipe\\rust-ui-devctl-{id}"),
            window: Value::Null,
        };
        let initial = pipe(&s.endpoint).unwrap();
        let stop = Arc::new(event().unwrap());
        let halted = stop.clone();
        let thread = std::thread::spawn(move || {
            let listener = initial;
            connect(listener.0, halted.0).unwrap();
            let error = read::<Request>(listener.0, halted.0).unwrap_err();
            assert_eq!(error.code, "MESSAGE_TOO_LARGE");
            finish(listener.0, halted.0, &Response::from_result(0, Err(error))).unwrap();
        });
        let name = wide(&s.endpoint);
        let h = Handle(
            unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                )
            }
            .unwrap(),
        );
        transfer(
            h.0,
            stop.0,
            &mut ((MAX_MESSAGE + 1) as u32).to_le_bytes(),
            true,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let response: Response = read(h.0, stop.0).unwrap();
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "MESSAGE_TOO_LARGE");
        drop(h);
        thread.join().unwrap();
    }
}
