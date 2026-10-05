# Review authority for UI milestones

UI-related rust-ui milestones use a split review. One reviewer must not
certify both sides.

| Reviewer | Owns |
|---|---|
| **Opus 5.5** (UI design authority) | visual fidelity, shadcn parity, component composition, spacing, typography, visual states, screenshots, visual regression, interaction feel, UI taste/polish |
| **Astra Advisor** | architecture, API/contracts, exporter correctness, semantic IDs, deterministic behavior, reproducibility, failure semantics, concurrency, lifecycle, platform correctness, accessibility semantics/contracts, performance/resource behavior, tooling, provenance, licensing, dependency/scope hygiene |

- Astra is not the visual taste authority.
- Opus is not the concurrency/platform/backend correctness authority.
- Findings outside a reviewer's column are advisory and are resolved by the
  owning reviewer.
- Non-UI milestones (platform foundation, runtime, backends) remain under
  `QUALITY_GATES.md` with Astra as reviewer.
