// reference:selftest - injects deliberately wrong values in memory and
// proves each validator fails. The CI truth is: if a wrong artifact passes,
// the gate is broken; this makes that auditable.
const fs = require("node:fs");
const path = require("node:path");
const {
  validateVersions, validateCoverage, validateBindings, validatePartKeys,
} = require("./validate");

const ROOT = path.join(__dirname, "..");
const j = (f) => JSON.parse(fs.readFileSync(path.join(ROOT, f), "utf8"));
const tokens = j("tokens.json");
const contract = j("contract.json");
const reference = j("reference.json");
const coverage = j("coverage.json");
const bindingsDoc = j("token-bindings.json");

let pass = 0, failed = 0;
const expect = (name, cond) => {
  if (cond) { pass++; console.log(`ok   selftest: ${name}`); }
  else { failed++; console.error(`FAIL selftest: ${name}`); }
};

// --- baseline sanity: unmutated artifacts must pass validators that can run
// without a live DOM (versions, coverage, part-key structure)
expect("versions pass on committed artifacts",
  validateVersions({ reference, contract, coverage, bindings: bindingsDoc }).length === 0);
expect("coverage passes on committed artifacts",
  validateCoverage(contract, coverage).length === 0);

// --- 1. wrong version fails ---
expect("wrong version detected", validateVersions({
  reference: { ...reference, reference_version: "0.2" },
  contract, coverage, bindings: bindingsDoc,
}).length > 0);
expect("missing candidate_revision detected", validateVersions({
  reference: { ...reference, candidate_revision: undefined },
  contract, coverage, bindings: bindingsDoc,
}).length > 0);

// --- 2. missing core state mapping fails coverage ---
const cov2 = JSON.parse(JSON.stringify(coverage));
delete cov2.families.button.states["hover:"];
// also remove any specimen reference so the selector is unmapped
for (const m of Object.values(cov2.families.button.states)) { delete m.specimens; delete m.specimen; }
expect("missing core state mapping detected", validateCoverage(contract, cov2).length > 0);

// --- 3. duplicate rendered part key detected (R03 live enumeration) ---
expect("duplicate part key detected",
  validatePartKeys({ "dialog.default": ["backdrop", "content", "content", "title"] }).length > 0);
expect("one-to-one part key enumeration passes on committed contract",
  validatePartKeys(Object.fromEntries(contract.elements.map((e) => [e.automation_id, Object.keys(e.parts || {})]))).length === 0);

// --- 4. bindings validator catches each injected wrong value ---
// fabricate a "live" map; each case mutates one property
const liveOk = () => ({
  "dialog.default|backdrop|light|": { "background-color": "rgba(0, 0, 0, 0.1)" },
  "button.default||light|": { height: "32px", "padding-left": "10px", "line-height": "20px" },
});
const only = (automation_id, prop, tp) =>
  bindingsDoc.bindings.find(
    (x) => x.automation_id === automation_id && x.css_property === prop && x.theme !== "dark" && x.token_path === tp,
  );

const bdBackdrop = only("dialog.default", "background-color", "translucent.backdrop-black-10.{theme}.alpha");
const bdHeight = only("button.default", "height", "control_heights_px.button.default");
const bdPad = only("button.default", "padding-left", "horizontal_padding_px.button.default.px");
const bdLh = only("button.default", "line-height", "typography.sizes_px.14.line_height");

expect("backdrop alpha=1 detected", validateBindings(tokens, [bdBackdrop], {
  "dialog.default|backdrop|light|": { "background-color": "rgba(0, 0, 0, 1)" },
}).errs.length > 0);
expect("backdrop alpha=0.1 passes", validateBindings(tokens, [bdBackdrop], {
  "dialog.default|backdrop|light|": { "background-color": "rgba(0, 0, 0, 0.1)" },
}).errs.length === 0);
expect("wrong control height detected", validateBindings(tokens, [bdHeight], {
  "button.default||light|": { height: "31px" },
}).errs.length > 0);
expect("wrong padding detected", validateBindings(tokens, [bdPad], {
  "button.default||light|": { "padding-left": "9px" },
}).errs.length > 0);
expect("wrong line-height detected", validateBindings(tokens, [bdLh], {
  "button.default||light|": { "line-height": "19px" },
}).errs.length > 0);
// tolerance sanity: a sub-tolerance color difference must NOT fail
const bdBg = bindingsDoc.bindings.find((x) => x.automation_id === "button.default" && x.css_property === "background-color" && x.token_path.includes("srgb") && x.theme === "light");
expect("1/255 color drift within tolerance", validateBindings(tokens, [bdBg], {
  "button.default||light|": { "background-color": `rgba(24, 24, 24, 1)` },
}).errs.length === 0);
expect("2/255 color drift detected", validateBindings(tokens, [bdBg], {
  "button.default||light|": { "background-color": `rgba(26, 26, 26, 1)` },
}).errs.length > 0);

// --- 5. switch thumb position: checked/unchecked mix-up detected ----------
// the rev-3 defect rendered switch.default.disabled as checked - the thumb
// translate bindings assert position per specimen so the mix-up fails
const bdSwitchTx = bindingsDoc.bindings.find(
  (x) => x.automation_id === "switch.default.checked" && x.css_property === "translate-x" && x.part === "thumb",
);
expect("switch thumb checked/unchecked mix-up detected",
  bdSwitchTx && validateBindings(tokens, [bdSwitchTx], {
    "switch.default.checked|thumb|light|": { "translate-x": "0px" },
    "switch.default.checked|thumb|dark|": { "translate-x": "0px" },
  }).errs.length > 0);

// --- 6. pixel guard: "forcing skipped" injection ---------------------------
// A forced specimen whose pseudo state was never applied paints IDENTICAL to
// its normal twin - simulate that by comparing a twin's crop against itself
// and assert the guard reports a failure.
const { decodePng } = require("./png");
const { forcedPairError } = require("./pixels");
const lightPng = decodePng(fs.readFileSync(path.join(ROOT, "screenshots/light/components.png")));
const rectOf = (id) => {
  const el = contract.elements.find((e) => e.automation_id === id);
  return el.placements.find((p) => p.page === "components");
};
const hoverRect = rectOf("button.default.hover");
const twinRect = rectOf("button.default");
// injection: forcing skipped -> forced specimen paints like its twin ->
// identical crops -> guard must FAIL
expect("skipped forcing detected by pixel guard",
  typeof forcedPairError(lightPng, twinRect, twinRect, "injected-skip") === "string");
// control: the real captured pair differs
expect("real forced specimen differs from twin in light capture",
  forcedPairError(lightPng, hoverRect, twinRect, "button.default.hover") === null);

if (failed) { console.error(`\nselftest: ${failed} failure(s)`); process.exit(1); }
console.log(`\nselftest: ${pass} injection checks all behaved`);
