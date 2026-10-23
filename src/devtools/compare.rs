use super::*;
use serde_json::{Value, json};

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
