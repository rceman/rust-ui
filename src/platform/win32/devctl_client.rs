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
fn err(e: impl ToString) -> Error {
    Error::new("IO_ERROR", e)
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Handle(HANDLE);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn event() -> Result<Handle> {
    unsafe {
        CreateEventW(None, true, false, None)
            .map(Handle)
            .map_err(err)
    }
}
// Every pending operation is cancelled AND joined before its OVERLAPPED/buffer
// can disappear. Stop is a kernel event, not an idle timeout/poll.
fn pending(h: HANDLE, ov: &mut OVERLAPPED, stop: HANDLE, ms: u32) -> Result<u32> {
    unsafe {
        let wait = WaitForMultipleObjects(&[ov.hEvent, stop], false, ms);
        let mut n = 0;
        if wait != WAIT_OBJECT_0 {
            let _ = CancelIoEx(h, Some(ov));
            let _ = GetOverlappedResult(h, ov, &mut n, true);
            return Err(Error::new(
                if wait == WAIT_TIMEOUT {
                    "TIMEOUT"
                } else {
                    "SESSION_CLOSED"
                },
                "pipe operation cancelled",
            ));
        }
        GetOverlappedResult(h, ov, &mut n, false).map_err(|e| {
            if e.code() == HRESULT::from_win32(ERROR_BROKEN_PIPE.0)
                || e.code() == HRESULT::from_win32(ERROR_PIPE_NOT_CONNECTED.0)
            {
                Error::new("SESSION_CLOSED", "pipe disconnected")
            } else {
                err(e)
            }
        })?;
        Ok(n)
    }
}
fn transfer(
    h: HANDLE,
    stop: HANDLE,
    bytes: &mut [u8],
    write: bool,
    deadline: Instant,
) -> Result<()> {
    let mut pos = 0;
    while pos < bytes.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Error::new("TIMEOUT", "pipe frame deadline"));
        }
        let ev = event()?;
        let mut ov = OVERLAPPED {
            hEvent: ev.0,
            ..Default::default()
        };
        let mut n = 0;
        let r = unsafe {
            if write {
                WriteFile(h, Some(&bytes[pos..]), Some(&mut n), Some(&mut ov))
            } else {
                ReadFile(h, Some(&mut bytes[pos..]), Some(&mut n), Some(&mut ov))
            }
        };
        if let Err(e) = r {
            if e.code() == HRESULT::from_win32(ERROR_BROKEN_PIPE.0)
                || e.code() == HRESULT::from_win32(ERROR_PIPE_NOT_CONNECTED.0)
            {
                return Err(Error::new("SESSION_CLOSED", "pipe disconnected"));
            }
            if e.code() != HRESULT::from_win32(ERROR_IO_PENDING.0) {
                return Err(err(e));
            }
            n = pending(
                h,
                &mut ov,
                stop,
                remaining.as_millis().min(u32::MAX as u128) as u32,
            )?;
        }
        if n == 0 {
            return Err(Error::new("SESSION_CLOSED", "pipe closed"));
        }
        pos += n as usize;
    }
    Ok(())
}
fn read_until<T: serde::de::DeserializeOwned>(
    h: HANDLE,
    stop: HANDLE,
    deadline: Instant,
) -> Result<T> {
    let mut size = [0u8; 4];
    transfer(h, stop, &mut size, false, deadline)?;
    let n = u32::from_le_bytes(size) as usize;
    if n > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "4 MiB frame limit"));
    }
    let mut bytes = vec![0; n];
    transfer(h, stop, &mut bytes, false, deadline)?;
    D::decode(&bytes)
}
fn read<T: serde::de::DeserializeOwned>(h: HANDLE, stop: HANDLE) -> Result<T> {
    read_until(h, stop, Instant::now() + Duration::from_secs(5))
}
fn write_until<T: Serialize>(h: HANDLE, stop: HANDLE, v: &T, deadline: Instant) -> Result<()> {
    transfer(h, stop, &mut D::encode(v)?, true, deadline)
}
fn write<T: Serialize>(h: HANDLE, stop: HANDLE, v: &T) -> Result<()> {
    transfer(
        h,
        stop,
        &mut D::encode(v)?,
        true,
        Instant::now() + Duration::from_secs(5),
    )
}
// Keep the pipe connected until the client consumes the response and closes.
// DisconnectNamedPipe immediately after WriteFile can discard buffered bytes.
fn finish(h: HANDLE, stop: HANDLE, response: &Response) -> Result<()> {
    if let Err(e) = write(h, stop, response) {
        if e.code == "MESSAGE_TOO_LARGE" {
            write(h, stop, &Response::from_result(response.id, Err(e)))?
        } else {
            return Err(e);
        }
    }
    let mut byte = [0u8; 1];
    match transfer(
        h,
        stop,
        &mut byte,
        false,
        Instant::now() + Duration::from_secs(1),
    ) {
        Err(e) if e.code == "SESSION_CLOSED" => Ok(()),
        Err(e) => Err(e),
        Ok(()) => Err(Error::new(
            "PROTOCOL_ERROR",
            "unexpected bytes after one response",
        )),
    }
}
fn pipe(name: &str) -> Result<Handle> {
    let w = wide(name);
    let h = unsafe {
        CreateNamedPipeW(
            PCWSTR(w.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            65536,
            65536,
            5000,
            None,
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return Err(err(windows::core::Error::from_thread()));
    }
    Ok(Handle(h))
}
fn connect(h: HANDLE, stop: HANDLE) -> Result<()> {
    let ev = event()?;
    let mut ov = OVERLAPPED {
        hEvent: ev.0,
        ..Default::default()
    };
    match unsafe { ConnectNamedPipe(h, Some(&mut ov)) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) => Ok(()),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_IO_PENDING.0) => {
            pending(h, &mut ov, stop, INFINITE).map(|_| ())
        }
        Err(e) => Err(err(e)),
    }
}
pub fn nonce() -> Result<String> {
    let mut bytes = [0u8; 32];
    unsafe {
        BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG)
            .ok()
            .map_err(err)?
    };
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub protocol: String,
    pub session_id: String,
    pub token: String,
    pub pid: u32,
    pub endpoint: String,
    pub window: Value,
}
fn valid_session(s: &Session) -> Result<()> {
    if s.protocol != PROTOCOL {
        return Err(Error::new("PROTOCOL_MISMATCH", "session version"));
    }
    if s.session_id.len() != 64
        || !s.session_id.bytes().all(|b| b.is_ascii_hexdigit())
        || s.endpoint != format!("\\\\.\\pipe\\rust-ui-devctl-{}", s.session_id)
        || (s.token.len() != 64 || !s.token.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(Error::new("INVALID_ARGUMENT", "session identity"));
    }
    Ok(())
}
pub fn request(s: &Session, command: &str, arguments: Value, timeout: u32) -> Result<Value> {
    valid_session(s)?;
    if !(1..=30000).contains(&timeout) {
        return Err(Error::new("INVALID_ARGUMENT", "timeout 1..30000ms"));
    }
    let w = wide(&s.endpoint);
    let start = Instant::now();
    let h = loop {
        match unsafe {
            CreateFileW(
                PCWSTR(w.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                None,
            )
        } {
            Ok(h) => break Handle(h),
            Err(e) => {
                if start.elapsed() >= Duration::from_millis(timeout as u64) {
                    return Err(Error::new("SESSION_NOT_FOUND", e));
                }
                if !matches!(e.code(),x if x==HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)||x==HRESULT::from_win32(ERROR_PIPE_BUSY.0))
                {
                    return Err(err(e));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    let stop = event()?;
    let mut arguments = if arguments.is_null() {
        serde_json::json!({})
    } else {
        arguments
    };
    let object = arguments
        .as_object_mut()
        .ok_or_else(|| Error::new("INVALID_ARGUMENT", "arguments must be object/null"))?;
    object.insert(
        "timeout_ms".into(),
        serde_json::json!(timeout.saturating_sub(250).max(1)),
    );
    let id = 1;
    write_until(
        h.0,
        stop.0,
        &Request {
            protocol: PROTOCOL.into(),
            token: s.token.clone(),
            id,
            command: command.into(),
            arguments,
        },
        start + Duration::from_millis(timeout as u64),
    )?;
    let response: Response =
        read_until(h.0, stop.0, start + Duration::from_millis(timeout as u64))?;
    if response.protocol != PROTOCOL || response.id != id {
        return Err(Error::new("PROTOCOL_MISMATCH", "response identity"));
    }
    if response.ok && response.error.is_none() {
        response
            .result
            .ok_or_else(|| Error::new("PROTOCOL_ERROR", "missing result"))
    } else if !response.ok && response.result.is_none() {
        Err(response
            .error
            .unwrap_or_else(|| Error::new("PROTOCOL_ERROR", "missing error")))
    } else {
        Err(Error::new(
            "PROTOCOL_ERROR",
            "inconsistent response envelope",
        ))
    }
}
// Deliberately inherit NO handles. A launched app must never retain the CLI's
// JSON stdout pipe, otherwise Command::output() callers wait forever for EOF.
struct Launched {
    process: Handle,
    pid: u32,
    published: bool,
}
impl Drop for Launched {
    fn drop(&mut self) {
        if !self.published {
            self.terminate();
        }
    }
}
impl Launched {
    fn terminate(&self) {
        unsafe {
            let _ = TerminateProcess(self.process.0, 1);
            let _ = WaitForSingleObject(self.process.0, 5000);
        }
    }
}
fn quote_arg(s: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in s.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        if c == '"' {
            out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
        } else {
            out.extend(std::iter::repeat_n('\\', slashes));
        }
        slashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}
fn launch_process(exe: &Path, args: &[String], id: &str, token: &str) -> Result<Launched> {
    let path = std::fs::canonicalize(exe).map_err(err)?;
    let application = path
        .to_str()
        .ok_or_else(|| Error::new("INVALID_ARGUMENT", "non-Unicode executable path"))?;
    let mut command = quote_arg(application);
    for arg in args {
        command.push(' ');
        command.push_str(&quote_arg(arg));
    }
    let mut command = wide(&command);
    if command.len() > 32767 {
        return Err(Error::new("INVALID_ARGUMENT", "Windows command-line limit"));
    }
    let mut environment = std::collections::BTreeMap::new();
    for (k, v) in std::env::vars_os() {
        let k = k.to_string_lossy().into_owned();
        let v = v.to_string_lossy().into_owned();
        environment.insert(k.to_uppercase(), (k, v));
    }
    environment.insert(
        "RUST_UI_DEVCTL_SESSION".into(),
        ("RUST_UI_DEVCTL_SESSION".into(), id.into()),
    );
    environment.insert(
        "RUST_UI_DEVCTL_TOKEN".into(),
        ("RUST_UI_DEVCTL_TOKEN".into(), token.into()),
    );
    let mut block = Vec::new();
    for (_, (k, v)) in environment {
        block.extend(wide(&format!("{k}={v}")));
    }
    block.push(0);
    let app = wide(application);
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(app.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_UNICODE_ENVIRONMENT | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
            Some(block.as_ptr() as *const _),
            None,
            &startup,
            &mut info,
        )
        .map_err(err)?;
        let _thread = Handle(info.hThread);
        Ok(Launched {
            process: Handle(info.hProcess),
            pid: info.dwProcessId,
            published: false,
        })
    }
}
pub fn launch(exe: &Path, args: &[String], out: &Path) -> Result<Value> {
    struct ReceiptReservation {
        path: std::path::PathBuf,
        published: bool,
    }
    impl Drop for ReceiptReservation {
        fn drop(&mut self) {
            if !self.published {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
    let reserved = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::new("INVALID_ARGUMENT", "session file already exists")
            } else {
                err(e)
            }
        })?;
    drop(reserved);
    let mut reservation = ReceiptReservation {
        path: out.into(),
        published: false,
    };
    let id = nonce()?;
    let token = nonce()?;
    let endpoint = format!("\\\\.\\pipe\\rust-ui-devctl-{id}");
    let mut child = launch_process(exe, args, &id, &token)?;
    let mut s = Session {
        protocol: PROTOCOL.into(),
        session_id: id,
        token,
        pid: child.pid,
        endpoint,
        window: Value::Null,
    };
    match request(&s, "handshake", Value::Null, 10000) {
        Ok(v) => {
            if v["pid"].as_u64() != Some(s.pid as u64) {
                return Err(Error::new(
                    "PROTOCOL_MISMATCH",
                    "handshake PID does not belong to launched child",
                ));
            }
            s.window = v.clone();
            if let Err(e) = D::write_json(out, &s) {
                return Err(e);
            }
            reservation.published = true;
            child.published = true;
            Ok(
                serde_json::json!({"session_id":s.session_id,"pid":s.pid,"endpoint":s.endpoint,"protocol":PROTOCOL,"session_file":out,"window":v}),
            )
        }
        Err(e) => Err(e),
    }
}
pub fn session(path: &Path) -> Result<Session> {
    let s: Session = serde_json::from_value(D::load_json(path)?).map_err(err)?;
    valid_session(&s)?;
    Ok(s)
}
pub fn cleanup(file: &Path) -> Result<Value> {
    let s = session(file)?;
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, s.pid) };
    match process {
        Ok(h) => {
            let process = Handle(h);
            if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_OBJECT_0 {
                return Err(Error::new("BUSY", "process still live; use shutdown"));
            }
        }
        Err(e) if e.code() == HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) => {}
        Err(e) => return Err(err(e)),
    };
    std::fs::remove_file(file).map_err(err)?;
    Ok(serde_json::json!({"stale_session_removed":true}))
}
pub fn shutdown(s: &Session, file: &Path) -> Result<Value> {
    let process = Handle(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, s.pid) }.map_err(err)?);
    let reply = request(s, "shutdown", Value::Null, 5000)?;
    if unsafe { WaitForSingleObject(process.0, 5000) } != WAIT_OBJECT_0 {
        return Err(Error::new("TIMEOUT", "process did not exit"));
    }
    std::fs::remove_file(file).map_err(err)?;
    Ok(reply)
}
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
