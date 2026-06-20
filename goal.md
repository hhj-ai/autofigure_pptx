# methodfig goal

You are Codex. Build an MVP Rust CLI called `methodfig`.

`methodfig` is a paper method-overview figure compiler. It takes a Markdown file containing a rough method section or method description, runs an agentic refinement loop, and outputs an editable `.pptx` figure plus camera-ready `.pdf` and `.png`. The target is a paper method overview / architecture figure, not a presentation deck. PPTX is the editable source format so the user can later open and modify the result in WPS Presentation or PowerPoint.

## References to inspect

Use these projects as reference material, not as code to copy blindly.

- https://github.com/Haojae/scipilot-figure-skill
  - Reference for publication-grade figure skill thinking, journal-style constraints, and CJK/font handling.
- https://github.com/ResearAI/AutoFigure
- https://github.com/ResearAI/AutoFigure-Edit
  - Reference for method text → editable scientific illustration and iterative refinement. Our output is PPTX, not SVG.
- https://github.com/OpenDCAI/Paper2Any
  - Closest existing direction: Paper2Figure supports model architecture diagrams / technical route diagrams with PPT + SVG and editable PPTX output.
- https://github.com/Noi1r/powerpoint-skill
  - Reference for a PptxGenJS-based PowerPoint skill for academic material.
- https://github.com/MiniMax-AI/skills/blob/main/skills/pptx-generator/SKILL.md
  - Reference for PPTX generation/editing workflow and PptxGenJS conventions.
- https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/blob/main/skills/paper-slides/SKILL.md
- https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep/blob/main/skills/paper-figure/SKILL.md
  - Reference for paper-writing workflows where hero/method figures are distinct from data plots.
- https://github.com/w1163222589-coder/slide-image-to-editable-pptx
  - Reference for decomposing a visual into native PPT shapes, editable text, and image assets.
- https://github.com/gitbrent/PptxGenJS
- https://gitbrent.github.io/PptxGenJS/docs/api-shapes/
  - PptxGenJS is the PPTX backend.
- https://openrouter.ai/docs/guides/overview/multimodal/image-generation
  - OpenRouter image generation uses chat completions / responses and `modalities: ["image"]` or `["image", "text"]`.
- https://www.wps.com/office/presentation/
- https://www.wps.com/academy/ppt-presentation/
  - WPS Presentation supports opening/editing/saving `.pptx`; design for human editing in WPS.
- https://learn.microsoft.com/en-us/typography/font-list/microsoft-yahei
- https://learn.microsoft.com/en-us/typography/fonts/windows_11_font_list
  - Use Windows/WPS-friendly fonts. Microsoft YaHei is legible at small sizes and common on Windows; DengXian/SimHei are also common Simplified Chinese fonts.

## Core product decision

This is not a “one-shot image generator”. The whole figure must remain editable.

Architecture:

```text
method.md
  -> reasoner creates / updates FigurePlan JSON
  -> agent decides whether small generated assets are needed
  -> image model generates only local small assets, never the full figure
  -> coder writes PptxGenJS TypeScript from FigurePlan + assets
  -> local Node renderer creates figure.pptx
  -> exporter creates figure.pdf and figure.png
  -> vision model reviews rendered PNGs
  -> reasoner creates PatchPlan
  -> agent routes each patch to the right executor
  -> repeat until accepted
```

The main loop should not use a fixed user-facing `--rounds`. It should iterate until review passes. Still implement safety caps:

```bash
methodfig run \
  --method path/to/method.md \
  --out runs/example \
  --style wps-clean \
  --max-iterations 12 \
  --max-cost-usd 3.00 \
  --max-minutes 20
```

The hidden caps are only guardrails. The normal control logic is “until pass”.

## Language and runtime

Use Rust for the main program.

Reason: this is a CLI/orchestration tool. Rust can ship as a mostly self-contained binary and avoids making users manage Python environments. Do not make Python a required runtime. Python can appear only in optional developer scripts, never in the critical user path.

Use TypeScript/Node only for the PPTX backend because PptxGenJS is the right library for editable PPTX generation.

Required runtime dependencies for MVP:

- `methodfig` Rust binary
- Node.js for the renderer
- LibreOffice / `soffice` for PPTX -> PDF
- Poppler / `pdftoppm` for PDF -> PNG

Add `methodfig doctor` to check Node, `soffice`, `pdftoppm`, fonts, and `.env` model configuration.

## Rust crates

Suggested crates:

- `clap` for CLI
- `dotenvy` for `.env`
- `serde`, `serde_json`, `schemars` for schemas
- `reqwest`, `tokio` for OpenAI-compatible HTTP calls
- `anyhow`, `thiserror` for errors
- `tracing`, `tracing-subscriber` for logs
- `uuid`, `sha2` for run IDs and asset cache keys
- `base64`, `mime_guess` for vision/image payloads

## Project layout

```text
methodfig/
  Cargo.toml
  .env.example
  README.md
  src/
    main.rs
    cli.rs
    config.rs
    agent.rs
    pipeline.rs
    schema.rs
    style.rs
    llm/
      mod.rs
      chat_provider.rs
      openai_compatible.rs
      openrouter_image.rs
      openai_images.rs
      replicate.rs
      mock.rs
    tools/
      asset_gen.rs
      pptx_codegen.rs
      render.rs
      export.rs
      review.rs
      doctor.rs
      validate.rs
    prompts.rs
  renderer/
    package.json
    tsconfig.json
    src/
      runtime.ts
      safe_api.ts
  examples/
    teacher_student.md
    multimodal_fusion.md
    pipeline.md
```

Prompts must be embedded in the Rust binary, not read as runtime files. Use Rust constants in `prompts.rs`, or use `include_str!()` if prompt files are included at compile time. After installation, the binary must not depend on a `prompts/` directory.

## CLI

MVP commands:

```bash
methodfig run --method method.md --out runs/name --style wps-clean
methodfig doctor
methodfig schema --print
methodfig resume --run runs/name
```

Important flags:

```text
--method PATH                  Markdown input
--out DIR                      Run directory
--style wps-clean|cvpr-clean|neurips-minimal
--aspect paper-wide|single-column|double-column|16:9
--target-width-mm 85|180       Review readability at paper width
--max-iterations N
--max-cost-usd FLOAT
--max-minutes N
--image-provider openrouter|openai_images|replicate|none
--mock-models                  For tests and local dry runs
--keep-intermediate            Keep every round artifact
```

## Environment variables

`.env` should allow separate OpenAI-compatible configs for each role:

```env
METHODFIG_REASONER_BASE_URL=https://api.openai.com/v1
METHODFIG_REASONER_API_KEY=...
METHODFIG_REASONER_MODEL=...

METHODFIG_CODER_BASE_URL=https://api.openai.com/v1
METHODFIG_CODER_API_KEY=...
METHODFIG_CODER_MODEL=...

METHODFIG_VISION_BASE_URL=https://api.openai.com/v1
METHODFIG_VISION_API_KEY=...
METHODFIG_VISION_MODEL=...

METHODFIG_IMAGE_PROVIDER=openrouter
METHODFIG_IMAGE_BASE_URL=https://openrouter.ai/api/v1
METHODFIG_IMAGE_API_KEY=...
METHODFIG_IMAGE_MODEL=...
```

All role providers should implement traits so the same OpenAI-compatible provider can be reused for all roles if desired.

## OpenRouter image support

Implement an `OpenRouterImageProvider`.

OpenRouter image generation should use `/api/v1/chat/completions` with `modalities`.

For image-only models, especially Flux-like models:

```json
{
  "model": "...",
  "messages": [
    {
      "role": "user",
      "content": "Generate a minimal flat vector-style icon..."
    }
  ],
  "modalities": ["image"],
  "stream": false,
  "image_config": {
    "aspect_ratio": "1:1",
    "image_size": "1K"
  }
}
```

For models that output both text and image, support `modalities: ["image", "text"]`.

Parse returned images from `choices[0].message.images[*].image_url.url`. The value may be a base64 data URL. Save generated assets as PNG files under the round asset directory.

Image generation quality does not need to be premium. These assets are small local icons/textures. Prefer cheap, fast, simple outputs over beautiful but hard-to-match outputs.

## Image model constraints

The image model must never generate the entire method overview. It can only generate small local visual assets.

Allowed asset types:

- flat icon
- symbolic pictogram
- tiny environment thumbnail
- abstract texture strip
- dataset/task thumbnail
- module icon, such as encoder, memory, robot, agent, graph, token, latent space

Forbidden for image assets:

- semantic labels
- letters
- formulas
- arrows that carry meaning
- full pipeline diagrams
- screenshots pretending to be the full figure
- watermark/signature
- photorealistic clutter unless explicitly requested by the asset spec

Default asset prompt constraints:

```text
minimal flat vector-style pictogram, no text, no letters, no numbers,
no watermark, no signature, transparent background if possible,
simple silhouette, paper figure style, clean geometry,
limited palette matching the provided colors, high contrast, centered object
```

If transparency is not supported, require white background and have the renderer treat it as an image tile with no border.

Cache assets by hash of `AssetSpec`. If an asset is acceptable in review, reuse it in later rounds.

## WPS/PPTX human editability constraints

This part is critical. The final `.pptx` must be comfortable to edit in WPS Presentation.

Use only normal PPTX objects that WPS users can edit from the toolbar:

- native text boxes
- native rectangles / rounded rectangles
- native ellipses
- native straight lines / elbow-like routed lines
- native arrows
- native dashed lines
- native PNG images for small assets only

Avoid features that often make cross-editor editing fragile:

- no SmartArt
- no animations/transitions
- no video/audio
- no 3D effects
- no glow/bevel/heavy shadows
- no gradient fills unless a style explicitly allows it
- no complex SVG-as-image for important editable content
- no text baked into images
- no full-figure raster image as the main figure
- no cloud fonts or custom bundled fonts
- no font embedding requirement
- no exotic line caps/joins that are hard to edit

### WPS-friendly fonts

Default style `wps-clean` should use:

```json
{
  "font_cjk": "Microsoft YaHei",
  "font_cjk_display_name": "微软雅黑",
  "font_latin": "Arial",
  "font_mono": "Consolas",
  "fallback_cjk": ["DengXian", "SimHei", "SimSun"],
  "fallback_latin": ["Calibri", "Arial"]
}
```

Rationale:

- WPS Presentation supports `.pptx` editing and saving.
- Microsoft YaHei / 微软雅黑 is common on Windows and is designed for Simplified Chinese on-screen readability.
- Windows also commonly includes DengXian / 等线 and SimHei / 黑体 for Simplified Chinese.
- Arial is a conservative Latin fallback.
- Avoid Aptos as the default even though it is current in Microsoft Office, because WPS/LibreOffice availability and substitution may be inconsistent across user machines.
- Avoid cloud fonts and decorative fonts.

Font rules:

- Use `Microsoft YaHei` for all labels by default, even English labels, unless style says otherwise. This keeps mixed Chinese/English figures consistent.
- Use `Arial` only for purely English technical labels if the style opts in.
- Use bold sparingly for module names.
- Avoid italics.
- Avoid tiny text. Review at the requested target paper width.
- Default label font sizes should be conservative for paper figures:
  - module labels: 8.5–10 pt
  - auxiliary labels: 7–8 pt
  - section/group labels: 10–12 pt
  - do not use less than 6.5 pt without a blocking warning
- Keep text labels short: normally <= 6 words or <= 12 Chinese characters per box.
- Simple math should use Unicode text if possible. Complex formulas should be avoided in the method overview or added as editable text labels, not rasterized by default.

### WPS-friendly lines and arrows

Use line widths that WPS users can easily edit and that survive export:

```text
auxiliary line: 0.75 pt
normal line:    1.00 pt
main flow:      1.50 pt
strong focus:   2.00 pt
```

Avoid lines thinner than 0.75 pt.

Use only these line styles in the default style:

```text
solid
dash
long dash
```

Semantic convention:

- solid arrow = data/control flow
- dashed arrow = loss/supervision/reference signal
- thin muted line = context/background connection
- thick accent line = main contribution path

Arrow rules:

- Use native PPTX arrowheads via PptxGenJS-supported line options.
- Prefer simple triangular end arrows.
- Avoid custom polygon arrows unless absolutely necessary.
- Avoid crossing arrows. If crossing is unavoidable, reroute through lanes or split the layout.
- Do not place text on top of arrows; use small nearby labels.

Shape rules:

- Main modules: rounded rectangles with simple fill + outline.
- Use corner radius consistently.
- Use flat colors only.
- No shadow by default.
- No transparent text.
- Transparency only for background blocks or muted context, max 15–20%.

## Styles

Implement at least:

- `wps-clean`: default, WPS-friendly, uses Microsoft YaHei + Arial, simple lines, muted palette.
- `cvpr-clean`: similar but slightly more academic/compact.
- `neurips-minimal`: monochrome-leaning, more whitespace.

Each style should define:

```json
{
  "fonts": {},
  "palette": {
    "background": "FFFFFF",
    "text": "1F2328",
    "muted_text": "5B6770",
    "stroke": "A8B0B8",
    "muted_fill": "F4F6F8",
    "primary": "...",
    "accent": "...",
    "warning": "..."
  },
  "line_widths": {},
  "corner_radius": {},
  "spacing": {},
  "font_sizes": {}
}
```

The style validator must reject or warn about:

- more than 4 semantic colors plus background/text
- unsupported font names
- too many font sizes
- line widths below 0.75 pt
- image assets with text
- objects not mapped to stable IDs

## Schemas

Create strongly typed schemas with `serde` and optionally export JSON Schema via `methodfig schema --print`.

### FigurePlan

Required fields:

```json
{
  "version": "0.1",
  "canvas": {
    "aspect": "paper-wide",
    "target_width_mm": 85,
    "safe_margin": 0.06
  },
  "story": {
    "main_message": "...",
    "visual_focus": ["..."],
    "reading_order": "left_to_right"
  },
  "layout": {
    "template": "pipeline|teacher_student|multimodal_fusion|training_inference_split|module_zoom_in",
    "grid": {"columns": 12, "rows": 6},
    "regions": []
  },
  "components": [
    {
      "id": "student_model",
      "label": "Student LM",
      "role": "main|context|input|output|loss|data|module",
      "visual_weight": "strong|normal|muted",
      "region": "center",
      "allowed_asset_id": "student_icon"
    }
  ],
  "edges": [
    {
      "id": "latent_supervision",
      "from": "teacher",
      "to": "student",
      "label": "latent residual",
      "semantic": "data_flow|supervision|loss|feedback|reference",
      "style": "solid|dash|long_dash",
      "importance": "main|normal|aux"
    }
  ],
  "annotations": [],
  "assets": [],
  "design": {
    "style": "wps-clean",
    "max_colors": 4,
    "font_policy": "wps_friendly",
    "avoid_arrow_crossing": true,
    "prefer_native_shapes": true
  }
}
```

Every component, edge, annotation, asset, and layout region must have a stable `id`.

### AssetSpec

```json
{
  "id": "vision_encoder_icon",
  "type": "generated_icon|generated_texture|imported",
  "prompt": "...",
  "negative_prompt": "text, letters, numbers, watermark, signature, photorealistic clutter",
  "usage": "inside_component|background_decoration|thumbnail",
  "size": "small|wide_strip|thumbnail",
  "transparent_background": true,
  "style_constraints": {
    "flat": true,
    "no_text": true,
    "match_palette": "wps-clean"
  },
  "status": "missing|generated|accepted|needs_regeneration"
}
```

### Review

The vision reviewer sees:

- clean high-resolution `figure.png`
- target paper-width preview, e.g. `figure_85mm_preview.png`
- optional `figure_review_overlay.png` with small numeric IDs / bounding boxes
- `layout_map.json` mapping object IDs to normalized bounding boxes

Review output:

```json
{
  "passed": false,
  "scores": {
    "semantic_fidelity": 8,
    "story_clarity": 7,
    "visual_hierarchy": 6,
    "paper_readability": 5,
    "layout_cleanliness": 7,
    "arrow_routing": 6,
    "color_semantics": 8,
    "aesthetic_quality": 6,
    "wps_editability": 9
  },
  "blocking_issues": [
    "Target width readability fails for labels in the bottom branch."
  ],
  "localized_issues": [
    {
      "target_id": "loss_arrow",
      "bbox": [0.12, 0.78, 0.58, 0.86],
      "severity": "blocking|major|minor",
      "issue": "Arrow crosses the main data path.",
      "evidence": "The dashed loss arrow intersects the student path.",
      "suggested_direction": "Move loss arrow to bottom lane."
    }
  ],
  "accepted_assets": ["vision_encoder_icon"],
  "rejected_assets": [
    {
      "asset_id": "robot_icon",
      "reason": "Too photorealistic and does not match flat style."
    }
  ]
}
```

### PatchPlan

The reasoner turns review into concrete operations:

```json
{
  "operations": [
    {
      "id": "op_001",
      "target_id": "student_block",
      "executor": "reasoner|coder|image_model",
      "operation_type": "layout_patch|style_patch|text_patch|asset_regeneration|edge_reroute",
      "action": "Increase width by 15% and make it the dominant central module.",
      "expected_effect": "Main contribution becomes visually dominant."
    }
  ],
  "stop_reason": "continue|accepted|cap_reached"
}
```

Patch routing:

- `reasoner`: revises FigurePlan story/layout/design semantics.
- `coder`: changes TypeScript/PptxGenJS implementation while preserving plan semantics.
- `image_model`: regenerates only the specific rejected asset.
- `agent`: runs local tools, validation, render/export, cache, resume.

Do not regenerate everything from scratch after review. Preserve state and apply patches. Every round should have lineage back to the previous round.

## Prompts to embed

Create prompt constants for:

- reason initial planner
- coder PptxGenJS generator
- vision reviewer
- reason patch planner
- asset generation prompt builder

High-level behavior:

### Reasoner initial planner

Input: method Markdown + style + target width.

Output: `FigurePlan` JSON only.

Rules:

- Extract the method story.
- Decide the diagram template.
- Identify main contribution and visual focus.
- Decide what should be native shapes vs generated small assets.
- Keep labels short.
- Include WPS editability constraints in the plan.
- Use stable IDs.
- Do not write code.

### Coder PptxGenJS generator

Input: `FigurePlan`, style tokens, asset paths, previous TypeScript if patching.

Output: TypeScript code only.

Rules:

- Generate a one-slide PPTX.
- Use PptxGenJS native shapes/text/lines.
- Preserve IDs in code comments and layout map.
- Use WPS-friendly fonts and line styles.
- No semantic text in images.
- No network calls.
- No reading arbitrary files except supplied asset paths.
- Emit `layout_map.json` with normalized bounding boxes.
- Keep code deterministic.

### Vision reviewer

Input: rendered images + layout map + plan.

Output: `Review` JSON only.

Rules:

- Review as a paper figure, not as a business slide.
- Judge semantic fidelity, story clarity, hierarchy, readability at target paper width, layout cleanliness, arrow routing, color semantics, aesthetic quality, WPS editability.
- Localize issues by stable ID when possible.
- Mark blocking issues clearly.
- Accept/reject individual generated assets.

### Patch planner

Input: previous `FigurePlan`, previous `Review`, previous `PatchPlan` if any.

Output: `PatchPlan` JSON only.

Rules:

- Modify existing state, do not start over.
- Route issues to the executor best suited to fix them.
- Keep accepted assets unchanged.
- Keep story changes minimal unless semantic fidelity fails.
- Prefer layout/style/text fixes before regenerating assets.

## Renderer security

The coder model produces TypeScript, so run it carefully.

MVP security requirements:

- Generated TypeScript should import only the local renderer runtime / PptxGenJS wrapper.
- No arbitrary `fs` except writing expected output files and reading whitelisted asset paths.
- No network.
- No child processes from generated code.
- Run Node with a timeout.
- Run from a temporary round directory.
- Validate that the expected files were created:
  - `figure.pptx`
  - `layout_map.json`
  - optionally `figure.ts.log`

A lightweight static scan is acceptable for MVP: reject code containing suspicious imports such as `child_process`, `http`, `https`, `net`, `dns`, or raw environment access.

## Export pipeline

Renderer creates PPTX.

Then:

```bash
soffice --headless --convert-to pdf --outdir round_dir figure.pptx
pdftoppm -png -r 300 figure.pdf figure
```

Also create review images:

- `figure.png` high resolution
- `figure_85mm_preview.png` or target width preview
- `figure_review_overlay.png` with IDs/bounding boxes
- `layout_map.json`

Review should look at the target-width preview because the figure is meant for papers.

## Output structure

```text
runs/name/
  input.md
  config_snapshot.json
  run.log
  round_000/
    figure_plan.json
    assets/
    figure.ts
    figure.pptx
    figure.pdf
    figure.png
    figure_85mm_preview.png
    figure_review_overlay.png
    layout_map.json
    review.json
    patch_plan.json
  round_001/
    ...
  final/
    figure.pptx
    figure.pdf
    figure.png
    figure_plan.json
    review.json
    assets/
```

If caps are reached without pass, still create `final/` from the best available round and write `final/status.json` with `accepted: false` and reasons.

## Acceptance thresholds

Default pass logic:

```text
blocking_issues is empty
semantic_fidelity >= 8
story_clarity >= 8
visual_hierarchy >= 8
paper_readability >= 8
layout_cleanliness >= 8
arrow_routing >= 8
aesthetic_quality >= 7
wps_editability >= 9
```

WPS editability should be strict. A beautiful rasterized figure is failure.

## Templates to support first

Implement these five as plan templates, even if only three have full deterministic helpers in MVP:

1. `pipeline`
2. `teacher_student`
3. `multimodal_fusion`
4. `training_inference_split`
5. `module_zoom_in`

Template semantics:

- `pipeline`: left-to-right method flow.
- `teacher_student`: teacher/context branch muted, student/main branch emphasized, dashed supervision/loss.
- `multimodal_fusion`: multiple input branches merge into shared module, outputs on right.
- `training_inference_split`: top/bottom or left/right split with training-only loss arrows muted.
- `module_zoom_in`: main pipeline plus inset box explaining internal module.

## MVP implementation order

1. Scaffold Rust CLI and `.env.example`.
2. Define schemas and export JSON schema.
3. Implement provider traits and mock providers.
4. Implement embedded prompt constants.
5. Implement run directory state machine.
6. Implement TypeScript renderer wrapper with one deterministic example.
7. Implement model-generated TypeScript path with basic safety scan.
8. Implement PPTX -> PDF -> PNG export.
9. Implement mock review loop that fails once and passes after a patch.
10. Implement OpenAI-compatible chat provider for reasoner/coder/vision.
11. Implement OpenRouter image provider for small assets.
12. Implement `wps-clean` style validator.
13. Implement `doctor`.
14. Add examples and README.

## Testing

Add tests for:

- `.env` parsing
- schema serialization/deserialization
- style validation
- WPS font/line policy warnings
- asset cache hashing
- patch routing
- run resume
- mock end-to-end pipeline
- generated-code safety scan

End-to-end mock test should not call real APIs.

## README expectations

README should explain:

- what problem this solves
- why PPTX is the source format
- why full-figure image generation is forbidden
- why Rust is the main language
- how to configure OpenAI-compatible models
- how to use OpenRouter/Flux-like image models for small assets
- what dependencies are needed
- how to run `doctor`
- how to inspect round artifacts
- how to open final PPTX in WPS and manually edit it

## Product taste

The output should look like a paper figure, not a corporate slide.

Good default appearance:

- clean white background
- small number of colors
- strong main path
- muted context branches
- short labels
- readable at single-column width
- no decorative clutter
- no large title unless useful
- no slide-like bullet lists
- no unnecessary legend if colors/lines are obvious
- generated icons are small and quiet

The most important invariant: semantic content, layout, typography, and assets are all editable or replaceable after generation. The user should be able to open `figure.pptx` in WPS Presentation and continue editing without fighting the file.
