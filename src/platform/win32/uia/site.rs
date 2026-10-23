#[implement(IRawElementProviderWindowlessSite)]
struct WindowlessSite {
    snap: Arc<Snapshot>,
    root: Mutex<Weak<IRawElementProviderFragment>>,
    /// the editor node this site serves — sibling navigation resolves
    /// through the active order relative to it
    node: NodeId,
}

impl IRawElementProviderWindowlessSite_Impl for WindowlessSite_Impl {
    fn GetAdjacentFragment(
        &self,
        direction: NavigateDirection,
    ) -> Result<IRawElementProviderFragment> {
        match direction {
            NavigateDirection_Parent => {
                // parent traversal needs a LIVE node — a removed editor's
                // site stops answering rather than reporting stale parents
                if !self.snap.live_ok(self.node.slot, self.node.generation) {
                    return Err(Error::from_hresult(HRESULT(
                        UIA_E_ELEMENTNOTAVAILABLE as i32,
                    )));
                }
                if let Some(r) = self.root.lock().unwrap().upgrade() {
                    return Ok(r);
                }
            }
            NavigateDirection_NextSibling | NavigateDirection_PreviousSibling => {
                let order = self.snap.order.lock().unwrap();
                if let Some(i) = order.iter().position(|(id, _)| *id == self.node) {
                    let j = if direction == NavigateDirection_NextSibling {
                        i + 1
                    } else {
                        i.checked_sub(1).unwrap_or(usize::MAX)
                    };
                    if let Some((_, e)) = order.get(j)
                        && let Some(f) = e.upgrade()
                    {
                        return Ok(f);
                    }
                }
            }
            _ => {}
        }
        Err(E_NOTIMPL.into())
    }
    fn GetRuntimeIdPrefix(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
}

// ---------------------------------------------------------------------------
// painted fragment — Button/Custom/Label children
// ---------------------------------------------------------------------------

/// The live state every provider reads through — `rebuild` updates it IN
/// PLACE for a surviving NodeId so providers persist across tree changes
/// and property values never go stale between WM_GETOBJECT calls.
struct ChildState {
    automation_id: Mutex<String>,
    name: Mutex<String>,
    rect: Mutex<UiaRect>,
    enabled: std::sync::atomic::AtomicBool,
    actionable: std::sync::atomic::AtomicBool,
    /// the node's published rect ∩ viewport clip was empty — a
    /// scroll-clipped element exists but shows nothing on screen
    offscreen: std::sync::atomic::AtomicBool,
    selected: Mutex<Option<bool>>,
    checked: Mutex<Option<bool>>,
    expanded: Mutex<Option<bool>>,
}

