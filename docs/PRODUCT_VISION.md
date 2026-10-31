# rust-ui product vision

**Status: STRATEGIC DIRECTION APPROVED IN PRINCIPLE BY PLANNER; FOUNDER PUBLICATION / MERGE AUTHORIZATION PENDING.** Technical profiles, numerical budgets, workload definitions and implementation sequencing remain provisional. This approval does not authorize compiler implementation or change the active P12 scope.

**Prompt:** `rust-ui-astra-PRODUCT-VISION-20261031-225600`

**Repository base:** `873c9996a908d5307c364925bfec6b4c1c40eb54` — verified remote `main` on 2026-10-31.

**Related proposal:** [Web-native feasibility and roadmap](WEB_NATIVE_FEASIBILITY_AND_ROADMAP.md).

## Strategic decision record

**Decision source:** `rust-ui-astra-VISION-APPROVAL-20261031-235000`. Planner reviewed both documents at `e95fcd92ae3897cc4ba40949678e6c4f8885d436` and approved the strategic direction in principle.

| Disposition | Decision for Lead / Worker onboarding |
|---|---|
| Approved direction | A cross-platform native desktop application framework and practical Electron/Tauri alternative; HTML/CSS-compatible authoring compiles into the existing typed Rust execution model. Direct Rust authoring remains first-class. Windows, macOS and Linux are the native backend targets. |
| Permanent native-runtime boundaries | One retained UI tree and shared semantic contracts; native input, text and accessibility ownership; no mandatory browser engine, WebView, DOM runtime or JavaScript VM in shipped applications. Desktop-only and desktop-responsive scope remains unchanged. |
| Permitted future capabilities | Flexbox, Grid, responsive layout, CSS variables, selectors, animations and transitions are not permanently rejected concepts. Their concrete native contracts and implementation require separate authorization and proof. |
| Temporary v0.1 exclusions | No frontend/parser integration, restricted layout/style vocabulary and deferred broader CSS behavior describe milestone limits, not the product ceiling. CSS parsing/cascade does not become a mandatory core dependency. |
| Provisional technical choices | 100,000,000-byte attributable application target; 40 MB empty shell; workload/platform allocations; versioned compatibility profile; initial migration experiment with Pomatez preferred for analysis; Windows-first sequencing followed by bounded macOS validation and Linux work. None is a measured guarantee. |
| Active implementation | **P12 remains the active implementation milestone.** Continue its current scope and independent reviews. Do not dispatch compiler/backend implementation from this decision. Shadcn Gallery remains an initial conformance/reference milestone, not the ultimate feature ceiling. |
| Remaining authority | Founder must authorize publication/merge of these strategic authority documents. Detailed technical decisions and future milestone tasks still need explicit approval. No merge is authorized here. |

The feasibility analysis below and in the companion document remains intact. This record distinguishes the approved destination from proposed means of reaching it; it does not replace existing runtime contracts or rewrite historical v0.1 decisions.

## Product ambition

rust-ui is intended to become a lightweight, high-performance, cross-platform **native desktop application framework** and a practical alternative to Electron and Tauri. Its ambition extends beyond a Rust widget library, a component gallery or a Shadcn implementation.

Developers should be able to author expressive interfaces using familiar HTML, CSS and component definitions, compile them into native applications, and migrate existing desktop web interfaces with progressively less manual work. Familiar authoring must coexist with explicit compatibility reports: familiarity is not a promise that arbitrary websites or JavaScript applications already run unchanged.

```text
HTML + CSS + component definitions + assets
                     |
             rust-ui compiler
                     |
       typed native UI and bindings
                     |
  one retained Rust tree / state / events /
         layout / style / motion runtime
                     |
          native platform backends
          Windows / macOS / Linux
```

Direct Rust authoring remains supported and first-class. Both paths use the same semantic components, retained identity, layout authority and renderer. Shadcn Base Nova is an important frozen reference and initial component system; it is not the only application design rust-ui may express.

## Developer experience to pursue

A developer brings an existing UI, analyzes its HTML/CSS, assets and framework usage, and receives a source-located report of what translates exactly, what needs a native adaptation, and what is unsupported. The compiler emits inspectable native code/data and typed event bindings. A native preview, semantic inspection, source mapping and focused visual comparison support short iteration loops.

The intended progression is:

1. Preserve static structure, styles, assets and accessible intent where the supported profile allows it.
2. Preserve stateful forms, navigation and components through native contracts.
3. Reuse or migrate application logic behind typed boundaries, with human review for semantic changes.
4. Expand compatibility using real migrations, rather than advertising universal conversion before it is demonstrated.

Supported behavior must be predictable. Unsupported selectors, declarations, HTML behavior or browser APIs must never disappear silently. A deliberately accepted adaptation is recorded as such. AI-assisted conversion can reduce manual effort; it does not certify behavioral equivalence.

## Native execution requirements

Shipped applications must not require Chromium, a WebView, a DOM runtime, a JavaScript VM, Node.js, Electron or Tauri. Build-time analysis may use suitable tools; those dependencies must not enter the deployed application automatically. The compiler may construct temporary source trees while compiling. It must not introduce a second retained UI tree in the running application.

The runtime retains these requirements:

- Event-driven updates and presentation; no continuous idle repainting.
- Transactional keyed reconciliation and generation-fenced asynchronous work.
- Typed component, style, layout, event and resource contracts.
- Incremental invalidation with correct old/new damage, including complete invalidation when a global change requires it.
- Bounded queues, caches, native resources and diagnostic output.
- Native keyboard, text editing, clipboard, IME and accessibility.
- Hardware acceleration where it improves the measured workload without excessive memory or resource costs.
- Platform-neutral semantics with one authoritative native realization per platform.

Windows is the current backend. Its Win32, Direct2D/DirectWrite, RichEdit and UIA implementation remains the current authority. The future macOS and Linux backends must realize shared semantics using appropriate platform services, not reproduce Windows terminology in public APIs.

## Desktop scope

The rule remains **desktop-only and fully desktop-responsive**. Window resizing, intrinsic measurement, wrapping, scrolling, compact navigation, split layouts and desktop breakpoints are essential. A fixed reference capture viewport is not a fixed-size product architecture.

Phone/tablet product behavior, iOS/Android, mobile navigation conventions and mobile safe-area contracts remain out of scope. “Desktop-only” must never be used to prohibit responsive layout. See [PRODUCT_SCOPE.md](PRODUCT_SCOPE.md).

## Performance ambition

The founder's target is **at or below 100 MB of total attributable application memory for representative desktop applications**, including application data for those workloads, UI runtime, layout, styles, text, rendering, events, transitions and animation infrastructure. This proposal interprets MB as 1,000,000 bytes; measurements must also publish bytes and, if used, MiB explicitly.

This is a demanding **provisional measurement target**, not a current verified achievement or approved workload guarantee. Framework overhead must leave useful room for the application. The companion proposal defines an empty-shell target, bounded application workloads, native accounting and separate platform allocations without raising the total ceiling for a more expensive backend.

Measure the complete application process family. Charge graphics and shared resources appropriately; do not hide them by reporting only a small Rust heap or one process's working set. Avoid double-counting aliases. Unknown attribution must be reported as unknown, not zero. Development tools have a separate disclosed cost and are absent from normal deployment.

The target does not claim that unlimited documents, images, windows, video, AI models or third-party services can fit under 100 MB. Each claim names the workload, dataset, display scale and enabled capabilities. Expanding these bounds creates a new measured workload; it cannot silently weaken the original target. Native input latency, startup and idle behavior must improve together with memory rather than be traded away through repeated decoding or synchronous work.

## Compatibility direction

HTML and CSS are intended to become meaningful authoring inputs, not decorative syntax over a permanently tiny styling subset. The long-term profile should cover semantic controls, cascade and specificity, custom properties, responsive constraints, useful Flexbox/Grid behavior, typography, visual effects, state styling and motion.

Build-time compilation removes parsing and static resolution from application startup. It cannot precompute every value affected by live content, available size, theme, focus, selection or time. These require efficient native runtime evaluation over the same retained tree.

The product will specify versioned supported semantics. A subset must implement its declared behavior correctly; it must not claim browser conformance based on syntax recognition or screenshots alone. Native editing and accessibility remain platform-owned, including documented restrictions on effects around native text surfaces.

The following are not product promises:

- Transparent conversion of arbitrary JavaScript/TypeScript, React/Vue/Svelte or browser APIs to Rust.
- A browser engine hidden behind compatibility helpers.
- Identical text rasterization on every OS merely because the same font file is supplied.
- Full web-standard coverage, rich-browser editing or arbitrary web content loading in the initial compiler.

## Framework responsibilities beyond widgets

A practical application framework also needs coherent window lifecycle, async effects, resources, navigation conventions, desktop menus/dialogs, application capabilities, packaging and deployment. These should grow from proven application needs. File/network/database functionality belongs in optional application libraries or services, not inside every UI node or stylesheet.

The framework must support useful applications without mandatory devtools listeners, watchers, preview processes or compiler services in production. Signing, installers and update integration are part of eventual product readiness; a compiled window alone is not an Electron/Tauri alternative.

## Relationship to current work

**P12 remains the active implementation milestone and continues independently.** Strategic approval does not interrupt active Workers, approve unreviewed P12 changes, expand its scope, or authorize a compiler implementation. Native rendering correctness is a dependency of the approved product direction.

The current optional-frontend reservation in [STYLE_CUSTOMIZATION_MODEL.md](STYLE_CUSTOMIZATION_MODEL.md#future-optional-authoring-frontends-reserved) is compatible with this direction. Earlier v0.1 exclusions must not be interpreted as permanent product bans on the future capabilities recorded above. Conflicting older policy wording needs a narrowly authorized future amendment; the no-mandatory-browser-runtime and one-tree boundaries remain permanent. Current implementation contracts remain in force until explicitly extended.

The frozen `reference/shadcn-gallery-v0.1/` remains immutable. Compiler experiments use separate fixtures and do not rewrite visual authority to make native comparison pass.

## Approval and durable authority

Planner approval in principle applies to the strategic destination recorded above. It does not approve every technical choice in the companion proposal, authorize implementation of its roadmap, or authorize merging this branch.

Founder publication/merge authorization remains required before these documents are adopted as published strategic authority. After that authorization, a narrowly scoped documentation change should reconcile conflicting current-policy wording, record separately approved technical choices and preserve historical v0.1 decisions as history. No existing policy is modified by this reconciliation.

Planner owns roadmap and milestone acceptance. Lead coordinates bounded implementation; Workers implement it. Astra owns systems/contracts/performance review and Opus owns visual/interaction quality under [REVIEW_AUTHORITY.md](REVIEW_AUTHORITY.md). Existing Universal Gates 1–20 remain authoritative; this proposal adds no new numbered gates.
