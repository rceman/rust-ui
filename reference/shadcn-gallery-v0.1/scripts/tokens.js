// Generates tokens.json - the FROZEN VISUAL REFERENCE tokens.
// Derives sRGB hex from the upstream oklch source strings via a
// deterministic OKLCH -> OKLAB -> linear-sRGB -> sRGB conversion
// (gamut-clipped by sRGB component clamp - the neutral palette is
// achromatic except destructive, which is in-gamut).
const fs = require("node:fs");
const path = require("node:path");

// ---- deterministic OKLCH -> sRGB ----
function oklchToSrgb(L, C, H, alpha = 1) {
  const hr = (H * Math.PI) / 180;
  const a = C * Math.cos(hr);
  const b = C * Math.sin(hr);
  // OKLab -> LMS (cubic, then invert matrix)
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541729 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ ** 3, m = m_ ** 3, s = s_ ** 3;
  let R = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
  let G = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
  let B = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
  const clamp = (v) => Math.min(1, Math.max(0, v));
  R = clamp(R); G = clamp(G); B = clamp(B); // gamut clip (documented)
  const toSrgb = (v) =>
    v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055;
  const hex = (v) => Math.round(toSrgb(v) * 255).toString(16).padStart(2, "0");
  return { hex: `#${hex(R)}${hex(G)}${hex(B)}`, alpha };
}

function parseOklch(src) {
  // "oklch(0.205 0 0)" or "oklch(1 0 0 / 10%)"
  const m = src.match(/oklch\(\s*([\d.]+%?)\s+([\d.]+)\s+([\d.]+)(?:\s*\/\s*([\d.]+%?))?\s*\)/);
  if (!m) throw new Error(`not oklch: ${src}`);
  const L = m[1].endsWith("%") ? parseFloat(m[1]) / 100 : parseFloat(m[1]);
  const alpha = m[4] ? (m[4].endsWith("%") ? parseFloat(m[4]) / 100 : parseFloat(m[4])) : 1;
  return oklchToSrgb(L, parseFloat(m[2]), parseFloat(m[3]), alpha);
}

const theme = {
  light: {
    background: "oklch(1 0 0)", foreground: "oklch(0.145 0 0)",
    card: "oklch(1 0 0)", "card-foreground": "oklch(0.145 0 0)",
    popover: "oklch(1 0 0)", "popover-foreground": "oklch(0.145 0 0)",
    primary: "oklch(0.205 0 0)", "primary-foreground": "oklch(0.985 0 0)",
    secondary: "oklch(0.97 0 0)", "secondary-foreground": "oklch(0.205 0 0)",
    muted: "oklch(0.97 0 0)", "muted-foreground": "oklch(0.556 0 0)",
    accent: "oklch(0.97 0 0)", "accent-foreground": "oklch(0.205 0 0)",
    destructive: "oklch(0.577 0.245 27.325)",
    "destructive-foreground": "oklch(0.97 0.01 17)",
    border: "oklch(0.922 0 0)", input: "oklch(0.922 0 0)", ring: "oklch(0.708 0 0)",
    surface: "oklch(0.98 0 0)",
    "code-highlight": "oklch(0.96 0 0)", "code-number": "oklch(0.56 0 0)",
    selection: "oklch(0 0 0)", "selection-foreground": "oklch(1 0 0)",
  },
  dark: {
    background: "oklch(0.145 0 0)", foreground: "oklch(0.985 0 0)",
    card: "oklch(0.205 0 0)", "card-foreground": "oklch(0.985 0 0)",
    popover: "oklch(0.205 0 0)", "popover-foreground": "oklch(0.985 0 0)",
    primary: "oklch(0.922 0 0)", "primary-foreground": "oklch(0.205 0 0)",
    secondary: "oklch(0.269 0 0)", "secondary-foreground": "oklch(0.985 0 0)",
    muted: "oklch(0.269 0 0)", "muted-foreground": "oklch(0.708 0 0)",
    accent: "oklch(0.269 0 0)", "accent-foreground": "oklch(0.985 0 0)",
    destructive: "oklch(0.704 0.191 22.216)",
    "destructive-foreground": "oklch(0.58 0.22 27)",
    border: "oklch(1 0 0 / 10%)", input: "oklch(1 0 0 / 15%)", ring: "oklch(0.556 0 0)",
    surface: "oklch(0.2 0 0)",
    "code-highlight": "oklch(0.27 0 0)", "code-number": "oklch(0.72 0 0)",
    selection: "oklch(0.922 0 0)", "selection-foreground": "oklch(0.205 0 0)",
  },
};

const resolveVar = {
  "surface-foreground": { light: "foreground", dark: "oklch(0.708 0 0)" },
  code: "surface",
  "code-foreground": "surface-foreground",
};

const colors = {};
for (const mode of ["light", "dark"]) {
  colors[mode] = {};
  for (const [k, src] of Object.entries(theme[mode])) {
    const d = parseOklch(src);
    colors[mode][k] = { oklch: src, srgb: d.hex, alpha: d.alpha };
  }
  // var-aliases
  colors[mode]["surface-foreground"] =
    mode === "light"
      ? colors[mode].foreground
      : parseOklchWrapped("oklch(0.708 0 0)");
  colors[mode].code = colors[mode].surface;
  colors[mode]["code-foreground"] = colors[mode]["surface-foreground"];
}
function parseOklchWrapped(src) {
  const d = parseOklch(src);
  return { oklch: src, srgb: d.hex, alpha: d.alpha };
}

// Derived translucent usages actually used by the Nova recipes.
const derived = (name, formula, light, dark) => ({
  name, formula, light, dark,
});
const o = parseOklchWrapped;
const mix = (base, alpha) => ({
  oklch: base.oklch,
  srgb: base.srgb,
  // color-mix(base a%) composes over base alpha - record effective alpha
  alpha: Math.round((base.alpha ?? 1) * alpha * 1000) / 1000,
});
const tokens = {
  "$schema": "rust-ui.shadcn-reference.tokens/0.1",
  header:
    "FROZEN VISUAL REFERENCE tokens - derived from upstream shadcn base-nova/neutral at commit 295a1f1. This is NOT the rust-ui runtime style authority: rust-ui's typed theme API stays canonical; these tokens exist only to compare the native implementation against this reference.",
  derivation:
    "srgb = OKLCH->OKLab->linear-sRGB->sRGB (Bjorn Ottosson matrices), gamut-clipped by component clamp; deterministic in scripts/tokens.js",
  colors,
  translucent: [
    derived("ring-50", "ring color at 50% alpha - focus ring (ring-3 ring-ring/50)", mix(colors.light.ring, 0.5), mix(colors.dark.ring, 0.5)),
    derived("foreground-10", "ring-foreground/10 - card/menu/dialog/popover edge ring", mix(colors.light.foreground, 0.1), mix(colors.dark.foreground, 0.1)),
    derived("backdrop-black-10", "dialog/alert-dialog backdrop bg-black/10", o("oklch(0 0 0)"), o("oklch(0 0 0)")),
    derived("input-30-dark", "dark controls bg-input/30 (input/textarea/select/checkbox/radio/button-outline)", mix(colors.dark.input, 0.3), mix(colors.dark.input, 0.3)),
    derived("input-50", "input/textarea disabled bg-input/50 - dark bg-input/80", mix(colors.light.input, 0.5), mix(colors.dark.input, 0.8)),
    derived("destructive-10-20", "badge/button destructive bg-destructive/10 hover/20 (dark bg-destructive/20 hover/30)", mix(colors.light.destructive, 0.1), mix(colors.dark.destructive, 0.2)),
    derived("destructive-ring-20-40", "aria-invalid ring-destructive/20 (dark /40)", mix(colors.light.destructive, 0.2), mix(colors.dark.destructive, 0.4)),
    derived("primary-80", "button/badge hover bg-primary/80 - secondary hover secondary/80", mix(colors.light.primary, 0.8), mix(colors.dark.primary, 0.8)),
    derived("muted-50", "alert-dialog/footer bg-muted/50 - dark ghost hover bg-muted/50", mix(colors.light.muted, 0.5), mix(colors.dark.muted, 0.5)),
    derived("foreground-60", "inactive tabs trigger text-foreground/60", mix(colors.light.foreground, 0.6), mix(colors.dark.foreground, 0.6)),
    derived("destructive-90", "alert description text-destructive/90", mix(colors.light.destructive, 0.9), mix(colors.dark.destructive, 0.9)),
    derived("secondary-hover-mix", "button secondary hover = color-mix(in oklch, secondary, foreground 5%) - evaluated at capture, recorded as formula", "color-mix(in oklch, secondary, foreground 5%)", "color-mix(in oklch, secondary, foreground 5%)"),
  ],
  typography: {
    families: {
      sans: { css: `"Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif`, source: "reference deviation - upstream Geist -> Segoe UI Variable", optical_sizing: "auto" },
      mono: { css: `"Cascadia Mono", Consolas, monospace`, source: "reference - upstream Geist Mono -> Cascadia Mono" },
    },
    sizes_px: {
      "12": { px: 12, line_height: null, utilities: "text-xs", used_by: "badge, kbd, select-label, menu label, tooltip, button size-xs" },
      "12.8": { px: 12.8, line_height: null, utilities: "text-[0.8rem]", used_by: "button size-sm, toggle size-sm" },
      "14": { px: 14, line_height: 20, utilities: "text-sm", used_by: "buttons, inputs (md+), labels, menus, card, alert" },
      "16": { px: 16, line_height: "leading-snug", utilities: "text-base", used_by: "card-title, dialog-title, alert-dialog-title, native input <md" },
      "18": { px: 18, utilities: "text-lg", used_by: "typography.large" },
      "20": { px: 20, utilities: "text-xl", used_by: "typography.lead, h4" },
      "24": { px: 24, utilities: "text-2xl", used_by: "h3" },
      "30": { px: 30, utilities: "text-3xl", used_by: "h2" },
      "36": { px: 36, utilities: "text-4xl", used_by: "h1" },
      "48": { px: 48, utilities: "lg:text-5xl", used_by: "h1 (>=1024px)" },
    },
    weights: { 400: "normal (body, inputs)", 500: "font-medium (buttons, labels, titles, badge, kbd)", 600: "font-semibold (headings, h2/h3/h4, large)", 800: "font-extrabold (h1)" },
  },
  spacing: { unit_px: 4, used: [0.5, 1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10] },
  radii_px: {
    base: { px: 10, source: "--radius 0.625rem x 16px" },
    sm: { px: 6, formula: "radius x 0.6" },
    md: { px: 8, formula: "radius x 0.8" },
    lg: { px: 10, formula: "radius x 1" },
    xl: { px: 14, formula: "radius x 1.4" },
    "2xl": { px: 18, formula: "radius x 1.8" },
    "3xl": { px: 22, formula: "radius x 2.2" },
    "4xl": { px: 26, formula: "radius x 2.6 (badge)" },
    "checkbox-4px": { px: 4, source: "rounded-[4px]" },
    "tooltip-arrow-2px": { px: 2, source: "rounded-[2px]" },
    full: { px: 9999, source: "rounded-full (switch, radio, skeleton avatar)" },
  },
  border_widths_px: [0, 1, 2],
  shadows: {
    "shadow-sm": "0 1px 2px 0 rgb(0 0 0 / 0.05) - tabs trigger active",
    "shadow-md": "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1) - dropdown/popover/select content",
    "shadow-lg": "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1) - menu sub-content",
  },
  control_heights_px: {
    "button.xs": 24, "button.sm": 28, "button.default": 32, "button.lg": 36,
    "button.icon-xs": 24, "button.icon-sm": 28, "button.icon": 32, "button.icon-lg": 36,
    input: 32, "select-trigger.default": 32, "select-trigger.sm": 28,
    checkbox: 16, radio: 16,
    "switch.default": { width: 32, height: 18.4 }, "switch.sm": { width: 24, height: 14 },
    "slider-thumb": 12, "slider-track": 4, "tabs-list": 32, kbd: 20, badge: 20,
    menubar: 32,
  },
  horizontal_padding_px: {
    "button.xs": { px: 8, source: "px-2" },
    "button.sm": { px: 10, source: "px-2.5" },
    "button.default": { px: 10, source: "px-2.5" },
    "button.lg": { px: 10, source: "px-2.5" },
    "button.icon-with-label-default": { px: 8, source: "px-2.5 + has-data-[icon]:pl/pr-2" },
    input: { px: 10, source: "cn-input px-2.5" },
    "select-trigger": { left: 10, right: 8, source: "cn-select-trigger pl-2.5 pr-2" },
    "menu-item": { x: 6, y: 4, source: "cn-dropdown-menu-item/select-item px-1.5 py-1" },
    badge: { px: 8, source: "cn-badge px-2" },
    kbd: { px: 4, source: "cn-kbd px-1" },
    "dialog-content": { px: 16, source: "p-4" },
    card: { px: 16, source: "px-4" },
  },
  // gallery presentation chrome - NOT part of the shadcn visual contract
  gallery_chrome: {
    caption_font_px: 10, // .specimen-caption text-[10px]
    caption: "font-mono text-[10px] text-muted-foreground whitespace-nowrap",
    specimen_frame: "rounded-xl ring-1 ring-foreground/10 bg-background p-5",
    note: "gallery chrome only - not a shadcn token; closure check may match here but reports the section",
  },
  focus_ring: { width_px: 3, color: "ring/50 (translucent.ring-50)", border: "border->ring", source: "focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:border-ring" },
  disabled: { opacity: 0.5, extras: "disabled:pointer-events-none; inputs also cursor-not-allowed; input/textarea disabled bg-input/50 (dark /80)", source: "recipes" },
  pressed: { translate_y_px: 1, source: "cn-button active:not-aria-[haspopup]:translate-y-px" },
};

fs.writeFileSync(
  path.join(__dirname, "..", "tokens.json"),
  JSON.stringify(tokens, null, 2),
);
console.log("tokens.json written");
