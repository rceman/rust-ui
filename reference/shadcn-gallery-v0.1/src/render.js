// Renderer: shell (Tabs nav + Toggle Group theme switch), sections, cells.
// Classic script - works from file://, no modules, no network.
// URL contract: ?page=<id>&theme=light|dark|system&capture=1[&state=<state>]
(() => {
  const qp = () => new URLSearchParams(location.search);
  const getParam = (k, d) => qp().get(k) || d;

  const SECTION_META = {
    button: ["Button", "Trigger actions; variants x forced states x sizes."],
    badge: ["Badge", "Status chips and inline labels."],
    separator: ["Separator", "Visual divider, horizontal and vertical."],
    card: ["Card", "Grouped content container."],
    alert: ["Alert", "Contextual messages with icon, title and description."],
    skeleton: ["Skeleton", "Loading placeholder shapes."],
    kbd: ["Kbd", "Keyboard key glyphs."],
    label: ["Label", "Form label paired with a control."],
    input: ["Input", "Single-line text field states."],
    textarea: ["Textarea", "Multi-line text field states."],
    checkbox: ["Checkbox", "Tri-state check control."],
    "radio-group": ["Radio Group", "Single-selection control set."],
    switch: ["Switch", "Binary toggle control."],
    slider: ["Slider", "Value scrubber - single and range thumbs."],
    select: ["Select", "Trigger states (open list under Overlays)."],
    field: ["Field", "Label + description + error wiring."],
    tabs: ["Tabs", "Default and line variants."],
    breadcrumb: ["Breadcrumb", "Path navigation with ellipsis and current page."],
    pagination: ["Pagination", "Page links with previous/next."],
    toggle: ["Toggle", "Pressed-state control."],
    "toggle-group": ["Toggle Group", "Grouped toggles, outline variant."],
    "button-group": ["Button Group", "Toolbar pattern."],
    h1: ["Typography", "Upstream typography scale."],
    dialog: ["Dialog", "Modal surface over a backdrop."],
    "alert-dialog": ["Alert Dialog", "Confirmational modal."],
    popover: ["Popover", "Anchored content, bottom/center."],
    tooltip: ["Tooltip", "Anchored hint, top/center with arrow."],
    "dropdown-menu": ["Dropdown Menu", "Anchored menu surface."],
    "native-text": ["Native Text", "Upstream input/textarea recipes - visual contract for the future native RichEdit text. `readonly` is a semantic state; visually = filled per upstream."],
  };
  const ORDER = [
    "button", "badge", "separator", "card", "alert", "skeleton", "kbd",
    "label", "input", "textarea", "checkbox", "radio-group", "switch",
    "slider", "select", "field",
    "tabs", "breadcrumb", "pagination", "toggle", "toggle-group", "button-group",
    "h1", "h2", "h3", "h4", "p", "blockquote", "lead", "large", "small",
    "muted", "inline-code", "list", "table",
    "dialog", "alert-dialog", "popover", "tooltip", "dropdown-menu",
    "native-text",
  ];
  const CATEGORY_ORDER = { components: 0, forms: 1, navigation: 2, typography: 3, overlays: 4, "native-text": 5 };
  // per-category description overrides (e.g. the open select list lives on
  // the Overlays page, while trigger states live under Forms)
  const SECTION_META_BY_CATEGORY = {
    "overlays:select": ["Select", "Open listbox anchored to its trigger - upstream side=bottom, align=center, sideOffset=4."],
  };
  // Matrix sections: variants x forced states
  const MATRIX = {
    button: {
      rows: ["default", "secondary", "outline", "ghost", "destructive", "link"],
      cols: ["normal", "hover", "pressed", "focus-visible", "disabled"],
      idOf: (v, st) => `button.${v}${st === "normal" ? "" : "." + st}`,
      sizeIds: ["size-xs", "size-sm", "size-lg", "icon-inline-start", "size-icon-xs", "size-icon-sm", "size-icon", "size-icon-lg"].map((s) => `button.default.${s}`),
    },
    toggle: {
      rows: ["default", "outline"],
      cols: ["off", "on", "hover", "focus-visible", "disabled"],
      idOf: (v, st) => `toggle.${v}.${st}`,
    },
  };
  // min grid column width per section so wide specimens never overlap
  // (applied via inline style - Tailwind cannot see template-built classes)
  const SECTION_MINW = {
    card: 340, alert: 340, skeleton: 300, field: 320, input: 300,
    textarea: 340, "radio-group": 300, select: 280, slider: 280,
    label: 300, breadcrumb: 440, pagination: 540, tabs: 420,
    "toggle-group": 300, "button-group": 300, "native-text": 340,
    checkbox: 240, switch: 240, badge: 200, separator: 220,
  };
  // uniform frame height per section (static class names only)
  const SECTION_CELLH = {
    card: "min-h-56", field: "min-h-40", skeleton: "min-h-24", alert: "min-h-32",
    slider: "min-h-24", "radio-group": "min-h-24", tabs: "min-h-24",
    "toggle-group": "min-h-24", "button-group": "min-h-24",
    breadcrumb: "min-h-24", pagination: "min-h-24", select: "min-h-24",
    input: "min-h-24", textarea: "min-h-32", "native-text": "min-h-28",
    checkbox: "min-h-24", switch: "min-h-24", badge: "min-h-20",
    separator: "min-h-24", label: "min-h-24", kbd: "min-h-20",
  };

  const h = (tag, attrs, inner) => {
    const a = Object.entries(attrs || {})
      .filter(([, v]) => v !== undefined && v !== null && v !== false)
      .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${v}"`))
      .join("");
    return `<${tag}${a}>${inner || ""}</${tag}>`;
  };
  const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");

  // specimen cell: framed specimen + mono caption below (uniform section height)
  function cell(entry, cellH) {
    const spec = entry.category === "overlays"
      ? entry.render() // overlay stages are already framed surfaces
      : h("div", { class: `specimen-frame flex flex-1 items-center justify-center p-5 ${cellH || "min-h-24"}` }, entry.render());
    return h("div", { class: "flex flex-col" },
      spec + h("div", { class: "specimen-caption" }, esc(entry.automation_id)));
  }

  // variantxstate matrix: row headers (variant) + column headers (state),
  // the whole matrix carries one frame
  function matrixSection(list, cfg, cellH) {
    const byId = {};
    for (const e2 of list) byId[e2.automation_id] = e2;
    const caption = (t) => h("div", { class: "text-muted-foreground text-xs" }, t);
    const rowsHtml = cfg.rows.map((v) =>
      h("div", { class: "text-muted-foreground flex items-center text-xs font-medium" }, v) +
      cfg.cols.map((st) => {
        const id = cfg.idOf(v, st);
        const ent = byId[id];
        const cellHCls = cellH || "min-h-20";
        return h("div", { class: `flex flex-col items-center justify-center gap-1.5 ${cellHCls}` },
          (ent ? ent.render() : "") + h("div", { class: "specimen-caption" }, esc(id)));
      }).join(""),
    ).join("");
    const sizeRow = (label, ids) =>
      h("div", { class: "mt-6 flex items-end gap-2" },
        h("div", { class: "text-muted-foreground w-20 shrink-0 pb-6 text-xs font-medium" }, label) +
        h("div", { class: "flex items-end gap-x-6" },
          ids.map((id) => {
            const ent = byId[id];
            return h("div", { class: "flex flex-col items-center gap-1.5" },
              (ent ? ent.render() : "") + h("div", { class: "specimen-caption" }, esc(id)));
          }).join("")));
    const sizeRows = cfg.sizeIds
      ? sizeRow("Sizes", cfg.sizeIds.slice(0, 4)) + sizeRow("Icon sizes", cfg.sizeIds.slice(4))
      : "";
    const headRow = h("div", {}) + cfg.cols.map((st) =>
      h("div", { class: "text-muted-foreground pb-1 text-center text-xs font-medium" }, st)).join("");
    return h("div", { class: "specimen-frame p-5" },
      h("div", {
        style: `display:grid;grid-template-columns:88px repeat(${cfg.cols.length},minmax(130px,1fr));column-gap:8px;row-gap:8px;align-items:center`,
      }, headRow + rowsHtml) + sizeRows);
  }

  function applyTheme(mode) {
    const root = document.documentElement;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const set = () =>
      root.classList.toggle(
        "dark",
        mode === "dark" || (mode === "system" && mq.matches),
      );
    set();
    if (mode === "system" && !window.__systemThemeBound) {
      mq.addEventListener("change", set);
      window.__systemThemeBound = true;
    }
  }

  const TABS_BASE =
    "cn-tabs-trigger relative inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center whitespace-nowrap text-foreground/60 transition-all hover:text-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring dark:text-muted-foreground dark:hover:text-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 data-active:bg-background data-active:text-foreground dark:data-active:border-input dark:data-active:bg-input/30 dark:data-active:text-foreground";
  const TG_ITEM =
    "cn-toggle cn-toggle-group-item cn-toggle-variant-outline cn-toggle-size-sm group/toggle inline-flex items-center justify-center whitespace-nowrap outline-none hover:bg-muted focus-visible:ring-[3px] disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 shrink-0";

  function navHtml(page) {
    return h(
      "div",
      { "data-slot": "tabs", "data-orientation": "horizontal", class: "cn-tabs group/tabs flex data-horizontal:flex-col" },
      h(
        "div",
        {
          "data-slot": "tabs-list", role: "tablist", "data-variant": "default",
          class: "cn-tabs-list cn-tabs-list-variant-default bg-muted group/tabs-list inline-flex w-fit items-center justify-center text-muted-foreground",
        },
        window.PAGES.map((p) =>
          h("a", {
            role: "tab",
            "aria-selected": p.id === page ? "true" : "false",
            "data-active": p.id === page || undefined,
            "data-slot": "tabs-trigger",
            "data-automation-id": `shell.nav.${p.id}`,
            class: TABS_BASE,
            href: `?page=${p.id}&theme=${getParam("theme", "system")}${getParam("capture", "") ? "&capture=" + getParam("capture", "") : ""}`,
          }, p.label),
        ).join(""),
      ),
    );
  }

  function themeSwitchHtml(theme) {
    return h(
      "div",
      {
        "data-slot": "toggle-group", role: "group",
        "data-variant": "outline", "data-size": "sm", "data-spacing": "2", "data-orientation": "horizontal",
        style: "--gap:2",
        class: "cn-toggle-group group/toggle-group flex w-fit flex-row items-center gap-[--spacing(var(--gap))]",
      },
      [["light", "sun", "Light"], ["dark", "moon", "Dark"], ["system", "monitor", "System"]]
        .map(([id, ic_, label]) =>
          h("button", {
            type: "button",
            "data-slot": "toggle-group-item",
            "data-variant": "outline", "data-size": "sm", "data-spacing": "2",
            "aria-pressed": theme === id ? "true" : "false",
            "data-automation-id": `shell.theme.${id}`,
            "data-theme-value": id,
            class: TG_ITEM,
          }, window.icon(ic_) + h("span", { class: "ml-1.5" }, label)),
        )
        .join(""),
    );
  }

  function mount() {
    const page = getParam("page", "all");
    const theme = getParam("theme", "system");
    const capture = getParam("capture", "") === "1";
    const state = getParam("state", "");
    document.documentElement.classList.toggle("capture-mode", capture);
    applyTheme(theme);

    const entries = window.CATALOG.filter(
      (e) => page === "all" || e.category === page,
    );

    // sections grouped by component within each category, in fixed order
    const groups = new Map();
    for (const e of entries) {
      const key =
        e.category === "typography" ? "typography"
        : e.category === "native-text" ? "native-text"
        : e.category === "overlays" ? `ov:${e.component}`
        : e.component;
      if (!groups.has(key)) groups.set(key, { cat: e.category, comp: e.component, list: [] });
      groups.get(key).list.push(e);
    }
    const ordered = [...groups.entries()].sort(
      (a, b) =>
        (CATEGORY_ORDER[a[1].cat] ?? 9) - (CATEGORY_ORDER[b[1].cat] ?? 9) ||
        ORDER.indexOf(a[1].comp) - ORDER.indexOf(b[1].comp),
    );

    const sections = ordered
      .map(([key, g]) => {
        const list = g.list;
        const metaKey = `${g.cat}:${g.comp}`;
        const [title, desc] =
          SECTION_META_BY_CATEGORY[metaKey] ||
          SECTION_META[g.comp] ||
          [g.comp[0].toUpperCase() + g.comp.slice(1), ""];
        const isOverlay = g.cat === "overlays";
        const isTypo = g.cat === "typography";
        const isNt = g.cat === "native-text";
        const cellH = SECTION_CELLH[g.comp];
        let body;
        if (MATRIX[g.comp] && !isOverlay) {
          body = matrixSection(list, MATRIX[g.comp], cellH);
        } else if (isOverlay) {
          body = h("div", { class: "flex flex-col gap-6" }, list.map((e2) => cell(e2)).join(""));
        } else if (isTypo) {
          // left-aligned typography specimens with inner padding; the contract
          // element is the typo element itself, the frame is chrome only
          body = h("div", { class: "flex max-w-3xl flex-col gap-4" },
            list.map((e2) =>
              h("div", { class: "flex flex-col" },
                h("div", { class: "specimen-frame p-6" }, e2.render()) +
                h("div", { class: "specimen-caption self-start" }, esc(e2.automation_id))),
            ).join(""));
        } else {
          const minW = SECTION_MINW[g.comp] || 180;
          body = h("div", {
            class: "items-stretch gap-x-4 gap-y-6",
            style: `display:grid;grid-template-columns:repeat(auto-fill,minmax(${minW}px,1fr))`,
          }, list.map((e2) => cell(e2, cellH)).join(""));
        }
        return h(
          "section",
          { class: "mt-10" },
          h("h2", { class: "text-base font-semibold tracking-tight" }, title) +
            h("p", { class: "text-muted-foreground mb-4 text-sm" }, desc) +
            body,
        );
      })
      .join("");

    document.getElementById("app").innerHTML =
      h(
        "header",
        { class: "border-b" },
        h(
          "div",
          { class: "mx-auto flex h-14 max-w-[1280px] items-center justify-between gap-4 px-6" },
          h(
            "div",
            { class: "flex items-center gap-6" },
            h("h1", { class: "text-sm font-semibold", "data-automation-id": "shell.title" }, "rust-ui Gallery") +
              navHtml(page),
          ) + themeSwitchHtml(theme),
        ),
      ) +
      h(
        "main",
        { class: "mx-auto max-w-[1280px] px-6 pt-8 pb-16" },
        h(
          "div",
          {},
          h("h1", { class: "text-xl font-semibold tracking-tight" },
            window.PAGES.find((p) => p.id === page)?.label || "All") +
            h("p", { class: "text-muted-foreground mt-1 text-sm" },
              `Frozen shadcn base-nova reference - ${entries.length} specimens`),
        ) + sections,
      );

    // interactions: nav links + theme buttons update URL without reload
    document.querySelectorAll("[data-automation-id^='shell.nav.']").forEach((a) => {
      a.addEventListener("click", (ev) => {
        ev.preventDefault();
        history.pushState(null, "", a.getAttribute("href"));
        mount();
      });
    });
    document.querySelectorAll("[data-theme-value]").forEach((b) => {
      b.addEventListener("click", () => {
        const p = qp();
        p.set("theme", b.getAttribute("data-theme-value"));
        history.pushState(null, "", "?" + p.toString());
        mount();
      });
    });

    // capture states: real focus + fixed selection on native-text
    if (page === "native-text") {
      const selId =
        state === "multiline-selection"
          ? "native-text.multiline.selection"
          : "native-text.single-line.selection";
      const el = document.querySelector(`[data-automation-id='${selId}']`);
      if (el && (state === "" || state === "multiline-selection")) {
        el.focus();
        if (state === "multiline-selection") el.setSelectionRange(22, 33);
        else el.setSelectionRange(0, 6);
      }
    }
    document.body.setAttribute("data-render-done", "");
  }

  window.addEventListener("popstate", mount);
  window.mountGallery = mount;
  mount();
})();
