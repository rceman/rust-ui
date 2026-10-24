/// Alert container props — `variant` selects the frozen palette
/// (`alert.default` / `alert.destructive`), `width` an optional fixed
/// width (the frozen specimen is 320dp), `patch` the surgical override
/// over `alert_recipe`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Alert {
    pub variant: AlertVariant,
    pub width: Option<crate::geom::Dp>,
    /// surgical patch — merges fieldwise within this build
    pub patch: crate::style::BoxStylePatch,
}

impl Alert {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn variant(mut self, v: AlertVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn width(mut self, w: crate::geom::Dp) -> Self {
        self.width = Some(w);
        self
    }
    /// surgical patch — later `.style` calls deep-merge, `None` never erases
    pub fn style(mut self, p: crate::style::BoxStylePatch) -> Self {
        self.patch.merge(&p);
        self
    }
}

/// Skeleton props — a static `bg-muted` placeholder block. `width`/
/// `height` are `Length` specs (the frozen specimen fixes both: avatar
/// 40×40, lines 168×16 and 112×16); `circle` selects the `rounded-full`
/// radius for avatar shapes. No animation — reduced-motion product scope.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    pub width: Length,
    pub height: Length,
    pub circle: bool,
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// fixed-size convenience — `width(Fixed(w)) + height(Fixed(h))`
    pub fn size(mut self, w: crate::geom::Dp, h: crate::geom::Dp) -> Self {
        self.width = Length::Fixed(w);
        self.height = Length::Fixed(h);
        self
    }
    /// `rounded-full` — the avatar shape (radius normalizes to the
    /// half-extent at paint)
    pub fn circle(mut self) -> Self {
        self.circle = true;
        self
    }
}

/// Checkbox props — the app model owns `checked`; `on_change` rides the
/// shared Press path (click + Space/Enter on the focused control).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Checkbox {
    pub checked: bool,
    pub disabled: bool,
    /// semantic `aria-invalid` — destructive border + ring
    pub invalid: bool,
    /// optional visible label painted at the frozen +24dp offset
    pub label: Option<&'static str>,
    /// accessible name when the visible label isn't the control name
    pub accessible_name: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// Switch props — same app-owned `checked` model; `size` selects the
/// frozen 32×18.4 track or the 24×14 `size-sm` track.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Switch {
    pub checked: bool,
    pub disabled: bool,
    pub invalid: bool,
    pub size: crate::theme::ToggleSize,
    pub label: Option<&'static str>,
    pub accessible_name: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// Radio-item options inside `ui.radio_group` — per-item `disabled` /
/// `invalid` overlays; `checked` derives from the group's `selected`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct RadioItemOpts {
    pub disabled: bool,
    pub invalid: bool,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// Field props — `orientation` picks the frozen vertical stack
/// (label/control/description) or the horizontal `content + control`
/// row; `width` pins the 288dp frozen box (unset = fill).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Field {
    pub orientation: crate::theme::FieldOrientation,
    pub width: Option<crate::geom::Dp>,
}

/// `ui.select` props — the app owns `open` + `selected` (key); `value`
/// is the trigger's display text (`None` paints `placeholder` in
/// `data-placeholder` muted-foreground). `automation_id` is REQUIRED —
/// it names the trigger AND is the popup's anchor identity. `width` is
/// the popup lane (frozen 208dp baseline); the trigger itself is w-fit.
#[derive(Clone, Debug, PartialEq)]
pub struct Select<'a> {
    pub automation_id: &'a str,
    /// popup chrome automation id — e.g. `select.default.open.content`
    pub content_id: Option<&'a str>,
    pub open: bool,
    /// key of the selected item — marks `item[key]` with the check
    /// indicator + `selected` semantics (the open focus lands on it)
    pub selected: Option<&'static str>,
    /// display text inside the trigger — `None` paints `placeholder`
    pub value: Option<&'a str>,
    pub placeholder: &'a str,
    pub disabled: bool,
    pub invalid: bool,
    pub size: ControlSize,
    /// popup lane width — `None` = the frozen 208dp baseline
    pub width: Option<crate::geom::Dp>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

impl<'a> Select<'a> {
    /// Minimal live select — `Select::new("select.env")`.
    pub fn new(automation_id: &'a str) -> Self {
        Select {
            automation_id,
            content_id: None,
            open: false,
            selected: None,
            value: None,
            placeholder: "Choose an option",
            disabled: false,
            invalid: false,
            size: ControlSize::default(),
            width: None,
            presented: Presented::default(),
        }
    }
}

/// Select open/selection events delivered through `on_change`.
pub enum SelectChange {
    /// open-state request — trigger press (`!open` at stage time, so a
    /// dismiss+press pair collapses to a close) or popup dismiss (`false`
    /// on outside-press/Escape)
    Open(bool),
    /// an option activated — click or Enter/Space on the live item;
    /// commit `selected`, close, focus returns to the trigger
    Pick(&'static str),
}

/// Per-item options inside `ui.select` content — `disabled` skips nav
/// and activation; `automation_id` gives the item its own element id
/// (specimen stages like `select.default.open.item-highlighted`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SelectItemOpts {
    pub disabled: bool,
    pub automation_id: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

/// `ui.tabs` props — `variant` picks the frozen list chrome, `selected`
/// is the app-owned active tab key.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tabs<'a> {
    pub variant: TabsVariant,
    pub selected: &'a str,
}

/// Per-tab options — `disabled` is inert + skipped by directional nav;
/// `automation_id` gives the trigger its own element id.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct TabOpts {
    pub disabled: bool,
    pub automation_id: Option<&'static str>,
    /// presentation-only staged visual state (`Presented` docs)
    pub presented: Presented,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DropdownMenu<'a> {
    pub automation_id: &'a str,
    pub content_id: Option<&'a str>,
    pub label: &'a str,
    pub open: bool,
    pub width: crate::geom::Dp,
}

impl<'a> DropdownMenu<'a> {
    pub fn new(automation_id: &'a str, label: &'a str) -> Self {
        Self {
            automation_id,
            content_id: None,
            label,
            open: false,
            width: crate::geom::dp(192.0),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DropdownMenuChange {
    Open(bool),
    Activate(&'static str),
    Checkbox(&'static str, bool),
    Radio(&'static str),
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum DropdownItemKind {
    #[default]
    Normal,
    Destructive,
    Checkbox {
        checked: bool,
    },
    Radio {
        selected: bool,
    },
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DropdownItemOpts {
    pub kind: DropdownItemKind,
    pub disabled: bool,
    pub icon: Option<crate::icons::Icon>,
    pub shortcut: Option<&'static str>,
    pub automation_id: Option<&'static str>,
    pub presented: Presented,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SettingsNav<'a> {
    pub automation_id: &'a str,
    pub collapsed: bool,
    pub selected: &'static str,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SettingsNavItemOpts {
    pub disabled: bool,
    pub automation_id: Option<&'static str>,
    pub presented: Presented,
}

// ------------------------- staging internals -------------------------------

/// Flat staged node — children are indices into `Tx.nodes`, so the whole
/// transaction is ONE reusable flat buffer, never a persistent second tree.
pub(crate) struct StagedNode {
    pub automation_id: Option<Rc<str>>,
    /// semantic part key inside the nearest automation-id owner —
    /// `ui.part("header", …)` / `ui.part("line", …)`; devtools emits it
    /// as `owner#part` (with parent part chains resolved)
    pub part_key: Option<Rc<str>>,
    /// semantic control role — set post-stage like `automation_id` by
    /// composite builders (combobox/option/tablist/tab)
    pub role: Option<SemanticRole>,
    pub key: ChildKey,
    pub data: NodeData,
    /// staged presentation-only visual state — builders set it
    /// post-stage like `automation_id` (never grants interactivity)
    pub presented: Presented,
    pub visibility: Visibility,
    pub layout: LayoutSpec,
    pub factories: EventFactorySet,
    /// adapters from enclosing `ui.scope` calls (leaf -> root)
    pub adapters: MsgAdapters,
    /// indices into `Tx.nodes`
    pub children: Vec<u32>,
}
