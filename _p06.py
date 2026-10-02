p='src/platform/win32/text.rs'; s=open(p,encoding='utf-8').read()

# 1. TimerPool type replaces the bare HashMap — tombstones + monotonic alloc
s=s.replace("""/// Allocate a collision-free win32 timer id — scans forward from the
/// sequence and skips still-live ids; `None` when the namespace is full
/// (capacity is honestly reported to msftedit, never silently dropped).
pub(crate) fn alloc_native_timer(
    pool: &std::sync::Arc<std::sync::Mutex<std::collections::HashMap<usize, (crate::NodeId, u32)>>>,
    seq: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> Option<usize> {
    use super::window::{NATIVE_TIMER_BASE, NATIVE_TIMER_CAP};
    let p = pool.lock().unwrap();
    for _ in 0..NATIVE_TIMER_CAP {
        let n = seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tid = NATIVE_TIMER_BASE + (n % NATIVE_TIMER_CAP);
        if !p.contains_key(&tid) {
            return Some(tid);
        }
    }
    None
}""","""/// THE native-timer authority — win32 id -> live owner, or a tombstone.
/// `Tombstone` exists because KillTimer leaves any already-queued
/// WM_TIMER deliverable (MS contract): an id is NEVER reallocated while
/// tombstoned, so a stale tick resolves to a dead entry and can never be
/// redirected at a different timer's owner.
#[derive(Clone, Copy)]
pub(crate) enum TimerState {
    Live(crate::NodeId, u32),
    Tombstone,
}

pub(crate) struct TimerPool {
    map: std::collections::HashMap<usize, TimerState>,
    /// monotonic allocation cursor — ids are allocated, never reused
    next: usize,
}

impl TimerPool {
    pub(crate) fn new() -> Self {
        TimerPool {
            map: std::collections::HashMap::new(),
            next: 0,
        }
    }

    /// Allocate a collision-free win32 timer id; `None` when the
    /// namespace is full (honestly reported to msftedit).
    pub(crate) fn alloc(&mut self) -> Option<usize> {
        use super::window::{NATIVE_TIMER_BASE, NATIVE_TIMER_CAP};
        for _ in 0..NATIVE_TIMER_CAP {
            let tid = NATIVE_TIMER_BASE + (self.next % NATIVE_TIMER_CAP);
            self.next = self.next.wrapping_add(1);
            if !self.map.contains_key(&tid) {
                return Some(tid);
            }
        }
        None
    }

    pub(crate) fn arm(&mut self, tid: usize, node: crate::NodeId, idtimer: u32) {
        self.map.insert(tid, TimerState::Live(node, idtimer));
    }

    /// Kill: the id becomes a tombstone — a queued stale WM_TIMER still
    /// resolves here and resolves to NO owner.
    pub(crate) fn kill(&mut self, tid: usize) {
        self.map.insert(tid, TimerState::Tombstone);
    }

    /// The live owner of `tid`, if any — tombstones/stale ids are None.
    pub(crate) fn owner(&self, tid: usize) -> Option<(crate::NodeId, u32)> {
        match self.map.get(&tid) {
            Some(TimerState::Live(node, idtimer)) => Some((*node, *idtimer)),
            _ => None,
        }
    }

    /// Currently live win32 ids (tombstones excluded).
    pub(crate) fn live_ids(&self) -> Vec<usize> {
        self.map
            .iter()
            .filter_map(|(t, s)| matches!(s, TimerState::Live(..)).then_some(*t))
            .collect()
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
    }
}""")

# 2. HostShared fields: timer_pool type + drop the external seq
s=s.replace("""    /// THE armed-timer authority — win32 id -> (owner, richedit id);
    /// shared by every peer on the window
    pub timer_pool:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<usize, (crate::NodeId, u32)>>>,
    /// monotonically increasing allocator sequence (skip-live policy)
    pub timer_seq: std::sync::Arc<std::sync::atomic::AtomicUsize>,""","""    /// THE armed-timer authority — shared `TimerPool` (live + tombstones)
    pub timer_pool: std::sync::Arc<std::sync::Mutex<TimerPool>>,""")

# 3. TxSetTimer: same (node,idtimer) re-arms the SAME win32 id — SetTimer
#    on an existing id replaces it (no orphan, no second live timer).
s=s.replace("""    fn TxSetTimer(&self, idtimer: u32, utimeout: u32) -> BOOL {
        let (hwnd, node, pool, seq) = {
            let s = self.s();
            (
                s.host.hwnd,
                s.host.node,
                s.host.timer_pool.clone(),
                s.host.timer_seq.clone(),
            )
        };
        let Some(node) = node else {
            return BOOL(0); // unattached host cannot route a timer
        };
        if hwnd.is_invalid() {
            return BOOL(0);
        }
        let Some(tid) = alloc_native_timer(&pool, &seq) else {
            return BOOL(0); // capacity exhausted — honest failure
        };
        let armed = unsafe {
            windows::Win32::UI::WindowsAndMessaging::SetTimer(Some(hwnd), tid, utimeout, None)
        };
        if armed == 0 {
            pool.lock().unwrap().remove(&tid);
            return BOOL(0);
        }
        pool.lock().unwrap().insert(tid, (node, idtimer));
        self.m().host.armed_timers.insert(idtimer, tid);
        BOOL(1)
    }
    fn TxKillTimer(&self, idtimer: u32) {
        let (hwnd, tid, pool) = {
            let mut s = self.m();
            let Some(tid) = s.host.armed_timers.remove(&idtimer) else {
                return;
            };
            (s.host.hwnd, tid, s.host.timer_pool.clone())
        };
        pool.lock().unwrap().remove(&tid);
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), tid);
        }
    }""","""    fn TxSetTimer(&self, idtimer: u32, utimeout: u32) -> BOOL {
        // re-arming the same logical (node,idtimer) RESETS the existing
        // win32 id — SetTimer on a live id replaces it; allocating a
        // second id would orphan a periodic timer that keeps firing
        let (hwnd, node, pool, existing) = {
            let s = self.s();
            (
                s.host.hwnd,
                s.host.node,
                s.host.timer_pool.clone(),
                s.host.armed_timers.get(&idtimer).copied(),
            )
        };
        let Some(node) = node else {
            return BOOL(0); // unattached host cannot route a timer
        };
        if hwnd.is_invalid() {
            return BOOL(0);
        }
        let tid = match existing {
            Some(tid) => tid,
            None => match pool.lock().unwrap().alloc() {
                Some(tid) => tid,
                None => return BOOL(0), // capacity exhausted — honest failure
            },
        };
        let armed = unsafe {
            windows::Win32::UI::WindowsAndMessaging::SetTimer(Some(hwnd), tid, utimeout, None)
        };
        if armed == 0 {
            if existing.is_none() {
                // never-armed id: tombstone it so a stray tick can't bind
                pool.lock().unwrap().kill(tid);
            }
            return BOOL(0);
        }
        pool.lock().unwrap().arm(tid, node, idtimer);
        self.m().host.armed_timers.insert(idtimer, tid);
        BOOL(1)
    }
    fn TxKillTimer(&self, idtimer: u32) {
        let (hwnd, tid, pool) = {
            let mut s = self.m();
            let Some(tid) = s.host.armed_timers.remove(&idtimer) else {
                return;
            };
            (s.host.hwnd, tid, s.host.timer_pool.clone())
        };
        // tombstone, not erase — a WM_TIMER queued before KillTimer must
        // still resolve to a dead owner
        pool.lock().unwrap().kill(tid);
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), tid);
        }
    }""")

# 4. shared kill-all helper + release() uses it (Drop calls release's path too)
s=s.replace("""    fn release(&mut self) {
        // text services release first (may call back into the host)
        // return the surface's bytes to the aggregate budget before the
        // native objects go — accounting mirrors allocation
        drop(self.tx.take());
        unsafe {
            HostBox::Release(self.host);
        }
        self.host = std::ptr::null_mut();
    }""","""    /// Kill every native timer this host armed — used by release() AND
    /// Drop; an orphan periodic timer must not outlive its peer.
    fn kill_armed_timers(&mut self) {
        if self.host.is_null() {
            return;
        }
        let (hwnd, tids, pool) = {
            let mut s = self.shared_mut();
            let t: Vec<usize> = s.host.armed_timers.values().copied().collect();
            s.host.armed_timers.clear();
            (s.host.hwnd, t, s.host.timer_pool.clone())
        };
        for tid in tids {
            pool.lock().unwrap().kill(tid);
            if !hwnd.is_invalid() {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), tid);
                }
            }
        }
    }

    fn release(&mut self) {
        // text services release first (may call back into the host)
        // return the surface's bytes to the aggregate budget before the
        // native objects go — accounting mirrors allocation
        drop(self.tx.take());
        self.kill_armed_timers();
        unsafe {
            HostBox::Release(self.host);
        }
        self.host = std::ptr::null_mut();
    }""")

s=s.replace("""        if !self.host.is_null() {
            drop(self.tx.take());
            // kill every native timer this host armed — a dead peer can
            // never receive WM_TIMER, so its ids must not stay live
            let (hwnd, tids, pool) = {
                let mut s = self.shared_mut();
                let t: Vec<usize> = s.host.armed_timers.values().copied().collect();
                s.host.armed_timers.clear();
                (s.host.hwnd, t, s.host.timer_pool.clone())
            };
            for tid in tids {
                pool.lock().unwrap().remove(&tid);
                if !hwnd.is_invalid() {
                    unsafe {
                        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), tid);
                    }
                }
            }
            unsafe {
                HostBox::Release(self.host);
            }
        }""","""        if !self.host.is_null() {
            drop(self.tx.take());
            // a dead peer can never receive WM_TIMER — tombstone its ids
            self.kill_armed_timers();
            unsafe {
                HostBox::Release(self.host);
            }
        }""")

# 5. HostShared init + arg shape — timer_seq gone
s=s.replace("""        timer_pool: std::sync::Arc<
            std::sync::Mutex<std::collections::HashMap<usize, (crate::NodeId, u32)>>,
        >,""","""        timer_pool: std::sync::Arc<std::sync::Mutex<TimerPool>>,""")
s=s.replace("""        timer_seq: std::sync::Arc<std::sync::atomic::AtomicUsize>,
""","")
s=s.replace("""                timer_seq: timer_seq.clone(),""","""""")
open(p,'w',encoding='utf-8').write(s)

# ---- mod.rs ----
p='src/platform/win32/mod.rs'; s=open(p,encoding='utf-8').read()
s=s.replace("""    /// richedit timer id). The host writes on TxSetTimer/TxKillTimer; the
    /// backend routes WM_TIMER through it — ONE armed-timer authority.
    pub timer_pool: Arc<Mutex<HashMap<usize, (NodeId, u32)>>>,""","""    /// richedit timer id). The host writes on TxSetTimer/TxKillTimer; the
    /// backend routes WM_TIMER through it — ONE armed-timer authority.
    pub timer_pool: Arc<Mutex<crate::platform::win32::text::TimerPool>>,""")
s=s.replace("""    pub(crate) fn native_timer_fire(&mut self, tid: usize) -> UiResult {
        let Some((node, nid)) = self.peer_ctx.timer_pool.lock().unwrap().get(&tid).copied() else {
            return Ok(()); // stale/unowned timer id — reject
        };""","""    pub(crate) fn native_timer_fire(&mut self, tid: usize) -> UiResult {
        let Some((node, nid)) = self.peer_ctx.timer_pool.lock().unwrap().owner(tid) else {
            return Ok(()); // stale/tombstoned timer id — reject
        };""")
s=s.replace("""            (Some(_), Some(n)) => {
                let due = n
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32;
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), DEADLINE_ID);
                    SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None);
                }
                self.deadline_timer = Some(n);
            }""","""            (Some(_), Some(n)) => {
                let due = n
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32;
                let armed = unsafe {
                    let _ = KillTimer(Some(self.hwnd), DEADLINE_ID);
                    SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None) != 0
                };
                // armed state mirrors native truth — a failed SetTimer
                // leaves deadline_timer None so the next turn retries
                self.deadline_timer = armed.then_some(n);
            }""")
s=s.replace("""            (None, Some(n)) => {
                let due = n
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32;
                unsafe {
                    SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None);
                }
                self.deadline_timer = Some(n);
            }""","""            (None, Some(n)) => {
                let due = n
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u32;
                let armed = unsafe {
                    SetTimer(Some(self.hwnd), DEADLINE_ID, due.max(1), None) != 0
                };
                self.deadline_timer = armed.then_some(n);
            }""")
s=s.replace("""        {
            let mut pool = self.peer_ctx.timer_pool.lock().unwrap();
            for tid in pool.keys().copied().collect::<Vec<_>>() {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), tid);
                }
            }
            pool.clear();
        }""","""        {
            let mut pool = self.peer_ctx.timer_pool.lock().unwrap();
            for tid in pool.live_ids() {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), tid);
                }
            }
            pool.clear();
        }""")
s=s.replace("""        timer_pool: Arc::new(Mutex::new(HashMap::new())),""","""        timer_pool: Arc::new(Mutex::new(crate::platform::win32::text::TimerPool::new())),""")
# PeerContext init for timer_seq?
open(p,'w',encoding='utf-8').write(s)
print("ok")
