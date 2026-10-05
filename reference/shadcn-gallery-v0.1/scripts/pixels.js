// Pixel helpers shared by capture.js (forced-state pixel guard) and
// scripts/selftest.js (fault injection). Pure functions over a decoded
// PNG ({width, height, data: RGBA Uint8Array} from png.js).

// Count pixels that differ between two same-size crops of one image.
// Coordinates are clamped to the image bounds; both rects must be
// the same size (callers compare same-size element twins).
function cropDiffCount(png, a, b) {
  const w = Math.round(Math.min(a.width, b.width));
  const h = Math.round(Math.min(a.height, b.height));
  const ax = Math.max(0, Math.round(a.x)), ay = Math.max(0, Math.round(a.y));
  const bx = Math.max(0, Math.round(b.x)), by = Math.max(0, Math.round(b.y));
  let diff = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const ai = ((ay + y) * png.width + ax + x) * 4;
      const bi = ((by + y) * png.width + bx + x) * 4;
      if (ai < 0 || bi < 0 || ai + 3 >= png.data.length || bi + 3 >= png.data.length) continue;
      if (
        png.data[ai] !== png.data[bi] ||
        png.data[ai + 1] !== png.data[bi + 1] ||
        png.data[ai + 2] !== png.data[bi + 2] ||
        png.data[ai + 3] !== png.data[bi + 3]
      ) diff++;
    }
  }
  return diff;
}

// Forced-state pixel guard: the screenshot crop of a forced-state specimen
// must differ from its normal-state twin. `pad` expands both crops so that
// effects painted OUTSIDE the element's rect are caught too - focus ring
// (box-shadow spreads 3px) and pressed translate (content shifts 1px inside
// an expanded crop). Returns a failure string or null.
function forcedPairError(png, forcedRect, twinRect, label) {
  const pad = 4;
  const a = { x: forcedRect.x - pad, y: forcedRect.y - pad, width: forcedRect.width + pad * 2, height: forcedRect.height + pad * 2 };
  const b = { x: twinRect.x - pad, y: twinRect.y - pad, width: twinRect.width + pad * 2, height: twinRect.height + pad * 2 };
  const diff = cropDiffCount(png, a, b);
  if (diff === 0) return `${label}: forced-state crop identical to twin - forced styles did not paint`;
  return null;
}

module.exports = { cropDiffCount, forcedPairError };
