use super::*;
use serde_json::{Value, json};

/// A semantic part of an automation node (`owner#part`). Rects share the
/// node's client-relative absolute coordinate space; `visible:false` parts
/// carry `reason` and no meaningful rect. The v0.1 native runtime emits no
/// parts (no compound components); synthetic snapshots exercise the mechanism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartNode {
    pub key: String,
    pub rect_dp: Rect,
    pub rect_px: Rect,
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
/// Staged presentation-only visual state (mirrors `crate::node::Presented`)
/// — diagnostic honesty only; `focused`/`hovered`/`pressed` remain the
/// runtime truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presented {
    pub hover: bool,
    pub pressed: bool,
    pub focus_visible: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub automation_id: Option<String>,
    pub kind: String,
    pub label: String,
    pub rect_dp: Rect,
    pub rect_px: Rect,
    pub visible: bool,
    pub enabled: bool,
    pub focused: bool,
    pub focus_visible: bool,
    #[serde(default)]
    pub hovered: bool,
    #[serde(default)]
    pub pressed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presented: Option<Presented>,
    /// staged app-model editor state — diagnostic; not runtime truth
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<(usize, usize)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_twips: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartNode>,
    pub children: Vec<Node>,
}
/// Semantic comparison identity: an element `owner` (automation_id), or a
/// keyed part of it. Display form `owner` / `owner#part` is what `--id` takes;
/// internal maps key the tuple, never a flattened fake automation ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticKey {
    pub owner: String,
    pub part: Option<String>,
}
impl SemanticKey {
    pub fn parse(id: &str) -> Result<Self> {
        if id.is_empty() {
            return Err(Error::new("INVALID_ARGUMENT", "empty target id"));
        }
        match id.split_once('#') {
            None => Ok(Self {
                owner: id.into(),
                part: None,
            }),
            Some((owner, part)) => {
                if owner.is_empty() || part.is_empty() || part.contains('#') {
                    return Err(Error::new(
                        "INVALID_ARGUMENT",
                        "id must be owner or owner#part",
                    ));
                }
                Ok(Self {
                    owner: owner.into(),
                    part: Some(part.into()),
                })
            }
        }
    }
    pub fn display(&self) -> String {
        match &self.part {
            Some(p) => format!("{}#{p}", self.owner),
            None => self.owner.clone(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: String,
    pub scale_factor: f64,
    pub client_dp: Rect,
    pub client_px: Rect,
    pub root: Node,
}
pub const SNAPSHOT_SCHEMA: &str = "rust-ui.devctl.snapshot-layout/0.2";
impl Snapshot {
    pub fn index(&self) -> Result<BTreeMap<String, (&Node, Option<&Node>)>> {
        if self.schema != SNAPSHOT_SCHEMA
            || !self.scale_factor.is_finite()
            || self.scale_factor <= 0.
        {
            return Err(Error::new("INVALID_ARGUMENT", "snapshot schema/scale"));
        }
        let mut out = BTreeMap::new();
        let mut count = 0;
        fn visit<'a>(
            n: &'a Node,
            p: Option<&'a Node>,
            o: &mut BTreeMap<String, (&'a Node, Option<&'a Node>)>,
            depth: usize,
            count: &mut usize,
        ) -> Result<()> {
            *count += 1;
            if depth > 128 || *count > 8192 || !n.rect_dp.valid() || !n.rect_px.valid() {
                return Err(Error::new("INVALID_ARGUMENT", "snapshot bounds"));
            }
            for part in &n.parts {
                if part.key.is_empty() || !part.rect_dp.valid() || !part.rect_px.valid() {
                    return Err(Error::new("INVALID_ARGUMENT", "part key/rect"));
                }
            }
            if let Some(id) = &n.automation_id {
                if o.insert(id.clone(), (n, p)).is_some() {
                    return Err(Error::new("INVALID_ARGUMENT", "duplicate automation ID"));
                }
            } else if !n.parts.is_empty() {
                return Err(Error::new(
                    "INVALID_ARGUMENT",
                    "parts require an automation-id owner",
                ));
            }
            for ch in &n.children {
                visit(ch, Some(n), o, depth + 1, count)?
            }
            Ok(())
        }
        visit(&self.root, None, &mut out, 0, &mut count)?;
        Ok(out)
    }
    /// Keyed part index `(owner, part)` -> part node; duplicate keys fail.
    pub fn part_index(&self) -> Result<BTreeMap<SemanticKey, &PartNode>> {
        let nodes = self.index()?;
        let mut out = BTreeMap::new();
        for (owner, (node, _)) in &nodes {
            for part in &node.parts {
                let key = SemanticKey {
                    owner: owner.clone(),
                    part: Some(part.key.clone()),
                };
                if out.insert(key, part).is_some() {
                    return Err(Error::new("INVALID_ARGUMENT", "duplicate (owner, part)"));
                }
            }
        }
        Ok(out)
    }
}
/// One comparison target from the frozen contract (or a snapshot used as a
/// reference): an element (`part: None`) or a keyed part. `rect` is the
/// absolute PAGE rect in Dp; parts also keep `relative_rect` (the frozen
/// x/y are relative to the owner's border-box origin). Hidden targets carry
/// `visible:false` + `reason` and no rect. `tier` is `None` for
/// snapshot-derived references (no tier authority there).
#[derive(Debug, Clone)]
pub struct ExpectedTarget {
    pub tier: Option<String>,
    pub capture: Option<String>,
    /// Absolute page/client rect in Dp; `None` for hidden (not-rendered) parts.
    pub rect: Option<Rect>,
    /// Part rect relative to the owner element's rect origin (parts only).
    pub relative_rect: Option<Rect>,
    pub visible: bool,
    pub reason: Option<String>,
    pub owner_attributed: bool,
    pub label_gap: Option<f64>,
    /// Capture interaction state (focus/selection) as opaque contract data.
    pub interaction: Option<Value>,
    /// Remaining numeric/string part metadata (e.g. paint fields).
    pub meta: BTreeMap<String, Value>,
}
