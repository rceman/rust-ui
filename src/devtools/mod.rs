//! Opt-in development protocol and deterministic comparison helpers.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
pub const PROTOCOL: &str = "rust-ui.devctl/0.1";
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;
pub const MAX_PIXELS: usize = 32 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: String,
    pub token: String,
    pub id: u64,
    pub command: String,
    pub arguments: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}
impl Error {
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn new(code: &str, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
            details: None,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub protocol: String,
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}
impl Response {
    pub fn from_result(id: u64, r: Result<Value>) -> Self {
        match r {
            Ok(v) => Self {
                protocol: PROTOCOL.into(),
                id,
                ok: true,
                result: Some(v),
                error: None,
            },
            Err(e) => Self {
                protocol: PROTOCOL.into(),
                id,
                ok: false,
                result: None,
                error: Some(e),
            },
        }
    }
}
pub fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(v).map_err(|e| Error::new("PROTOCOL_ERROR", e))?;
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "4 MiB limit"));
    }
    let mut out = (bytes.len() as u32).to_le_bytes().to_vec();
    out.extend(bytes);
    Ok(out)
}
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "4 MiB limit"));
    }
    serde_json::from_slice(bytes).map_err(|e| Error::new("PROTOCOL_ERROR", e))
}
pub fn read_frame<R: Read, T: serde::de::DeserializeOwned>(r: &mut R) -> Result<T> {
    let mut n = [0; 4];
    r.read_exact(&mut n).map_err(ioerr)?;
    let n = u32::from_le_bytes(n) as usize;
    if n > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "frame exceeds 4 MiB"));
    }
    let mut b = vec![0; n];
    r.read_exact(&mut b).map_err(ioerr)?;
    decode(&b)
}
pub fn ioerr(e: impl ToString) -> Error {
    Error::new("IO_ERROR", e)
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|x| x.is_finite())
            && self.width >= 0.
            && self.height >= 0.
    }
    /// Grow by `pad` Dp on each side; the result still snaps through
    /// `snap_rect`, so comparison padding follows the single authority.
    pub fn grow(self, pad: f64) -> Rect {
        Rect {
            x: self.x - pad,
            y: self.y - pad,
            width: self.width + 2. * pad,
            height: self.height + 2. * pad,
        }
    }
}
/// Integer pixel rect on the left/top/right/bottom lattice (shared geom type).
pub type PixelRect = crate::geom::PhysicalRect;
/// Grow an already-snapped pixel rect by `pad` pixels on each side
/// (saturating at the i32 lattice; a degenerate result fails bounds checks).
pub fn grow_px(r: PixelRect, pad: i64) -> PixelRect {
    let sat = |v: i64| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    PixelRect {
        left: sat(i64::from(r.left) - pad),
        top: sat(i64::from(r.top) - pad),
        right: sat(i64::from(r.right) + pad),
        bottom: sat(i64::from(r.bottom) + pad),
    }
}
/// THE devctl coordinate snap: logical Dp rect -> integer pixel rect at
/// `scale` px/Dp. Every edge delegates to `crate::geom::Rect::physical` /
/// `ScaleFactor` (round half away from zero; width = right - left). The
/// f64 -> f32 cast happens exactly once, here; no other devtools/CLI code may
/// round rects. Non-finite/non-positive scale, non-finite or unrepresentable
/// coordinates/edges fail INVALID_ARGUMENT (`try_to_physical` semantics).
pub fn snap_rect(logical: Rect, scale: f64) -> Result<PixelRect> {
    if !scale.is_finite() || scale <= 0. {
        return Err(Error::new(
            "INVALID_ARGUMENT",
            "scale must be positive finite",
        ));
    }
    let s = crate::geom::ScaleFactor::new(scale as f32)
        .ok_or_else(|| Error::new("INVALID_ARGUMENT", "scale unrepresentable"))?;
    let conv = |v: f64| -> Result<f32> {
        if !v.is_finite() || v.abs() > f64::from(f32::MAX) {
            return Err(Error::new(
                "INVALID_ARGUMENT",
                "rect coordinate unrepresentable",
            ));
        }
        Ok(v as f32)
    };
    let r = crate::geom::Rect {
        x: conv(logical.x)?,
        y: conv(logical.y)?,
        width: conv(logical.width)?,
        height: conv(logical.height)?,
    };
    for v in [r.x, r.y, r.x + r.width, r.y + r.height] {
        s.try_to_physical(v)
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "rect edge unrepresentable at scale"))?;
    }
    Ok(r.physical(s))
}
fn px_dims(r: &PixelRect) -> (i64, i64) {
    (
        i64::from(r.right) - i64::from(r.left),
        i64::from(r.bottom) - i64::from(r.top),
    )
}
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
pub const CONTRACT_SCHEMA: &str = "rust-ui.shadcn-reference.contract/0.2";
pub const TIERS: [&str; 3] = ["core", "later", "reference-only"];
/// Faithful read-only adapter for the frozen `contract/0.2`. Capture keys are
/// `"<page>/<state>"` and resolved exactly — an element without that capture
/// contributes nothing, and an empty result fails. Every element produces an
/// `owner` target; every capture part produces an `owner#part` target, hidden
/// parts included (explicit `visible:false`/`reason`, no rect).
pub fn reference_targets(
    v: &Value,
    capture: &str,
) -> Result<BTreeMap<SemanticKey, ExpectedTarget>> {
    if v["$schema"] != CONTRACT_SCHEMA {
        return Err(Error::new("INVALID_ARGUMENT", "reference contract schema"));
    }
    let mut out = BTreeMap::new();
    for el in v["elements"]
        .as_array()
        .ok_or_else(|| Error::new("INVALID_ARGUMENT", "elements missing"))?
    {
        let tier = el["tier"]
            .as_str()
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "element tier"))?;
        if !TIERS.contains(&tier) {
            return Err(Error::new(
                "INVALID_ARGUMENT",
                format!("unknown element tier {tier}"),
            ));
        }
        let Some(cap) = el["captures"].get(capture) else {
            continue;
        };
        let id = el["automation_id"]
            .as_str()
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "reference ID"))?;
        let rect: Rect = serde_json::from_value(cap["rect"].clone()).map_err(ioerr)?;
        if !rect.valid() {
            return Err(Error::new("INVALID_ARGUMENT", "reference geometry"));
        }
        let interaction = cap.get("interaction").cloned();
        let insert = |out: &mut BTreeMap<SemanticKey, ExpectedTarget>,
                      key: SemanticKey,
                      target: ExpectedTarget|
         -> Result<()> {
            if out.insert(key, target).is_some() {
                return Err(Error::new("INVALID_ARGUMENT", "duplicate target"));
            }
            Ok(())
        };
        for (part_key, part) in cap["parts"].as_object().into_iter().flatten() {
            let visible = part["visible"].as_bool().unwrap_or(true);
            let meta = part
                .as_object()
                .map(|m| {
                    m.iter()
                        .filter(|(k, _)| {
                            !matches!(
                                k.as_str(),
                                "x" | "y"
                                    | "width"
                                    | "height"
                                    | "visible"
                                    | "reason"
                                    | "owner_attributed"
                                    | "label_gap"
                            )
                        })
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let target = if visible {
                let relative = Rect {
                    x: part["x"].as_f64().unwrap_or(f64::NAN),
                    y: part["y"].as_f64().unwrap_or(f64::NAN),
                    width: part["width"].as_f64().unwrap_or(f64::NAN),
                    height: part["height"].as_f64().unwrap_or(f64::NAN),
                };
                if !relative.valid() {
                    return Err(Error::new("INVALID_ARGUMENT", "part geometry"));
                }
                ExpectedTarget {
                    tier: Some(tier.into()),
                    capture: Some(capture.into()),
                    rect: Some(Rect {
                        x: rect.x + relative.x,
                        y: rect.y + relative.y,
                        width: relative.width,
                        height: relative.height,
                    }),
                    relative_rect: Some(relative),
                    visible: true,
                    reason: None,
                    owner_attributed: part["owner_attributed"].as_bool().unwrap_or(false),
                    label_gap: part["label_gap"].as_f64(),
                    interaction: interaction.clone(),
                    meta,
                }
            } else {
                ExpectedTarget {
                    tier: Some(tier.into()),
                    capture: Some(capture.into()),
                    rect: None,
                    relative_rect: None,
                    visible: false,
                    reason: part["reason"].as_str().map(str::to_owned),
                    owner_attributed: part["owner_attributed"].as_bool().unwrap_or(false),
                    label_gap: part["label_gap"].as_f64(),
                    interaction: None,
                    meta,
                }
            };
            insert(
                &mut out,
                SemanticKey {
                    owner: id.into(),
                    part: Some(part_key.clone()),
                },
                target,
            )?;
        }
        insert(
            &mut out,
            SemanticKey {
                owner: id.into(),
                part: None,
            },
            ExpectedTarget {
                tier: Some(tier.into()),
                capture: Some(capture.into()),
                rect: Some(rect),
                relative_rect: None,
                visible: true,
                reason: None,
                owner_attributed: false,
                label_gap: None,
                interaction,
                meta: BTreeMap::new(),
            },
        )?;
    }
    if out.is_empty() {
        return Err(Error::new("TARGET_NOT_FOUND", "capture has no elements"));
    }
    Ok(out)
}
/// Targets derived from a snapshot used as a reference (calibration). Nodes
/// and parts map 1:1; there is no tier authority, so `tier` is `None` and
/// every target is in scope for any `--tier`.
pub fn snapshot_targets(s: &Snapshot) -> Result<BTreeMap<SemanticKey, ExpectedTarget>> {
    let nodes = s.index()?;
    let parts = s.part_index()?;
    let target = |rect: Rect, visible: bool, reason: Option<String>| ExpectedTarget {
        tier: None,
        capture: None,
        rect: Some(rect),
        relative_rect: None,
        visible,
        reason,
        owner_attributed: false,
        label_gap: None,
        interaction: None,
        meta: BTreeMap::new(),
    };
    let mut out = BTreeMap::new();
    for (id, (n, _)) in &nodes {
        out.insert(
            SemanticKey {
                owner: id.clone(),
                part: None,
            },
            target(n.rect_dp, n.visible, None),
        );
    }
    for (key, p) in &parts {
        out.insert(key.clone(), target(p.rect_dp, p.visible, p.reason.clone()));
    }
    Ok(out)
}
/// Tier scope for `compare-layout`/`compare-element` `--tier`: `all`, or a
/// comma-separated subset of `TIERS`. Default is `core` (the approved frozen
/// core surface). An explicit `--id` bypasses the filter.
#[derive(Debug, Clone)]
pub struct TierScope {
    pub tiers: Vec<String>,
    pub all: bool,
}
impl TierScope {
    pub fn parse(spec: Option<&str>) -> Result<Self> {
        match spec.unwrap_or("core") {
            "all" => Ok(Self {
                tiers: TIERS.iter().map(|s| s.to_string()).collect(),
                all: true,
            }),
            spec => {
                let tiers: Vec<String> =
                    spec.split(',').map(str::trim).map(str::to_owned).collect();
                if let Some(t) = tiers.iter().find(|t| !TIERS.contains(&t.as_str())) {
                    return Err(Error::new("INVALID_ARGUMENT", format!("unknown tier {t}")));
                }
                Ok(Self { tiers, all: false })
            }
        }
    }
    /// Contract tier `None` (snapshot-derived references) is always in scope.
    pub fn includes(&self, tier: Option<&str>) -> bool {
        match tier {
            None => true,
            Some(t) => self.all || self.tiers.iter().any(|x| x == t),
        }
    }
}
/// One layout row: expected target rect vs a measured native rect. `actual`
/// is `(rect_dp, visible)` of the native element or part.
pub fn layout_row(
    expected: &ExpectedTarget,
    actual: Option<(Rect, bool)>,
    tolerance: f64,
    mode: &str,
) -> Value {
    let Some(want) = expected.rect else {
        // Hidden (not-rendered) expected part: passes iff the native side is
        // absent or not visible; a visible native part is an explicit failure.
        return match actual {
            Some((_, true)) => {
                json!({"expected":"hidden","reason":expected.reason,"actual_visible":true,"pass":false})
            }
            _ => json!({"expected":"hidden","reason":expected.reason,"pass":true}),
        };
    };
    let Some((got, actual_visible)) = actual else {
        return json!({"expected":want,"pass":false,"missing":true});
    };
    let delta = Rect {
        x: got.x - want.x,
        y: got.y - want.y,
        width: got.width - want.width,
        height: got.height - want.height,
    };
    let pass = (expected.visible == actual_visible)
        && delta.width.abs() <= tolerance
        && delta.height.abs() <= tolerance
        && (mode == "size-only" || (delta.x.abs() <= tolerance && delta.y.abs() <= tolerance));
    json!({"expected":want,"actual":got,"delta":delta,"tolerance":tolerance,"pass":pass})
}
pub fn compare_layout(
    expected: &BTreeMap<SemanticKey, ExpectedTarget>,
    actual: &Snapshot,
    tolerance: f64,
    mode: &str,
    id: Option<&str>,
    tiers: &TierScope,
) -> Result<Value> {
    if !tolerance.is_finite() || tolerance < 0. || !matches!(mode, "absolute" | "size-only") {
        return Err(Error::new("INVALID_ARGUMENT", "tolerance/mode"));
    }
    let only = id.map(SemanticKey::parse).transpose()?;
    let nodes = actual.index()?;
    let parts = actual.part_index()?;
    let mut rows = Vec::new();
    let mut missing = 0;
    let mut failed = 0;
    let mut excluded = 0;
    for (key, target) in expected {
        if let Some(only) = &only {
            if only != key {
                continue;
            }
        } else if !tiers.includes(target.tier.as_deref()) {
            excluded += 1;
            continue;
        }
        let actual = match &key.part {
            None => nodes.get(&key.owner).map(|(n, _)| (n.rect_dp, n.visible)),
            Some(_) => parts.get(key).map(|p| (p.rect_dp, p.visible)),
        };
        let mut row = layout_row(target, actual, tolerance, mode);
        row["id"] = json!(key.display());
        if let Some(part) = &key.part {
            row["part"] = json!(part);
        }
        row["tier"] = json!(target.tier);
        if row["pass"] != true {
            if row["missing"] == true {
                missing += 1
            } else {
                failed += 1
            }
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(Error::new("TARGET_NOT_FOUND", "ID absent from expected"));
    }
    let expected_element = |k: &str| {
        expected.get(&SemanticKey {
            owner: k.into(),
            part: None,
        })
    };
    let unexpected: Vec<_> = nodes
        .keys()
        .filter(|k| id.is_none() && expected_element(k).is_none())
        .cloned()
        .collect();
    let out_of_scope: Vec<_> = nodes
        .keys()
        .filter(|k| {
            id.is_none() && expected_element(k).is_some_and(|t| !tiers.includes(t.tier.as_deref()))
        })
        .cloned()
        .collect();
    Ok(
        json!({"mode":mode,"pass":missing==0&&failed==0&&unexpected.is_empty(),"scope":{"tiers":tiers.tiers,"excluded_by_tier":excluded},"matched":rows.len()-missing,"missing":missing,"unexpected":unexpected,"out_of_scope":out_of_scope,"failed":failed,"comparisons":rows}),
    )
}
/// `compare-element` artifacts: the 0.2 report plus the images the CLI
/// persists (`reference.png`, `native-raw.png`, `native-canonical.png`,
/// `diff.png`). `native_raw` is unaltered raw evidence — never normalized.
pub struct ElementArtifacts {
    pub result: Value,
    pub reference_crop: Image,
    pub native_raw: Image,
    pub native_canonical: Image,
    pub diff: Option<Image>,
}
/// Offline element comparison over the two raster spaces.
///
/// RAW_NATIVE_CAPTURE: `native_image` (full WGC client raster, must equal
/// `native_snapshot.client_px`). CANONICAL_COMPARISON_RASTER: 1.0 px/Dp on
/// each element's OWN logical lattice — `snap_rect(rect.grow(pad), 1.0)` —
/// so a position difference between reference and native is not resampled
/// away or mis-sampled; only equal/differing logical sizes change the
/// canonical dimensions. `reference_scale`/`reference_dims` describe the
/// reference raster's space (contract: `device_scale_factor` + the capture
/// page's `document`; snapshot reference: its `scale_factor`/`client_px`).
/// Fail-closed: image/dimension disagreement or a footprint outside a raster
/// errors rather than clamps.
#[allow(clippy::too_many_arguments)]
pub fn compare_element(
    targets: &BTreeMap<SemanticKey, ExpectedTarget>,
    key: &SemanticKey,
    reference_image: &Image,
    reference_scale: f64,
    reference_dims: (f64, f64),
    native_snapshot: &Snapshot,
    native_image: &Image,
    native_key: &SemanticKey,
    tiers: &TierScope,
    pad: f64,
    tolerance: f64,
    mode: &str,
    channel_tolerance: u8,
) -> Result<ElementArtifacts> {
    if !tolerance.is_finite() || tolerance < 0. || !matches!(mode, "absolute" | "size-only") {
        return Err(Error::new("INVALID_ARGUMENT", "tolerance/mode"));
    }
    if !pad.is_finite() || pad < 0. || pad > 256. {
        return Err(Error::new("INVALID_ARGUMENT", "--pad 0..256"));
    }
    let target = targets
        .get(key)
        .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "reference ID"))?;
    let want = target
        .rect
        .filter(|_| target.visible)
        .ok_or_else(|| Error::new("TARGET_NOT_VISIBLE", "hidden reference target"))?;
    let (native_rect, native_visible) = if native_key.part.is_none() {
        let nodes = native_snapshot.index()?;
        let n = nodes
            .get(&native_key.owner)
            .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "native ID"))?
            .0;
        (n.rect_dp, n.visible)
    } else {
        let parts = native_snapshot.part_index()?;
        let p = parts
            .get(native_key)
            .ok_or_else(|| Error::new("TARGET_NOT_FOUND", "native part"))?;
        (p.rect_dp, p.visible)
    };
    if !native_visible {
        return Err(Error::new("TARGET_NOT_VISIBLE", native_key.display()));
    }
    if f64::from(native_image.width) != native_snapshot.client_px.width
        || f64::from(native_image.height) != native_snapshot.client_px.height
    {
        return Err(Error::new(
            "INVALID_ARGUMENT",
            "native-image dimensions must equal snapshot client_px",
        ));
    }
    if f64::from(reference_image.width) != reference_dims.0
        || f64::from(reference_image.height) != reference_dims.1
    {
        return Err(Error::new(
            "INVALID_ARGUMENT",
            "reference-image dimensions must equal the declared reference extent",
        ));
    }
    if !reference_scale.is_finite() || reference_scale <= 0. {
        return Err(Error::new("INVALID_ARGUMENT", "reference scale"));
    }
    let native_scale = native_snapshot.scale_factor;
    // Each side snaps its OWN grown logical rect: the reference canonical
    // lattice and the native canonical lattice are independent; equal logical
    // sizes give equal canonical dimensions, a logical size difference shows
    // as honest missing pixels at the right/bottom — never a scale artifact.
    let grown_ref = want.grow(pad);
    let grown_native = native_rect.grow(pad);
    let ref_canonical = snap_rect(grown_ref, 1.0)?;
    let native_canonical_rect = snap_rect(grown_native, 1.0)?;
    let reference_px = snap_rect(grown_ref, reference_scale)?;
    let native_px = snap_rect(grown_native, native_scale)?;
    let reference_crop = reference_image.crop(&reference_px)?;
    let native_raw = native_image.crop(&native_px)?;
    let native_canonical = native_image.canonicalize(native_scale, &native_canonical_rect)?;
    let reference_canonical = reference_image.canonicalize(reference_scale, &ref_canonical)?;
    let (visual, diff) = compare_image(&reference_canonical, &native_canonical, channel_tolerance)?;
    let mut geometry = layout_row(target, Some((native_rect, native_visible)), tolerance, mode);
    geometry["id"] = json!(key.display());
    geometry["native_id"] = json!(native_key.display());
    geometry["tier"] = json!(target.tier);
    let px = |r: &PixelRect| json!({"left":r.left,"top":r.top,"right":r.right,"bottom":r.bottom});
    let dims = |r: &PixelRect| {
        [
            i64::from(r.right) - i64::from(r.left),
            i64::from(r.bottom) - i64::from(r.top),
        ]
    };
    let visual_raw = if reference_scale == native_scale {
        let (mut v, _) = compare_image(&reference_crop, &native_raw, channel_tolerance)?;
        v["computed"] = json!(true);
        v
    } else {
        json!({"computed":false,"reason":format!("raw native raster is at {native_scale} px/Dp and reference at {reference_scale} px/Dp; cross-scale raw diff is not meaningful")})
    };
    let result = json!({
        "schema":"rust-ui.devctl.compare-element/0.2",
        "id":key.display(),
        "native_id":native_key.display(),
        "pass":geometry["pass"]==true&&visual["pass"]==true,
        "scope":{"tiers":tiers.tiers,"tier_of_target":target.tier},
        "geometry":geometry,
        "raster":{
            "reference":{"file":"reference.png","scale_px_per_dp":reference_scale,"crop_rect_px":px(&reference_px),"canonical_rect_dp":px(&ref_canonical),"dimensions":dims(&reference_px)},
            "native_raw":{"file":"native-raw.png","scale_px_per_dp":native_scale,"crop_rect_px":px(&native_px),"dimensions":dims(&native_px)},
            "native_canonical":{"file":"native-canonical.png","scale_px_per_dp":1.0,"canonical_rect_dp":px(&native_canonical_rect),"dimensions":dims(&native_canonical_rect)}
        },
        "normalization":{
            "source_scale":native_scale,"target_scale":1.0,
            "source_dimensions":dims(&native_px),"canonical_dimensions":dims(&native_canonical_rect),
            "resampler":"area-box/1","identity":native_scale==1.0,
            "transform":{
                "mapping":"source_px = canonical_px * source_scale",
                "canonical_rect_dp":px(&native_canonical_rect),
                "source_footprint_px":{"x":native_scale*f64::from(native_canonical_rect.left),"y":native_scale*f64::from(native_canonical_rect.top),"width":native_scale*(i64::from(native_canonical_rect.right)-i64::from(native_canonical_rect.left)) as f64,"height":native_scale*(i64::from(native_canonical_rect.bottom)-i64::from(native_canonical_rect.top)) as f64}
            }
        },
        "visual_canonical":visual,
        "visual_raw":visual_raw,
        "evidence_classes":{
            "geometry":"measured",
            "whole_component_raster":"measured",
            "flat_fills":"not_measured","borders":"not_measured","radii":"not_measured","icons":"not_measured",
            "text_bounds_baseline":"not_measured","glyph_raster":"not_measured",
            "note":"Chromium and DirectWrite/D2D glyph pixels are not expected to be byte-identical; geometry is contract authority; canonical raster is a visual signal"
        },
        "padding_comparison_px":pad
    });
    Ok(ElementArtifacts {
        result,
        reference_crop,
        native_raw,
        native_canonical,
        diff,
    })
}
#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl Image {
    pub fn load(p: &Path) -> Result<Self> {
        let file = std::fs::File::open(p).map_err(ioerr)?;
        let mut decoder = png::Decoder::new(file);
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        decoder.set_limits(png::Limits {
            bytes: MAX_PIXELS * 8,
        });
        let mut reader = decoder.read_info().map_err(ioerr)?;
        let info = reader.info();
        let count = (info.width as usize)
            .checked_mul(info.height as usize)
            .filter(|n| *n <= MAX_PIXELS)
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "image pixel budget"))?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(ioerr)?;
        let b = &buf[..info.buffer_size()];
        let mut rgba = Vec::with_capacity(count * 4);
        match info.color_type {
            png::ColorType::Rgba => rgba.extend(b),
            png::ColorType::Rgb => {
                for p in b.chunks_exact(3) {
                    rgba.extend([p[0], p[1], p[2], 255])
                }
            }
            png::ColorType::Grayscale => {
                for v in b {
                    rgba.extend([*v, *v, *v, 255])
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for p in b.chunks_exact(2) {
                    rgba.extend([p[0], p[0], p[0], p[1]])
                }
            }
            _ => return Err(Error::new("INVALID_ARGUMENT", "PNG color type")),
        }
        Ok(Self {
            width: info.width,
            height: info.height,
            rgba,
        })
    }
    pub fn validate(&self) -> Result<()> {
        let n = (self.width as usize)
            .checked_mul(self.height as usize)
            .filter(|n| *n > 0 && *n <= MAX_PIXELS)
            .ok_or_else(|| Error::new("INVALID_ARGUMENT", "image extent"))?;
        if self.rgba.len() != n * 4 {
            return Err(Error::new("INVALID_ARGUMENT", "RGBA length"));
        }
        Ok(())
    }
    pub fn save(&self, p: &Path) -> Result<()> {
        self.validate()?;
        atomic_write(p, |file| {
            let mut encoder = png::Encoder::new(file, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .map_err(ioerr)?
                .write_image_data(&self.rgba)
                .map_err(ioerr)
        })
    }
    /// Crop at an INTEGER pixel rect (a `snap_rect`/`grow_px` result). There
    /// is no float->int rounding here: `left/top/right/bottom` are already the
    /// snapped edges. The whole rect must lie inside the image.
    pub fn crop(&self, r: &PixelRect) -> Result<Self> {
        self.validate()?;
        let (l, t) = (i64::from(r.left), i64::from(r.top));
        let (w, h) = px_dims(r);
        if w <= 0
            || h <= 0
            || l < 0
            || t < 0
            || i64::from(r.right) > i64::from(self.width)
            || i64::from(r.bottom) > i64::from(self.height)
        {
            return Err(Error::new("TARGET_NOT_VISIBLE", "crop outside image"));
        }
        let (x, y, w, h) = (l as u32, t as u32, w as u32, h as u32);
        let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
        for yy in y..y + h {
            let start = ((yy * self.width + x) * 4) as usize;
            rgba.extend(&self.rgba[start..start + w as usize * 4]);
        }
        Ok(Self {
            width: w,
            height: h,
            rgba,
        })
    }
    /// `area-box/1`: resample a RAW raster at `source_scale` px/Dp onto the
    /// CANONICAL_COMPARISON_RASTER lattice — one 1.0-px/Dp pixel per logical
    /// Dp of `canonical` (a `snap_rect(.., 1.0)` result). Canonical pixel
    /// (i,j) covers the source box `[s*(C.left+i), s*(C.left+i+1))` x
    /// `[s*(C.top+j), s*(C.top+j+1))`; each channel is the exact
    /// fractional-coverage area average of overlapping source pixels
    /// (separable weights, straight sRGB-encoded RGBA channels, f64
    /// accumulation in row-major order, round half up, clamp 0..255). A
    /// footprint reaching outside the source fails TARGET_NOT_VISIBLE (no
    /// edge clamping/replication). At `source_scale == 1.0` footprints are
    /// exact source pixels and the output is bit-identical to `crop`.
    pub fn canonicalize(&self, source_scale: f64, canonical: &PixelRect) -> Result<Image> {
        self.validate()?;
        if !source_scale.is_finite() || source_scale <= 0. {
            return Err(Error::new("INVALID_ARGUMENT", "source scale"));
        }
        let (w, h) = px_dims(canonical);
        if w <= 0 || h <= 0 || w as u64 * h as u64 > MAX_PIXELS as u64 {
            return Err(Error::new("INVALID_ARGUMENT", "canonical extent"));
        }
        let (w, h) = (w as u32, h as u32);
        let mut rgba = vec![0; w as usize * h as usize * 4];
        for j in 0..i64::from(h) {
            for i in 0..i64::from(w) {
                // s * (C.left + i): one product per edge so non-binary scales
                // cannot fabricate a zero-overlap boundary source pixel.
                let fx0 = source_scale * (i64::from(canonical.left) + i) as f64;
                let fx1 = source_scale * (i64::from(canonical.left) + i + 1) as f64;
                let fy0 = source_scale * (i64::from(canonical.top) + j) as f64;
                let fy1 = source_scale * (i64::from(canonical.top) + j + 1) as f64;
                let (sx0, sx1) = (fx0.floor() as i64, fx1.ceil() as i64);
                let (sy0, sy1) = (fy0.floor() as i64, fy1.ceil() as i64);
                if sx0 < 0 || sy0 < 0 || sx1 > i64::from(self.width) || sy1 > i64::from(self.height)
                {
                    return Err(Error::new(
                        "TARGET_NOT_VISIBLE",
                        "canonical footprint outside raw capture",
                    ));
                }
                let mut acc = [0f64; 4];
                let mut weight = 0f64;
                for sy in sy0..sy1 {
                    let wy = fy1.min((sy + 1) as f64) - fy0.max(sy as f64);
                    for sx in sx0..sx1 {
                        let wx = fx1.min((sx + 1) as f64) - fx0.max(sx as f64);
                        let wpx = wx * wy;
                        if wpx <= 0. {
                            continue;
                        }
                        let base = ((sy as usize * self.width as usize) + sx as usize) * 4;
                        for c in 0..4 {
                            acc[c] += f64::from(self.rgba[base + c]) * wpx;
                        }
                        weight += wpx;
                    }
                }
                if weight <= 0. {
                    return Err(Error::new("TARGET_NOT_VISIBLE", "empty footprint"));
                }
                let out = ((j as usize * w as usize) + i as usize) * 4;
                for c in 0..4 {
                    rgba[out + c] = (acc[c] / weight).round().clamp(0., 255.) as u8;
                }
            }
        }
        Ok(Image {
            width: w,
            height: h,
            rgba,
        })
    }
}
pub fn compare_image(a: &Image, b: &Image, tolerance: u8) -> Result<(Value, Option<Image>)> {
    a.validate()?;
    b.validate()?;
    let w = a.width.max(b.width);
    let h = a.height.max(b.height);
    let total = w as u64 * h as u64;
    if total > MAX_PIXELS as u64 {
        return Err(Error::new(
            "INVALID_ARGUMENT",
            "difference canvas pixel budget",
        ));
    }
    let (mut count, mut missing, mut max, mut sum) = (0u64, 0u64, 0u8, 0u64);
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    let mut heat = Vec::with_capacity(total as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let pixel = |image: &Image| -> Option<[u8; 4]> {
                if x < image.width && y < image.height {
                    let i = (y as usize * image.width as usize + x as usize) * 4;
                    Some(image.rgba[i..i + 4].try_into().unwrap())
                } else {
                    None
                }
            };
            let pa = pixel(a);
            let pb = pixel(b);
            let absent = pa.is_none() || pb.is_none();
            let p = pa.unwrap_or([0; 4]);
            let q = pb.unwrap_or([0; 4]);
            let mut peak = 0;
            for i in 0..4 {
                let delta = p[i].abs_diff(q[i]);
                peak = peak.max(delta);
                sum += u64::from(delta);
            }
            max = max.max(peak);
            if absent {
                missing += 1;
                heat.extend([255, 0, 255, 255]);
            } else {
                heat.extend([peak.saturating_mul(4), 0, 0, 255]);
            }
            if absent || peak > tolerance {
                count += 1;
                bbox = Some(match bbox {
                    None => (x, y, x + 1, y + 1),
                    Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x + 1), b.max(y + 1)),
                });
            }
        }
    }
    let dimensions = a.width == b.width && a.height == b.height;
    Ok((
        json!({"pass":dimensions&&count==0,"dimensions_match":dimensions,"expected":[a.width,a.height],"actual":[b.width,b.height],"dimensions":[w,h],"tolerance":tolerance,"mismatched_pixels":count,"missing_pixels":missing,"pixel_domain":"union canvas; absent samples transparent; missing pixels always mismatch","mismatch_percent":count as f64*100./total as f64,"max_channel_delta":max,"mean_absolute_channel_error":sum as f64/(total*4) as f64,"difference_bbox":bbox.map(|(l,t,r,b)|Rect{x:l as f64,y:t as f64,width:(r-l) as f64,height:(b-t) as f64})}),
        Some(Image {
            width: w,
            height: h,
            rgba: heat,
        }),
    ))
}
pub fn load_json(path: &Path) -> Result<Value> {
    let f = std::fs::File::open(path).map_err(ioerr)?;
    let mut bytes = Vec::new();
    f.take(MAX_MESSAGE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(ioerr)?;
    decode(&bytes)
}
// Publish only complete artifacts. A failed encode/write preserves an existing
// destination and the temporary file is removed on every failure.
fn atomic_write(path: &Path, write: impl FnOnce(std::fs::File) -> Result<()>) -> Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = path.with_extension(format!("devctl-{}-{n}.tmp", std::process::id()));
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(ioerr)?;
    let guard = Temp(temp.clone());
    write(file)?;
    std::fs::rename(&temp, path).map_err(ioerr)?;
    drop(guard);
    Ok(())
}
pub fn write_json(path: &Path, v: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(v).map_err(ioerr)?;
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "JSON output exceeds 4 MiB"));
    }
    atomic_write(path, |mut f| f.write_all(&bytes).map_err(ioerr))
}
#[cfg(windows)]
#[path = "../platform/win32/devctl_client.rs"]
pub mod client;

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Snapshot {
        let r = Rect {
            x: 10.,
            y: 20.,
            width: 30.,
            height: 40.,
        };
        Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            scale_factor: 1.25,
            client_dp: Rect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 100.,
            },
            client_px: Rect {
                x: 0.,
                y: 0.,
                width: 125.,
                height: 125.,
            },
            root: Node {
                automation_id: Some("probe.button".into()),
                kind: "button".into(),
                label: "test \"\\\n\r\t é".into(),
                rect_dp: r,
                rect_px: Rect {
                    x: 13.,
                    y: 25.,
                    width: 37.,
                    height: 50.,
                },
                visible: true,
                enabled: true,
                focused: false,
                focus_visible: false,
                hovered: false,
                pressed: false,
                presented: None,
                selection: None,
                line_height: None,
                line_twips: None,
                parts: vec![],
                children: vec![],
            },
        }
    }
    #[test]
    fn protocol_round_trip_and_json_escaping() {
        let r = Request {
            protocol: PROTOCOL.into(),
            token: "nonce".into(),
            id: 42,
            command: "tree".into(),
            arguments: json!({"s":"\"\\\n\r\t\u{1}日本"}),
        };
        let encoded = encode(&r).unwrap();
        let got: Request = read_frame(&mut &encoded[..]).unwrap();
        assert_eq!(got.arguments, r.arguments);
        assert_eq!(got.id, 42);
    }
    #[test]
    fn oversized_and_malformed_frames_fail() {
        assert_eq!(
            read_frame::<_, Request>(&mut &((MAX_MESSAGE + 1) as u32).to_le_bytes()[..])
                .unwrap_err()
                .code,
            "MESSAGE_TOO_LARGE"
        );
        assert!(decode::<Request>(b"{}").is_err());
        assert!(decode::<Value>(b"{wrong}").is_err());
    }
    #[test]
    fn response_error_code_preserved() {
        let r = Response::from_result(9, Err(Error::new("TARGET_NOT_FOUND", "probe")));
        assert!(!r.ok);
        assert_eq!(r.error.unwrap().code, "TARGET_NOT_FOUND");
        assert!(r.result.is_none());
    }
    #[test]
    fn deterministic_snapshot_and_strict_duplicates() {
        let mut s = fixture();
        assert_eq!(
            serde_json::to_vec(&s).unwrap(),
            serde_json::to_vec(&s.clone()).unwrap()
        );
        let clone = s.root.clone();
        s.root.children.push(clone);
        assert!(s.index().is_err());
        s.root.children.clear();
        s.scale_factor = f64::NAN;
        assert!(s.index().is_err());
    }
    #[test]
    fn layout_self_and_one_dp_mutation() {
        let mut s = fixture();
        let expected = snapshot_targets(&s).unwrap();
        let core = TierScope::parse(None).unwrap();
        assert_eq!(
            compare_layout(&expected, &s, 0., "absolute", None, &core).unwrap()["pass"],
            true
        );
        s.root.rect_dp.x += 1.;
        let result = compare_layout(&expected, &s, 0., "absolute", None, &core).unwrap();
        assert_eq!(result["pass"], false);
        assert_eq!(result["comparisons"][0]["delta"]["x"], 1.);
        assert!(compare_layout(&expected, &s, 0., "absolute", Some("missing"), &core).is_err());
        assert!(compare_layout(&expected, &s, f64::NAN, "absolute", None, &core).is_err());
    }
    #[test]
    fn image_self_and_one_pixel_bbox() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![0; 24],
        };
        let mut b = a.clone();
        assert_eq!(compare_image(&a, &b, 0).unwrap().0["mismatched_pixels"], 0);
        b.rgba[(1 * 3 + 2) * 4] = 255;
        let (v, diff) = compare_image(&a, &b, 0).unwrap();
        assert_eq!(v["mismatched_pixels"], 1);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":2.,"y":1.,"width":1.,"height":1.})
        );
        assert_eq!(v["max_channel_delta"], 255);
        assert_eq!(diff.unwrap().rgba[(1 * 3 + 2) * 4], 255);
    }
    #[test]
    fn crop_visible_bounds_and_padding() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![128; 24],
        };
        let r = snap_rect(
            Rect {
                x: 1.,
                y: 0.,
                width: 2.,
                height: 2.,
            },
            1.0,
        )
        .unwrap();
        let b = a.crop(&r).unwrap();
        assert_eq!((b.width, b.height), (2, 2));
        assert_eq!(r.left, 1);
        // Padding grows the LOGICAL rect before snapping (compare-element),
        // or the snapped px rect directly (screenshot evidence crop).
        let grown = snap_rect(
            Rect {
                x: 1.,
                y: 0.,
                width: 2.,
                height: 2.,
            }
            .grow(1.),
            1.0,
        )
        .unwrap();
        assert_eq!(
            a.crop(&grown).err().unwrap().code,
            "TARGET_NOT_VISIBLE",
            "padded crop outside image must fail closed"
        );
        assert_eq!(
            a.crop(&grow_px(r, 1)).err().unwrap().code,
            "TARGET_NOT_VISIBLE"
        );
    }
    #[test]
    fn png_round_trip() {
        let a = Image {
            width: 3,
            height: 2,
            rgba: vec![128; 24],
        };
        let p = std::env::temp_dir().join(format!("rust-ui-devctl-png-{}.png", std::process::id()));
        a.save(&p).unwrap();
        let b = Image::load(&p).unwrap();
        std::fs::remove_file(p).unwrap();
        assert_eq!(a.rgba, b.rgba);
    }
    #[test]
    fn frozen_reference_adapter_uses_capture_identity() {
        let v = json!({"$schema":"rust-ui.shadcn-reference.contract/0.2","elements":[{"automation_id":"button.default","tier":"core","captures":{"components/default":{"rect":{"x":1,"y":2,"width":30,"height":32},"parts":{}},"components/pressed":{"rect":{"x":1,"y":3,"width":30,"height":32},"parts":{}}}}]});
        let key = SemanticKey::parse("button.default").unwrap();
        assert_eq!(
            reference_targets(&v, "components/pressed").unwrap()[&key]
                .rect
                .unwrap()
                .y,
            3.
        );
        assert!(reference_targets(&v, "missing/default").is_err());
    }
    #[test]
    fn unequal_dimensions_produce_diff_without_resizing() {
        let a = Image {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        };
        let b = Image {
            width: 1,
            height: 2,
            rgba: vec![255; 8],
        };
        let (v, diff) = compare_image(&a, &b, 0).unwrap();
        assert_eq!(v["dimensions_match"], false);
        assert_eq!(v["missing_pixels"], 2);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":1.,"y":0.,"width":1.,"height":2.})
        );
        assert_eq!(diff.unwrap().width, 2);
    }
    #[test]
    fn image_invalid_extent_rejected() {
        assert!(
            compare_image(
                &Image {
                    width: 0,
                    height: 0,
                    rgba: vec![]
                },
                &Image {
                    width: 0,
                    height: 0,
                    rgba: vec![]
                },
                0
            )
            .is_err()
        );
        assert!(
            Image {
                width: 3,
                height: 3,
                rgba: vec![0; 4]
            }
            .validate()
            .is_err()
        );
    }
    // --- D02: single snap authority -------------------------------------
    fn px(r: &PixelRect) -> (i64, i64, i64, i64) {
        (
            i64::from(r.left),
            i64::from(r.top),
            i64::from(r.right),
            i64::from(r.bottom),
        )
    }
    #[test]
    fn snap_rect_edge_semantics() {
        // Integer edges stay exact at every scale.
        let r = Rect {
            x: 10.,
            y: 20.,
            width: 30.,
            height: 40.,
        };
        for s in [1.0, 1.25, 1.5, 2.0] {
            let p = snap_rect(r, s).unwrap();
            assert_eq!(px(&p).0, (10. * s).round() as i64);
            assert_eq!(px(&p).1, (20. * s).round() as i64);
            // Width is right-left of snapped edges, never round(width*s) —
            // x=10,w=30 at s=1.25: 12.5->13 and 50->50 gives 37, not 38.
            assert_eq!(
                px(&p).2 - px(&p).0,
                ((40. * s).round() - (10. * s).round()) as i64
            );
        }
        // Fractional origin/size: each edge rounds half away from zero —
        // the width is never rounded directly.
        let f = snap_rect(
            Rect {
                x: 300.75,
                y: 0.25,
                width: 32.5,
                height: 32.75,
            },
            1.0,
        )
        .unwrap();
        assert_eq!(px(&f), (301, 0, 333, 33));
        // Frozen button.default (y=300.75, h=32) snaps to 301..333 = 32 px.
        let b = snap_rect(
            Rect {
                x: 0.,
                y: 300.75,
                width: 120.,
                height: 32.,
            },
            1.0,
        )
        .unwrap();
        assert_eq!((b.top, b.bottom), (301, 333));
        // Fractional right/bottom edges round independently.
        let e = snap_rect(
            Rect {
                x: 0.4,
                y: 0.6,
                width: 9.1,
                height: 9.9,
            },
            1.0,
        )
        .unwrap();
        assert_eq!(px(&e), (0, 1, 10, 11));
        // Near-zero origin still snaps, but a rect outside the image fails
        // bounds when cropped.
        let small = Image {
            width: 4,
            height: 4,
            rgba: vec![0; 64],
        };
        let neg = snap_rect(
            Rect {
                x: -1.,
                y: 0.,
                width: 2.,
                height: 2.,
            },
            1.0,
        )
        .unwrap();
        assert_eq!(neg.left, -1);
        assert_eq!(small.crop(&neg).err().unwrap().code, "TARGET_NOT_VISIBLE");
        assert_eq!(snap_rect(r, 0.).unwrap_err().code, "INVALID_ARGUMENT");
        assert_eq!(snap_rect(r, f64::NAN).unwrap_err().code, "INVALID_ARGUMENT");
        assert!(
            snap_rect(
                Rect {
                    x: 0.,
                    y: 0.,
                    width: f64::INFINITY,
                    height: 1.
                },
                1.0
            )
            .is_err()
        );
    }
    /// The previous crop policy (floor origin/ceil far edge), kept here only
    /// to audit the frozen contract against the new authority.
    fn old_crop_edges(r: Rect) -> (i64, i64, i64, i64) {
        (
            r.x.floor() as i64,
            r.y.floor() as i64,
            (r.x + r.width).ceil() as i64,
            (r.y + r.height).ceil() as i64,
        )
    }
    #[test]
    fn systemic_frozen_contract_snap_audit() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("reference/shadcn-gallery-v0.1/contract.json");
        let v = load_json(&path).unwrap();
        assert_eq!(v["$schema"], CONTRACT_SCHEMA);
        let mut records = Vec::new();
        for el in v["elements"].as_array().unwrap() {
            for (_k, cap) in el["captures"].as_object().unwrap() {
                let rect: Rect = serde_json::from_value(cap["rect"].clone()).unwrap();
                records.push((rect, true));
                for (_pk, part) in cap["parts"].as_object().into_iter().flatten() {
                    if part["visible"].as_bool().unwrap_or(true) {
                        records.push((
                            Rect {
                                x: rect.x + part["x"].as_f64().unwrap(),
                                y: rect.y + part["y"].as_f64().unwrap(),
                                width: part["width"].as_f64().unwrap(),
                                height: part["height"].as_f64().unwrap(),
                            },
                            false,
                        ));
                    }
                }
            }
        }
        assert_eq!(
            records.iter().filter(|r| r.1).count(),
            581,
            "element-capture records"
        );
        assert_eq!(
            records.iter().filter(|r| !r.1).count(),
            806,
            "visible-part records"
        );
        let mut disagree = 0usize;
        let mut disagree_elements = 0usize;
        for &(r, is_element) in &records {
            let old = old_crop_edges(r);
            let new = px(&snap_rect(r, 1.0).unwrap());
            if old != new {
                disagree += 1;
                if is_element {
                    disagree_elements += 1;
                }
            }
            for s in [1.0, 1.25, 1.5, 2.0] {
                let p = snap_rect(r, s).unwrap();
                // Edge snap spec, restated independently of the impl.
                assert_eq!(i64::from(p.left), (r.x * s).round() as i64);
                assert_eq!(i64::from(p.right), ((r.x + r.width) * s).round() as i64);
                // Canonical lattice (scale 1.0) is the reference-crop space;
                // integer-size-in-Dp rects keep an integer px size only when
                // width*scale is itself integral.
                let c = snap_rect(r, 1.0).unwrap();
                if (r.width * s).fract() == 0. {
                    assert_eq!(i64::from(p.right) - i64::from(p.left), (r.width * s) as i64);
                }
                if (r.height * s).fract() == 0. {
                    assert_eq!(
                        i64::from(p.bottom) - i64::from(p.top),
                        (r.height * s) as i64
                    );
                }
                assert!(c.right >= c.left && c.bottom >= c.top);
            }
        }
        // Measured on this checkout: 421/581 element records disagree with
        // the retired floor/ceil policy (the independent review's figure);
        // including the 806 visible-part records it is 999/1387.
        assert_eq!(disagree_elements, 421, "element old-vs-new disagreements");
        assert_eq!(disagree, 999, "element+part old-vs-new disagreements");
    }
    // --- D01: two raster spaces + area-box/1 ----------------------------
    /// Flat logical regions: `colors` columns of `cell_dp` Dp, `rows_dp` Dp
    /// tall, rasterized at `scale` px/Dp. Region boundaries land on integer
    /// source pixels for the tested scales.
    fn flat_regions(scale: f64, cell_dp: u32, rows_dp: u32, colors: &[[u8; 4]]) -> Image {
        let cell = (cell_dp as f64 * scale).round() as usize;
        let rows = (rows_dp as f64 * scale).round() as usize;
        let w = cell * colors.len();
        let mut rgba = vec![0; w * rows * 4];
        for y in 0..rows {
            for x in 0..w {
                let c = colors[x / cell];
                rgba[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
            }
        }
        Image {
            width: w as u32,
            height: rows as u32,
            rgba,
        }
    }
    #[test]
    fn canonical_identity_determinism_and_calibration() {
        let colors = [
            [200, 0, 0, 255],
            [0, 200, 0, 255],
            [0, 0, 200, 255],
            [200, 200, 200, 255],
        ];
        // Region boundaries at Dp multiples of 4 (s=1.25 -> 5px), 2 (s=1.5
        // -> 3px), 1 (s=2.0 -> 2px) keep canonical footprints inside a single
        // region; canonicalization is then pixel-exact.
        for &(s, cell_dp) in &[(1.25f64, 4u32), (1.5, 2), (2.0, 1)] {
            let reference = flat_regions(1.0, cell_dp, 4, &colors);
            let native = flat_regions(s, cell_dp, 4, &colors);
            let c = PixelRect {
                left: 0,
                top: 0,
                right: (colors.len() as u32 * cell_dp) as i32,
                bottom: 4,
            };
            let canonical = native.canonicalize(s, &c).unwrap();
            assert_eq!((canonical.width, canonical.height), (c.right as u32, 4));
            let (v, _) = compare_image(&reference, &canonical, 0).unwrap();
            assert_eq!(v["missing_pixels"], 0, "no L-border/clamped edge");
            assert_eq!(v["mismatched_pixels"], 0, "s={s} calibration");
            // Deterministic: byte-equal on re-run.
            assert_eq!(canonical.rgba, native.canonicalize(s, &c).unwrap().rgba);
        }
        // s == 1.0 must be BIT-IDENTICAL to a plain crop.
        let img = flat_regions(1.0, 4, 4, &colors);
        let c = PixelRect {
            left: 2,
            top: 1,
            right: 11,
            bottom: 3,
        };
        assert_eq!(
            img.canonicalize(1.0, &c).unwrap().rgba,
            img.crop(&c).unwrap().rgba
        );
        // Same canonical raster -> zero diff; one pixel -> exact diff + bbox.
        let mut altered = img.clone();
        altered.rgba[(2 * 16 + 5) * 4] = 9;
        let canon = img.canonicalize(1.0, &c).unwrap();
        let changed = altered.canonicalize(1.0, &c).unwrap();
        let (v, _) = compare_image(&canon, &changed, 0).unwrap();
        assert_eq!(v["mismatched_pixels"], 1);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":3.,"y":1.,"width":1.,"height":1.})
        );
        // Style-different native, identical logical geometry: the diff bbox
        // is exactly the differing logical region in canonical space.
        let mut different = colors;
        different[1] = [255, 255, 0, 255];
        let styled = flat_regions(1.25, 4, 4, &different);
        let canon = styled
            .canonicalize(
                1.25,
                &PixelRect {
                    left: 0,
                    top: 0,
                    right: 16,
                    bottom: 4,
                },
            )
            .unwrap();
        let (v, _) = compare_image(&img, &canon, 0).unwrap();
        assert_eq!(v["missing_pixels"], 0);
        assert_eq!(v["mismatched_pixels"], 16);
        assert_eq!(
            v["difference_bbox"],
            json!({"x":4.,"y":0.,"width":4.,"height":4.})
        );
        // Footprint outside the raw capture fails closed.
        assert_eq!(
            img.canonicalize(
                1.25,
                &PixelRect {
                    left: 0,
                    top: 0,
                    right: 16,
                    bottom: 4
                }
            )
            .err()
            .unwrap()
            .code,
            "TARGET_NOT_VISIBLE"
        );
    }
    // --- D03: contract adapter, tiers, parts ----------------------------
    #[test]
    fn tier_scope_parse_and_filter() {
        assert_eq!(TierScope::parse(None).unwrap().tiers, ["core"]);
        assert!(TierScope::parse(Some("all")).unwrap().all);
        assert!(
            TierScope::parse(Some("core,later"))
                .unwrap()
                .includes(Some("later"))
        );
        assert!(
            !TierScope::parse(Some("core"))
                .unwrap()
                .includes(Some("later"))
        );
        assert!(TierScope::parse(Some("core")).unwrap().includes(None));
        assert!(TierScope::parse(Some("bogus")).is_err());
        assert!(TierScope::parse(Some("all,core")).is_err());
    }
    fn contract_fixture() -> Value {
        json!({"$schema":CONTRACT_SCHEMA,"elements":[
          {"automation_id":"checkbox.default.unchecked","tier":"core",
           "captures":{"forms/default":{"rect":{"x":204,"y":1265.5,"width":16,"height":16},
             "parts":{"label":{"x":24,"y":1,"width":71.984,"height":14,
               "owner_attributed":true,"label_gap":8,"color":"#000"},
                      "ghost":{"visible":false,"reason":"not-rendered"}},
             "interaction":{"focused":true,"selection":[0,6]}}}},
          {"automation_id":"later.widget","tier":"later",
           "captures":{"forms/default":{"rect":{"x":0,"y":0,"width":10,"height":10},"parts":{}}}},
          {"automation_id":"other.widget","tier":"later",
           "captures":{"forms/default":{"rect":{"x":0,"y":20,"width":10,"height":10},"parts":{}}}}
        ]})
    }
    #[test]
    fn adapter_parts_tiers_hidden_and_absolute_rects() {
        let t = reference_targets(&contract_fixture(), "forms/default").unwrap();
        // Element + two parts of the checkbox + two later elements.
        assert_eq!(t.len(), 5);
        let label = &t[&SemanticKey::parse("checkbox.default.unchecked#label").unwrap()];
        // Part x/y are owner-relative; absolute rect derives from the owner.
        assert_eq!(label.rect.unwrap().x, 228.);
        assert_eq!(label.relative_rect.unwrap().x, 24.);
        assert!(label.owner_attributed);
        assert_eq!(label.label_gap, Some(8.));
        let ghost = &t[&SemanticKey::parse("checkbox.default.unchecked#ghost").unwrap()];
        assert!(!ghost.visible && ghost.rect.is_none());
        assert_eq!(ghost.reason.as_deref(), Some("not-rendered"));
        let el = &t[&SemanticKey::parse("checkbox.default.unchecked").unwrap()];
        assert_eq!(
            el.interaction,
            Some(json!({"focused":true,"selection":[0,6]}))
        );
        // Wrong capture key: exact lookup, no fallback.
        assert_eq!(
            reference_targets(&contract_fixture(), "forms/pressed")
                .unwrap_err()
                .code,
            "TARGET_NOT_FOUND"
        );
        // Unknown tier fails closed.
        let mut bad = contract_fixture();
        bad["elements"][0]["tier"] = json!("bogus");
        assert_eq!(
            reference_targets(&bad, "forms/default").unwrap_err().code,
            "INVALID_ARGUMENT"
        );
    }
    #[test]
    fn compare_layout_parts_hidden_and_out_of_scope() {
        let targets = reference_targets(&contract_fixture(), "forms/default").unwrap();
        // Native: checkbox owner only, no parts; later.widget present;
        // other.widget absent from contract perspective is fine — it IS in
        // the expected map (later tier, out of scope under default core).
        let mut s = fixture();
        s.root.automation_id = Some("checkbox.default.unchecked".into());
        s.root.rect_dp = Rect {
            x: 204.,
            y: 1265.5,
            width: 16.,
            height: 16.,
        };
        s.root.children = vec![Node {
            automation_id: Some("later.widget".into()),
            kind: "custom".into(),
            label: String::new(),
            rect_dp: Rect {
                x: 0.,
                y: 0.,
                width: 10.,
                height: 10.,
            },
            rect_px: Rect {
                x: 0.,
                y: 0.,
                width: 13.,
                height: 13.,
            },
            visible: true,
            enabled: true,
            focused: false,
            focus_visible: false,
            hovered: false,
            pressed: false,
            presented: None,
            selection: None,
            line_height: None,
            line_twips: None,
            parts: vec![],
            children: vec![],
        }];
        let core = TierScope::parse(None).unwrap();
        let v = compare_layout(&targets, &s, 0., "absolute", None, &core).unwrap();
        // core rows: element + label + ghost; later targets excluded/counted.
        assert_eq!(v["scope"]["excluded_by_tier"], 2);
        assert_eq!(v["out_of_scope"], json!(["later.widget"]));
        assert_eq!(v["unexpected"], json!([]));
        // label is a visible expected part absent natively -> missing.
        assert_eq!(v["missing"], 1);
        // ghost expected hidden and absent natively -> its row passes.
        let ghost_row = v["comparisons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "checkbox.default.unchecked#ghost")
            .unwrap();
        assert_eq!(ghost_row["pass"], true);
        // Hidden expected but rendered natively -> explicit failure.
        s.root.parts = vec![PartNode {
            key: "ghost".into(),
            rect_dp: Rect {
                x: 0.,
                y: 0.,
                width: 4.,
                height: 4.,
            },
            rect_px: Rect {
                x: 0.,
                y: 0.,
                width: 5.,
                height: 5.,
            },
            visible: true,
            reason: None,
        }];
        let v = compare_layout(&targets, &s, 0., "absolute", None, &core).unwrap();
        let ghost_row = v["comparisons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "checkbox.default.unchecked#ghost")
            .unwrap();
        assert_eq!(ghost_row["pass"], false);
        assert_eq!(ghost_row["reason"], "not-rendered");
        // Visible part compares like an element (absolute rect derivation).
        s.root.parts = vec![PartNode {
            key: "label".into(),
            rect_dp: Rect {
                x: 228.,
                y: 1266.5,
                width: 71.984,
                height: 14.,
            },
            rect_px: Rect {
                x: 285.,
                y: 1583.,
                width: 90.,
                height: 18.,
            },
            visible: true,
            reason: None,
        }];
        let v = compare_layout(
            &targets,
            &s,
            0.,
            "absolute",
            Some("checkbox.default.unchecked#label"),
            &core,
        )
        .unwrap();
        assert_eq!(v["pass"], true);
        assert_eq!(v["comparisons"].as_array().unwrap().len(), 1);
        // Explicit --id bypasses the tier filter.
        let v = compare_layout(&targets, &s, 0., "absolute", Some("other.widget"), &core).unwrap();
        assert_eq!(v["missing"], 1);
        // Duplicate (owner,part) rejected.
        s.root.parts.push(PartNode {
            key: "label".into(),
            rect_dp: Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            rect_px: Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            visible: true,
            reason: None,
        });
        assert!(s.part_index().is_err());
        // Snapshot 0.2 strictness: the old schema string is rejected.
        s.schema = "rust-ui.devctl.snapshot-layout/0.1".into();
        assert!(s.index().is_err());
        assert!(snapshot_targets(&s).is_err());
    }
    #[test]
    fn semantic_key_parse_display() {
        let k = SemanticKey::parse("owner#group[a].item[b].icon").unwrap();
        assert_eq!(k.owner, "owner");
        assert_eq!(k.part.as_deref(), Some("group[a].item[b].icon"));
        assert_eq!(k.display(), "owner#group[a].item[b].icon");
        assert!(SemanticKey::parse("owner").unwrap().part.is_none());
        for bad in ["", "#", "owner#", "#part", "a#b#c"] {
            assert!(SemanticKey::parse(bad).is_err(), "{bad}");
        }
    }
    // --- compare_element pipeline (offline, portable) --------------------
    fn target_at(rect: Rect) -> ExpectedTarget {
        ExpectedTarget {
            tier: Some("core".into()),
            capture: None,
            rect: Some(rect),
            relative_rect: None,
            visible: true,
            reason: None,
            owner_attributed: false,
            label_gap: None,
            interaction: None,
            meta: BTreeMap::new(),
        }
    }
    /// `w`x`h` image of `bg` with the element's `rect` (snapped at `scale`)
    /// filled by `colors` in columns of `cell_dp` logical Dp.
    fn element_image(
        w: u32,
        h: u32,
        rect: Rect,
        scale: f64,
        cell_dp: u32,
        colors: &[[u8; 4]],
        bg: [u8; 4],
    ) -> Image {
        let mut rgba = vec![0; w as usize * h as usize * 4];
        for i in 0..w as usize * h as usize {
            rgba[i * 4..i * 4 + 4].copy_from_slice(&bg);
        }
        // Fill the full source-pixel FOOTPRINT extent (floor/ceil of the
        // scaled edges), not just the snapped rect: a fractional logical edge
        // still contributes its flat colour to the boundary source pixels it
        // overlaps, so canonicalization yields the exact flat colour.
        let x0 = (rect.x * scale).floor() as i64;
        let x1 = ((rect.x + rect.width) * scale).ceil() as i64;
        let y0 = (rect.y * scale).floor() as i64;
        let y1 = ((rect.y + rect.height) * scale).ceil() as i64;
        for sy in y0.max(0)..y1.min(i64::from(h)) {
            for sx in x0.max(0)..x1.min(i64::from(w)) {
                let u = ((sx as f64 + 0.5) / scale - rect.x) / f64::from(cell_dp);
                let c = colors[(u.floor() as usize).max(0) % colors.len()];
                let i = ((sy as usize * w as usize) + sx as usize) * 4;
                rgba[i..i + 4].copy_from_slice(&c);
            }
        }
        Image {
            width: w,
            height: h,
            rgba,
        }
    }
    fn native_snapshot(rect: Rect, scale: f64, client_px: (u32, u32)) -> Snapshot {
        let px = snap_rect(rect, scale).unwrap();
        Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            scale_factor: scale,
            client_dp: Rect {
                x: 0.,
                y: 0.,
                width: client_px.0 as f64 / scale,
                height: client_px.1 as f64 / scale,
            },
            client_px: Rect {
                x: 0.,
                y: 0.,
                width: client_px.0 as f64,
                height: client_px.1 as f64,
            },
            root: Node {
                automation_id: Some("probe.el".into()),
                kind: "button".into(),
                label: "probe".into(),
                rect_dp: rect,
                rect_px: Rect {
                    x: px.left as f64,
                    y: px.top as f64,
                    width: (px.right - px.left) as f64,
                    height: (px.bottom - px.top) as f64,
                },
                visible: true,
                enabled: true,
                focused: false,
                focus_visible: false,
                hovered: false,
                pressed: false,
                presented: None,
                selection: None,
                line_height: None,
                line_twips: None,
                parts: vec![],
                children: vec![],
            },
        }
    }
    /// Ten distinct column colours — one per 4-Dp column of a 40-Dp element,
    /// so a per-column mutation lands in exactly one logical region.
    const PALETTE: [[u8; 4]; 10] = [
        [200, 0, 0, 255],
        [0, 200, 0, 255],
        [0, 0, 200, 255],
        [200, 200, 200, 255],
        [90, 30, 30, 255],
        [30, 90, 30, 255],
        [30, 30, 90, 255],
        [150, 120, 30, 255],
        [30, 150, 120, 255],
        [120, 30, 150, 255],
    ];
    #[test]
    fn compare_element_position_independent_canonical() {
        let want = Rect {
            x: 100.,
            y: 50.,
            width: 40.,
            height: 20.,
        };
        let at = Rect {
            x: 24.,
            y: 12.,
            width: 40.,
            height: 20.,
        };
        let mut targets = BTreeMap::new();
        targets.insert(SemanticKey::parse("ref.el").unwrap(), target_at(want));
        let core = TierScope::parse(None).unwrap();
        let key = SemanticKey::parse("ref.el").unwrap();
        let native_key = SemanticKey::parse("probe.el").unwrap();
        // Element content is identical in logical space although it sits at
        // a different position in a differently scaled raster.
        let reference = element_image(160, 100, want, 1.0, 4, &PALETTE, [255; 4]);
        let native_img = element_image(125, 80, at, 1.25, 4, &PALETTE, [255; 4]);
        let snap = native_snapshot(at, 1.25, (125, 80));
        let r = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &snap,
            &native_img,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .unwrap();
        assert_eq!(r.result["visual_canonical"]["missing_pixels"], 0);
        assert_eq!(r.result["visual_canonical"]["mismatched_pixels"], 0);
        assert_eq!(
            r.result["raster"]["native_canonical"]["canonical_rect_dp"],
            json!({"left":24,"top":12,"right":64,"bottom":32})
        );
        assert_eq!(
            r.result["raster"]["reference"]["canonical_rect_dp"],
            json!({"left":100,"top":50,"right":140,"bottom":70})
        );
        // Style-different native, identical logical geometry: the diff is
        // exactly the differing logical region in canonical space.
        let mut different = PALETTE;
        different[1] = [255, 255, 0, 255];
        let styled = element_image(125, 80, at, 1.25, 4, &different, [255; 4]);
        let r = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &snap,
            &styled,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .unwrap();
        assert_eq!(r.result["visual_canonical"]["missing_pixels"], 0);
        assert_eq!(
            r.result["visual_canonical"]["difference_bbox"],
            json!({"x":4.,"y":0.,"width":4.,"height":20.})
        );
        assert_eq!(r.result["visual_canonical"]["mismatched_pixels"], 80);
    }
    #[test]
    fn compare_element_size_delta_is_missing_pixels_not_scale_noise() {
        let want = Rect {
            x: 100.,
            y: 50.,
            width: 40.,
            height: 20.,
        };
        let at = Rect {
            x: 100.,
            y: 50.,
            width: 44.,
            height: 20.,
        };
        let mut targets = BTreeMap::new();
        targets.insert(SemanticKey::parse("ref.el").unwrap(), target_at(want));
        let core = TierScope::parse(None).unwrap();
        let key = SemanticKey::parse("ref.el").unwrap();
        let native_key = SemanticKey::parse("probe.el").unwrap();
        let reference = element_image(160, 100, want, 1.0, 4, &PALETTE, [255; 4]);
        let native_img = element_image(200, 100, at, 1.25, 4, &PALETTE, [255; 4]);
        let snap = native_snapshot(at, 1.25, (200, 100));
        let r = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &snap,
            &native_img,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .unwrap();
        // 44 vs 40 logical width: canonical dims differ by exactly the
        // logical strip; missing pixels are the 4x20 right-hand strip only.
        assert_eq!(
            r.result["raster"]["native_canonical"]["dimensions"],
            json!([44, 20])
        );
        assert_eq!(r.result["visual_canonical"]["missing_pixels"], 80);
        assert_eq!(r.result["visual_canonical"]["mismatched_pixels"], 80);
        assert_eq!(r.result["geometry"]["delta"]["width"], 4.);
        assert_eq!(r.result["geometry"]["pass"], false);
        // A native footprint reaching outside the raw capture fails closed.
        let off = Rect {
            x: 500.,
            y: 0.,
            width: 40.,
            height: 20.,
        };
        let off_snap = native_snapshot(off, 1.25, (200, 100));
        let e = compare_element(
            &targets,
            &key,
            &reference,
            1.0,
            (160., 100.),
            &off_snap,
            &native_img,
            &native_key,
            &core,
            0.,
            0.,
            "absolute",
            0,
        )
        .err()
        .unwrap();
        assert_eq!(e.code, "TARGET_NOT_VISIBLE");
    }
    #[test]
    fn canonicalize_non_binary_scales_have_no_edge_artifacts() {
        // s*(C.left+i) as one product keeps footprints exact at non-binary
        // scales: a flat region resamples to the exact flat colour with no
        // missing pixels and no out-of-raster pixel.
        for &s in &[1.1f64, 1.75] {
            let mut rgba = vec![0; 220 * 220 * 4];
            for p in rgba.chunks_exact_mut(4) {
                p.copy_from_slice(&[77, 77, 77, 255]);
            }
            let img = Image {
                width: 220,
                height: 220,
                rgba,
            };
            let c = snap_rect(
                Rect {
                    x: 10.,
                    y: 10.,
                    width: 40.,
                    height: 20.,
                },
                1.0,
            )
            .unwrap();
            let canonical = img.canonicalize(s, &c).unwrap();
            assert!((canonical.width == 40) && (canonical.height == 20));
            assert!(
                canonical
                    .rgba
                    .chunks_exact(4)
                    .all(|p| p == [77, 77, 77, 255]),
                "s={s} produced a non-flat pixel"
            );
        }
    }
}
