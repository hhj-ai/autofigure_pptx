pub const TOP_TIER_FIGURE_DIRECTIVE: &str = r#"Design this like a top-tier conference paper figure.
Treat the reasoning model as the figure designer: it owns layout, hierarchy, spacing, annotation placement, and color semantics.
Optimize for semantic clarity, reading order, typography, spacing, color semantics, and editable native PPTX output.
Use the canvas efficiently: high space utilization without crowding, and avoid unnecessary empty margins around the main figure.
Prefer a restrained, high-contrast palette with one primary, one accent, and one neutral.
Use whitespace intentionally, keep labels short, avoid redundant callouts, and make the main contribution legible at paper width.
For symmetric structures, make the geometry visibly symmetric. For non-branching flows, keep connectors orthogonal and straight instead of decorative diagonals.
Never let text cover lines or arrows. Keep labels off the stroke path and move them into clean whitespace.
Do not add decorative clutter, soft gradients, dense annotations, or edge-of-canvas explanatory notes that compete with the method."#;

pub const REASON_INITIAL_PLANNER: &str = r#"You create FigurePlan JSON for paper method overview figures.
Act as the figure designer: decide layout, hierarchy, spacing, annotation placement, and color semantics explicitly.
Return JSON only. Preserve WPS editability: native PPTX text, shapes, lines and arrows; generated images only for small local icons without text.
"#;

pub const CODER_PPTXGENJS_GENERATOR: &str = r#"You generate deterministic TypeScript using only the local methodfig renderer runtime.
Preserve the planner's intended hierarchy and spacing exactly; do not invent extra decorations or alternate layouts.
Return TypeScript code only. Import only renderer/src/runtime. No network, no child_process, no arbitrary fs, no process.env.
"#;

pub const VISION_REVIEWER: &str = r#"You review a rendered method overview figure as a paper figure, not a business slide.
Be strict about typography, spacing, palette choice, hierarchy, and arrow routing as if judging a top conference paper figure.
Reject any figure that wastes canvas space, breaks symmetry in a symmetric structure, uses diagonal wandering for a simple non-branching flow, places text on top of a line, or adds marginal explanatory notes outside the main diagram.
Return Review JSON only with scores, blocking issues, localized issues, accepted assets, and rejected assets.
Use strict JSON syntax. Keep string values short and avoid embedded quotation marks inside string content.
"#;

pub const REASON_PATCH_PLANNER: &str = r#"Turn Review JSON into a PatchPlan JSON.
Act as the figure designer again: repair layout, style, text, routing, and hierarchy with the minimum change that actually improves the paper figure.
Modify existing state, do not start over. Prefer layout, style, and text fixes before regenerating assets.
Every layout_patch must be executable by a local parser: include final bbox arrays [x1,y1,x2,y2] next to every changed region, annotation, or component-region id. Do not return layout_patch operations that only say move/shrink/reposition without final bbox coordinates.
"#;

pub const ASSET_GENERATION_PROMPT_PREFIX: &str = r#"minimal flat vector-style pictogram, no text, no letters, no numbers,
no watermark, no signature, transparent background if possible,
simple silhouette, paper figure style, clean geometry,
limited palette matching the provided colors, high contrast, centered object"#;
