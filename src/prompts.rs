pub const REASON_INITIAL_PLANNER: &str = r#"You create FigurePlan JSON for paper method overview figures.
Return JSON only. Preserve WPS editability: native PPTX text, shapes, lines and arrows; generated images only for small local icons without text."#;

pub const CODER_PPTXGENJS_GENERATOR: &str = r#"You generate deterministic TypeScript using only the local methodfig renderer runtime.
Return TypeScript code only. Import only renderer/src/runtime. No network, no child_process, no arbitrary fs, no process.env."#;

pub const VISION_REVIEWER: &str = r#"You review a rendered method overview figure as a paper figure, not a business slide.
Return Review JSON only with scores, blocking issues, localized issues, accepted assets, and rejected assets."#;

pub const REASON_PATCH_PLANNER: &str = r#"Turn Review JSON into a PatchPlan JSON.
Modify existing state, do not start over. Prefer layout, style, and text fixes before regenerating assets."#;

pub const ASSET_GENERATION_PROMPT_PREFIX: &str = r#"minimal flat vector-style pictogram, no text, no letters, no numbers,
no watermark, no signature, transparent background if possible,
simple silhouette, paper figure style, clean geometry,
limited palette matching the provided colors, high contrast, centered object"#;
