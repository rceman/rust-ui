// Canonical Chromium launch for the reference capture/check pipeline.
// Every browser session that measures or screenshots the gallery MUST come
// from here so rasterization is reproducible.
//
// --disable-partial-raster (C05): after paint invalidation (CDP
// forcePseudoState + in-page measurement) Chromium can re-raster only the
// invalidated rect into the existing tile; a rounded-rect border at a
// fractional position then gets different AA coverage than a full-tile
// raster (observed: 8 +/-1 px on the settings-nav Select trigger's rounded
// corners, nondeterministically). Disabling partial raster makes every
// raster deterministic.
const { chromium } = require("playwright");

const CANONICAL_ARGS = [
  "--disable-gpu",
  "--force-color-profile=srgb",
  "--disable-lcd-text",
  "--disable-partial-raster",
];

const VIEWPORT = { width: 1440, height: 1000 };

async function launchCanonical() {
  return chromium.launch({ channel: "chromium", args: CANONICAL_ARGS });
}

module.exports = { CANONICAL_ARGS, VIEWPORT, launchCanonical };
