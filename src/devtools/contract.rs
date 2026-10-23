use super::*;
use serde_json::{Value, json};

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
