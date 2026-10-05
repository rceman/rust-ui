// Generates token-bindings.json - every row binds a CORE specimen's computed
// CSS output to a tokens.json path. reference:check resolves the token value
// and compares the LIVE computed value after state forcing:
//   colors -> sRGB+alpha, tol <= 1/255/channel, alpha <= 0.005
//   lengths -> exact within 0.01px
// token_path forms:
//   colors.<theme>.<name>.srgb|alpha
//   translucent.<name>.<theme>.srgb|alpha
//   control_heights_px.<key>            -> px number (or {width,height})
//   horizontal_padding_px.<key>.<px|left|right|x|y>
//   radii_px.<key>.px
//   typography.sizes_px.<n>.px|line_height
//   typography.weight_values.<name>
//   disabled.opacity | focus_ring.width_px | pressed.translate_y_px
// theme "both" expands to light+dark; {theme} in a path resolves per theme.
const fs = require("node:fs");
const path = require("node:path");

const B = [];
const b = (automation_id, theme, css_property, token_path, extra = {}) =>
  B.push({ automation_id, theme, css_property, token_path, ...extra });
const both = (id, prop, tok, extra = {}) => {
  b(id, "light", prop, tok, extra); b(id, "dark", prop, tok, extra);
};
const col = (id, prop, name, extra = {}) => {
  both(id, prop, `colors.{theme}.${name}.srgb`, extra);
  both(id, prop, `colors.{theme}.${name}.alpha`, extra);
};
const trn = (id, prop, name, extra = {}) => {
  both(id, prop, `translucent.${name}.{theme}.srgb`, extra);
  both(id, prop, `translucent.${name}.{theme}.alpha`, extra);
};
const num = (id, prop, tok, extra = {}) => both(id, prop, tok, extra);
// focus ring = box-shadow 0 0 0 3px ring/50 - width + color + alpha
const ring = (id, extra = {}) => {
  num(id, "box-shadow-width", "focus_ring.width_px", extra);
  both(id, "box-shadow-color", "translucent.ring-50.{theme}.srgb", extra);
  both(id, "box-shadow-alpha", "translucent.ring-50.{theme}.alpha", extra);
};

// ---------------- button ----------------
// variant normal backgrounds / colors
const BTN = {
  default: ["primary", "primary-foreground"],
  secondary: ["secondary", "secondary-foreground"],
  outline: ["background", "foreground"],
  ghost: ["transparent", "foreground"],
  destructive: ["destructive", "destructive"],
  link: ["transparent", "primary"],
};
// theme-divergent bgs: outline dark = bg-input/30; destructive = destructive/10|/20
for (const [v, [bg, fg]] of Object.entries(BTN)) {
  if (v === "outline") {
    b("button.outline", "light", "background-color", "colors.light.background.srgb");
    b("button.outline", "dark", "background-color", "translucent.input-30-dark.dark.srgb");
    b("button.outline", "dark", "background-color", "translucent.input-30-dark.dark.alpha");
  } else if (v === "destructive") {
    trn(`button.${v}`, "background-color", "destructive-10-20"); // bg-destructive/10 light, /20 dark
  } else if (bg !== "transparent") {
    col(`button.${v}`, "background-color", bg);
  } else {
    both(`button.${v}`, "background-color", "special.transparent.srgb");
    both(`button.${v}`, "background-color", "special.transparent.alpha");
  }
  col(`button.${v}`, "color", fg);
}
// hover outputs
trn("button.default.hover", "background-color", "primary-80");
both("button.secondary.hover", "background-color", "translucent.secondary-hover.{theme}.srgb");
both("button.secondary.hover", "background-color", "translucent.secondary-hover.{theme}.alpha");
b("button.outline.hover", "light", "background-color", "colors.light.muted.srgb");
b("button.outline.hover", "dark", "background-color", "translucent.input-50-dark.dark.srgb");
b("button.outline.hover", "dark", "background-color", "translucent.input-50-dark.dark.alpha");
col("button.outline.hover", "color", "foreground");
b("button.ghost.hover", "light", "background-color", "colors.light.muted.srgb");
b("button.ghost.hover", "dark", "background-color", "translucent.muted-50.dark.srgb");
b("button.ghost.hover", "dark", "background-color", "translucent.muted-50.dark.alpha");
trn("button.destructive.hover", "background-color", "destructive-20-30");
// pressed output: translate-y-px via transform matrix
num("button.default.pressed", "translate-y", "pressed.translate_y_px");
// focus ring
ring("button.default.focus-visible");
both("button.default.focus-visible", "border-color", "colors.{theme}.ring.srgb");
// disabled
num("button.default.disabled", "opacity", "disabled.opacity");
// invalid output - dark border is destructive/50 (dark:aria-invalid branch)
b("button.default.invalid", "light", "border-color", "colors.light.destructive.srgb");
b("button.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.srgb");
b("button.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.alpha");
both("button.default.invalid", "box-shadow-alpha", "translucent.destructive-ring-20-40.{theme}.alpha");
// geometry: height/padding/font/radius
num("button.default", "height", "control_heights_px.button.default");
num("button.default", "padding-left", "horizontal_padding_px.button.default.px");
num("button.default", "padding-right", "horizontal_padding_px.button.default.px");
num("button.default", "font-size", "typography.sizes_px.14.px");
num("button.default", "font-weight", "typography.weight_values.medium");
num("button.default", "line-height", "typography.sizes_px.14.line_height");
num("button.default", "border-top-width", "border_widths_px.1");
num("button.default", "border-top-left-radius", "radii_px.lg.px");
// inline icon branch (R05): padding-inline-start 8px = px-2
num("button.default.icon-inline-start", "padding-left", "horizontal_padding_px.button.icon-with-label-default.px");
num("button.default.icon-inline-end", "padding-right", "horizontal_padding_px.button.icon-with-label-default.px");
num("button.default.size-xs", "height", "control_heights_px.button.xs");
num("button.default.size-sm", "height", "control_heights_px.button.sm");
num("button.default.size-lg", "height", "control_heights_px.button.lg");

// ---------------- icon-button ----------------
num("icon-button.default", "width", "control_heights_px.button.icon");
num("icon-button.default", "height", "control_heights_px.button.icon");
for (const s of ["icon-xs", "icon-sm", "icon", "icon-lg"])
  num(`icon-button.default.size-${s}`, "width", `control_heights_px.button.${s}`);
col("icon-button.default", "background-color", "primary");
trn("icon-button.default.hover", "background-color", "primary-80");
ring("icon-button.default.focus-visible");
num("icon-button.default.disabled", "opacity", "disabled.opacity");
num("icon-button.default.pressed", "translate-y", "pressed.translate_y_px");

// ---------------- input / textarea ----------------
for (const comp of ["input", "textarea"]) {
  num(`${comp}.default.placeholder`, "border-top-width", "border_widths_px.1");
  both(`${comp}.default.placeholder`, "border-color", "colors.{theme}.input.srgb");
  num(`${comp}.default.placeholder`, "padding-left", `horizontal_padding_px.${comp === "input" ? "input" : "input"}.px`);
  num(`${comp}.default.placeholder`, "border-top-left-radius", "radii_px.lg.px");
  both(`${comp}.default.filled`, "color", "colors.{theme}.foreground.srgb");
  ring(`${comp}.default.focus-visible`);
  both(`${comp}.default.focus-visible`, "border-color", "colors.{theme}.ring.srgb");
  num(`${comp}.default.disabled`, "opacity", "disabled.opacity");
  b(`${comp}.default.invalid`, "light", "border-color", "colors.light.destructive.srgb");
  b(`${comp}.default.invalid`, "dark", "border-color", "translucent.destructive-50-dark.dark.srgb");
  b(`${comp}.default.invalid`, "dark", "border-color", "translucent.destructive-50-dark.dark.alpha");
  both(`${comp}.default.invalid`, "box-shadow-alpha", "translucent.destructive-ring-20-40.{theme}.alpha");
  num(`${comp}.default.filled`, "font-size", "typography.sizes_px.14.px");
}
num("input.default.placeholder", "height", "control_heights_px.input");
b("input.default.placeholder", "dark", "background-color", "translucent.input-30-dark.dark.srgb");
b("input.default.placeholder", "dark", "background-color", "translucent.input-30-dark.dark.alpha");

// ---------------- checkbox / radio / switch ----------------
num("checkbox.default.unchecked", "width", "control_heights_px.checkbox");
num("checkbox.default.unchecked", "height", "control_heights_px.checkbox");
num("checkbox.default.unchecked", "border-top-left-radius", "radii_px.checkbox-4px.px");
col("checkbox.default.checked", "background-color", "primary");
col("checkbox.default.checked", "border-color", "primary");
ring("checkbox.default.focus-visible");
num("checkbox.default.disabled", "opacity", "disabled.opacity");
b("checkbox.default.invalid", "light", "border-color", "colors.light.destructive.srgb");
b("checkbox.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.srgb");
b("checkbox.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.alpha");
col("radio-group.default.selected", "background-color", "primary", { part: "item[option-a]" });
col("radio-group.default.selected", "background-color", "primary-foreground", { part: "item[option-a].indicator.icon" });
num("radio-group.default.selected", "border-top-left-radius", "radii_px.full.px", { part: "item[option-a]" });
// forced focus-visible lands on item[option-b] (the specimen's forced item)
ring("radio-group.default.focus-visible", { part: "item[option-b]" });
num("radio-group.default.disabled", "opacity", "disabled.opacity", { part: "item[option-a]" });
// checked+invalid keeps border-primary (aria-invalid:aria-checked:border-primary);
// unchecked+invalid gets border-destructive
b("radio-group.default.invalid", "light", "border-color", "colors.light.primary.srgb", { part: "item[option-a]" });
b("radio-group.default.invalid", "light", "border-color", "colors.light.destructive.srgb", { part: "item[option-b]" });
b("radio-group.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.srgb", { part: "item[option-a]" });
b("radio-group.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.alpha", { part: "item[option-a]" });
b("radio-group.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.srgb", { part: "item[option-b]" });
b("radio-group.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.alpha", { part: "item[option-b]" });
col("switch.default.checked", "background-color", "primary");
num("switch.default.checked", "width", "control_heights_px.switch.default.width");
num("switch.default.checked", "height", "control_heights_px.switch.default.height");
num("switch.default.size-sm", "width", "control_heights_px.switch.sm.width");
num("switch.default.size-sm", "height", "control_heights_px.switch.sm.height");
num("switch.default.unchecked", "border-top-left-radius", "radii_px.full.px");
ring("switch.default.focus-visible");
num("switch.default.disabled", "opacity", "disabled.opacity");
b("switch.default.invalid", "light", "border-color", "colors.light.destructive.srgb");
b("switch.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.srgb");
b("switch.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.alpha");
// thumb position: data-checked/-unchecked live on the THUMB itself ->
// translate-x calc(100%-2px) of thumb size (default 16 -> 14px; sm 12 -> 10px)
num("switch.default.checked", "translate-x", "switch_thumb_px.default.translate_checked", { part: "thumb" });
num("switch.default.unchecked", "translate-x", "switch_thumb_px.default.translate_unchecked", { part: "thumb" });
num("switch.default.size-sm", "translate-x", "switch_thumb_px.sm.translate_checked", { part: "thumb" });
num("switch.default.size-sm-unchecked", "translate-x", "switch_thumb_px.sm.translate_unchecked", { part: "thumb" });
num("switch.default.checked", "width", "switch_thumb_px.default.size", { part: "thumb" });
num("switch.default.size-sm", "width", "switch_thumb_px.sm.size", { part: "thumb" });
// disabled/disabled-checked: assert the thumb position per specimen so a
// checked/unchecked mix-up (rev-3 defect) fails the bindings
num("switch.default.disabled", "translate-x", "switch_thumb_px.default.translate_unchecked", { part: "thumb" });
num("switch.default.disabled-checked", "translate-x", "switch_thumb_px.default.translate_checked", { part: "thumb" });
// thumb colour: bg-background light both states; dark unchecked=foreground,
// dark checked=primary-foreground
const thumbBg = (id, lightTok, darkTok, extra = {}) => {
  b(id, "light", "background-color", `colors.light.${lightTok}.srgb`, extra);
  b(id, "light", "background-color", `colors.light.${lightTok}.alpha`, extra);
  b(id, "dark", "background-color", `colors.dark.${darkTok}.srgb`, extra);
  b(id, "dark", "background-color", `colors.dark.${darkTok}.alpha`, extra);
};
thumbBg("switch.default.checked", "background", "primary-foreground", { part: "thumb" });
thumbBg("switch.default.unchecked", "background", "foreground", { part: "thumb" });
thumbBg("switch.default.size-sm", "background", "primary-foreground", { part: "thumb" });
thumbBg("switch.default.size-sm-unchecked", "background", "foreground", { part: "thumb" });
thumbBg("switch.default.disabled", "background", "foreground", { part: "thumb" });
thumbBg("switch.default.disabled-checked", "background", "primary-foreground", { part: "thumb" });
// switches inside field.horizontal + the settings-nav content rows
num("field.horizontal", "translate-x", "switch_thumb_px.default.translate_checked", { part: "thumb" });
thumbBg("field.horizontal", "background", "primary-foreground", { part: "thumb" });
num("settings-nav.default", "translate-x", "switch_thumb_px.default.translate_checked", { part: "content.field[animations].thumb" });
thumbBg("settings-nav.default", "background", "primary-foreground", { part: "content.field[animations].thumb" });
num("settings-nav.default", "translate-x", "switch_thumb_px.default.translate_unchecked", { part: "content.field[compact].thumb" });
thumbBg("settings-nav.default", "background", "foreground", { part: "content.field[compact].thumb" });

// ---------------- select ----------------
num("select.default.placeholder", "height", "control_heights_px.select-trigger.default");
num("select.default.size-sm", "height", "control_heights_px.select-trigger.sm");
num("select.default.placeholder", "padding-left", "horizontal_padding_px.select-trigger.left");
num("select.default.placeholder", "padding-right", "horizontal_padding_px.select-trigger.right");
num("select.default.placeholder", "border-top-left-radius", "radii_px.lg.px");
ring("select.default.focus-visible");
num("select.default.disabled", "opacity", "disabled.opacity");
b("select.default.invalid", "light", "border-color", "colors.light.destructive.srgb");
b("select.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.srgb");
b("select.default.invalid", "dark", "border-color", "translucent.destructive-50-dark.dark.alpha");
col("select.default.open.content", "background-color", "popover");
num("select.default.open.content", "border-top-left-radius", "radii_px.lg.px");
num("select.default.open.content", "padding-left", "horizontal_padding_px.menu-item.x", { part: "item[development]" });
num("select.default.open.content", "padding-top", "horizontal_padding_px.menu-item.y", { part: "item[development]" });
num("select.default.open.content", "font-size", "typography.sizes_px.14.px", { part: "item[development]" });
num("select.default.open.content", "margin-top", "horizontal_padding_px.menu-item.y", { part: "separator" });
col("select.default.open.item-highlighted", "background-color", "accent"); // highlighted item
num("select.default.open.content", "opacity", "disabled.opacity", { part: "item[deprecated]" });
num("select.default.open.trigger", "height", "control_heights_px.button.default");

// ---------------- tabs ----------------
num("tabs.default.active", "height", "control_heights_px.tabs-list", { part: "list" });
b("tabs.default.active", "light", "background-color", "colors.light.background.srgb", { part: "list.tab[account]" });
b("tabs.default.active", "dark", "background-color", "translucent.input-30-dark.dark.srgb", { part: "list.tab[account]" });
b("tabs.default.active", "dark", "background-color", "translucent.input-30-dark.dark.alpha", { part: "list.tab[account]" });
num("tabs.default.inactive", "font-size", "typography.sizes_px.14.px", { part: "list.tab[password]" });
num("tabs.default.disabled", "opacity", "disabled.opacity", { part: "list.tab[password]" });
ring("tabs.default.focus-visible", { part: "list.tab[password]" });

// ---------------- dropdown-menu ----------------
col("dropdown-menu.default.content", "background-color", "popover");
num("dropdown-menu.default.content", "border-top-left-radius", "radii_px.lg.px");
num("dropdown-menu.default.content", "padding-left", "horizontal_padding_px.menu-item.x", { part: "item[profile]" });
num("dropdown-menu.default.content", "padding-top", "horizontal_padding_px.menu-item.y", { part: "item[profile]" });
num("dropdown-menu.default.content", "font-size", "typography.sizes_px.14.px", { part: "item[profile]" });
col("dropdown-menu.default.item-highlighted", "background-color", "accent"); // highlighted item
num("dropdown-menu.default.content", "opacity", "disabled.opacity", { part: "item[billing]" });
both("dropdown-menu.default.content", "color", "colors.{theme}.destructive.srgb", { part: "item[delete]" });
trn("dropdown-menu.default.item-destructive-highlighted", "background-color", "destructive-10-20"); // destructive highlighted /10 light /20 dark

// ---------------- dialog ----------------
trn("dialog.default", "background-color", "backdrop-black-10", { part: "backdrop" });
col("dialog.default.content", "background-color", "popover");
num("dialog.default.content", "border-top-left-radius", "radii_px.xl.px");
num("dialog.default.content", "padding-left", "horizontal_padding_px.dialog-content.px");
num("dialog.default.content", "font-weight", "typography.weight_values.medium", { part: "header.title" });
num("dialog.default.content", "font-size", "typography.sizes_px.14.px", { part: "header.description" });
trn("alert-dialog.default", "background-color", "backdrop-black-10", { part: "backdrop" });
col("alert-dialog.default.content", "background-color", "popover");

// ---------------- popover / tooltip ----------------
col("popover.default.content", "background-color", "popover");
num("popover.default.content", "border-top-left-radius", "radii_px.lg.px");
col("tooltip.default.content", "background-color", "foreground");
num("tooltip.default.content", "font-size", "typography.sizes_px.12.px");

// ---------------- card / badge / alert / kbd / separator / skeleton / label ----------------
col("card.default", "background-color", "card");
num("card.default", "border-top-left-radius", "radii_px.xl.px");
num("card.default", "padding-left", "horizontal_padding_px.card.px", { part: "content" });
num("card.default", "font-size", "typography.sizes_px.16.px", { part: "header.title" });
num("card.default", "font-weight", "typography.weight_values.medium", { part: "header.title" });
col("badge.default", "background-color", "primary");
num("badge.default", "border-top-left-radius", "radii_px.4xl.px");
num("badge.default", "font-size", "typography.sizes_px.12.px");
col("alert.destructive", "color", "destructive", { part: "title" });
num("kbd.default", "font-size", "typography.sizes_px.12.px");
num("kbd.default", "font-weight", "typography.weight_values.medium");
num("kbd.default", "padding-left", "horizontal_padding_px.kbd.px");
col("separator.horizontal", "background-color", "border");
num("separator.horizontal", "height", "border_widths_px.1");
col("skeleton.default", "background-color", "muted", { part: "avatar" });
num("label.default", "font-size", "typography.sizes_px.14.px", { part: "label" });
num("label.default", "font-weight", "typography.weight_values.medium", { part: "label" });

// ---------------- field ----------------
num("field.default", "font-size", "typography.sizes_px.14.px", { part: "description" });
both("field.default.invalid", "color", "colors.{theme}.destructive.srgb", { part: "error" });
num("field.default.disabled", "opacity", "disabled.opacity", { part: "control" }); // input itself
num("field.horizontal", "font-weight", "typography.weight_values.medium", { part: "content.label" });

// ---------------- settings-nav ----------------
col("settings-nav.default", "background-color", "sidebar", { part: "sidebar" });
num("settings-nav.default", "width", "control_heights_px.sidebar-width", { part: "sidebar" });
num("settings-nav.general", "height", "control_heights_px.sidebar-item-height");
col("settings-nav.appearance", "background-color", "sidebar-accent"); // active item
num("settings-nav.appearance", "font-size", "typography.sizes_px.14.px", { part: "label" });
col("settings-nav-item.default.hover", "background-color", "sidebar-accent");
num("settings-nav-item.default.focus-visible", "box-shadow-width", "focus_ring.sidebar_width_px");
both("settings-nav-item.default.focus-visible", "box-shadow-color", "colors.{theme}.sidebar-ring.srgb");
both("settings-nav-item.default.focus-visible", "box-shadow-alpha", "colors.{theme}.sidebar-ring.alpha");
num("settings-nav-item.default.disabled", "opacity", "disabled.opacity");
col("settings-nav-item.default.active", "background-color", "sidebar-accent");
// collapsed rail width
b("settings-nav.default", "light", "width", "control_heights_px.sidebar-icon-rail", { part: "sidebar", capture_state: "settings-nav-collapsed" });
b("settings-nav.default", "dark", "width", "control_heights_px.sidebar-icon-rail", { part: "sidebar", capture_state: "settings-nav-collapsed" });

// ---------------- typography ----------------
const TYPO = {
  "typography.h1": [48, "extrabold"], "typography.h2": [30, "semibold"], // h1 = lg:text-5xl at 1440px
  "typography.h3": [24, "semibold"], "typography.h4": [20, "semibold"],
  "typography.lead": [20, "normal"], "typography.p": [16, "normal"],
  "typography.small": [14, "medium"], "typography.muted": [14, "normal"],
  "typography.inline-code": [14, "semibold"],
};
for (const [id, [fs2, w]] of Object.entries(TYPO)) {
  num(id, "font-size", `typography.sizes_px.${fs2}.px`);
  num(id, "font-weight", `typography.weight_values.${w}`);
}
col("typography.muted", "color", "muted-foreground");
// figure element carries the docs-chrome bg/radius/size (globals.css rule)
col("typography.code-block", "background-color", "code");
num("typography.code-block", "border-top-left-radius", "radii_px.2xl.px");
num("typography.code-block", "font-size", "typography.sizes_px.14.px");
// mdx-components pre px-4 py-3.5 - the inset the R02 ink check could not see
num("typography.code-block", "padding-left", "horizontal_padding_px.code-block-pre.x", { part: "pre" });
num("typography.code-block", "padding-right", "horizontal_padding_px.code-block-pre.x", { part: "pre" });
num("typography.code-block", "padding-top", "horizontal_padding_px.code-block-pre.y", { part: "pre" });
num("typography.code-block", "padding-bottom", "horizontal_padding_px.code-block-pre.y", { part: "pre" });

// ---------------- gallery shell ----------------
num("shell.title", "font-size", "typography.sizes_px.14.px");
num("shell.title", "font-weight", "typography.weight_values.semibold");

fs.writeFileSync(path.join(__dirname, "..", "token-bindings.json"),
  JSON.stringify({
    "$schema": "rust-ui.shadcn-reference.token-bindings/0.1",
    reference_version: "0.1",
    candidate_revision: 2,
    rule: "each binding compares the LIVE computed value (after forced state) against the tokens.json value; colors to sRGB+alpha (tol <=1/255 per channel, alpha <=0.005), lengths exact to 0.01px; box-shadow-width/-color/-alpha inspect the focus ring shadow",
    bindings: B,
  }, null, 2),
);
console.log("token-bindings.json written:", B.length, "bindings");
