use std::sync::Arc;

use crate::UiResult;
#[cfg(test)]
use crate::runtime::Runtime;
use crate::runtime::UpdateCtx;
use crate::tasks::{Executor, Mailbox, UiProxy};
use crate::ui::Ui;

/// `App::new(state, update, view)` — two functions are the whole contract.
/// `run()` drives the real Windows backend (HWND/message pump); other
/// platforms report `UiError::Unsupported`.
pub struct App<S, M, U, V> {
    pub(crate) state: S,
    _m: std::marker::PhantomData<fn() -> M>,
    pub(crate) update: U,
    pub(crate) view: V,
    pub(crate) title: String,
    pub(crate) executor: Option<Arc<dyn Executor>>,
    /// the window's bounded mailbox, shared by `UiProxy` and task sends
    pub(crate) mailbox: Arc<Mailbox>,
}

impl<S: 'static, M: 'static, U, V> App<S, M, U, V>
where
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    pub fn new(state: S, update: U, view: V) -> Self {
        App {
            state,
            _m: std::marker::PhantomData,
            update,
            view,
            title: String::new(),
            executor: None,
            mailbox: Mailbox::new(),
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Optional executor — required only when `UpdateCtx::spawn` is used.
    pub fn executor(mut self, executor: Arc<dyn Executor>) -> Self {
        self.executor = Some(executor);
        self
    }

    /// External worker bridge — the same bounded mailbox task sends use.
    /// Typed `M: Send` gate lives only on this method.
    pub fn proxy(&self) -> UiProxy<M>
    where
        M: Send,
    {
        UiProxy::new(self.mailbox.clone())
    }

    /// Blocking run: on Windows this is the real HWND/message-pump backend;
    /// other platforms report `Unsupported`.
    pub fn run(self) -> UiResult
    where
        M: 'static,
        U: 'static,
        V: 'static,
    {
        #[cfg(windows)]
        {
            crate::platform::win32::run(self)
        }
        #[cfg(not(windows))]
        {
            Err(crate::UiError::Unsupported(
                "native backend is Windows-only",
            ))
        }
    }
}

/// Core-only constructor: drive a `Runtime` against an injected peer host
/// (tests; the Windows backend will build the same shape internally).
#[cfg(test)]
pub(crate) fn runtime_for<S, M, U, V>(
    app: App<S, M, U, V>,
    peer_factory: Box<
        dyn Fn(crate::node::PeerSpec) -> crate::UiResult<Box<dyn crate::node::TextPeer>>,
    >,
    theme: crate::theme::Theme,
    appearance: crate::theme::Appearance,
) -> Runtime<S, M, U, V>
where
    M: 'static,
    U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
    V: Fn(&S, &mut Ui<'_, '_, M>),
{
    Runtime::new(
        app.state,
        app.update,
        app.view,
        app.executor,
        peer_factory,
        theme,
        appearance,
        app.mailbox,
    )
}
