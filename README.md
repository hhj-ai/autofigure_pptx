# methodfig

`methodfig` is an MVP Rust CLI for compiling rough paper method descriptions into editable method-overview figures. It writes a one-slide `.pptx` as the source artifact, then exports camera-ready `.pdf` and `.png` for paper use.

The target is a paper architecture / method overview figure, not a presentation deck.

## Why PPTX

The final figure should remain editable in WPS Presentation or PowerPoint. Semantic content is represented as native PPTX text boxes, rectangles, ellipses, lines, arrows, and small PNG assets. `methodfig` forbids full-figure raster generation because a beautiful flattened image is hard to edit and fails the main product invariant.

Small generated assets are allowed only for local pictograms, thumbnails, or texture strips. They must not contain labels, formulas, semantic arrows, watermarks, or the complete figure.

## Runtime Dependencies

- Rust toolchain for building `methodfig`
- Node.js and npm for the PptxGenJS renderer
- LibreOffice / `soffice` for `.pptx` to `.pdf`
- Poppler / `pdftoppm` for `.pdf` to `.png`
- WPS/Windows-friendly fonts such as `Microsoft YaHei`, with `DengXian`, `SimHei`, `SimSun`, `Arial`, or `Calibri` as fallbacks

Install renderer dependencies once:

```bash
cd renderer
npm install
npm run build
```

Check local readiness:

```bash
cargo run -- doctor
```

## Configuration

Copy `.env.example` to `.env` and configure separate OpenAI-compatible providers for each role:

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
```

OpenRouter image generation is used only for small local assets:

```env
METHODFIG_IMAGE_PROVIDER=openrouter
METHODFIG_IMAGE_BASE_URL=https://openrouter.ai/api/v1
METHODFIG_IMAGE_API_KEY=...
METHODFIG_IMAGE_MODEL=...
METHODFIG_IMAGE_MODALITIES=image
```

Use `METHODFIG_IMAGE_MODALITIES=image,text` for OpenRouter models that return both text and images through chat completions. The generated image is still constrained to small local assets; semantic labels remain editable PPTX text.

The MVP includes `--mock-models` for tests and local dry runs. Mock mode does not call real APIs.

## Usage

For a real environment run with automatic output naming, use the helper script:

```bash
bash scripts/run_real_env.sh examples/teacher_student.md
```

The script names the run directory as `<content-summary>_<timestamp>` under `runs/`, uses `--image-provider none`, and leaves the iteration cap to the CLI default instead of hardcoding one in the script.

If you want to call the CLI directly:

```bash
cargo run -- run \
  --method examples/teacher_student.md \
  --out runs/teacher_student \
  --style wps-clean \
  --aspect paper-wide \
  --target-width-mm 85 \
  --max-cost-usd 3.00 \
  --max-minutes 20 \
  --image-provider none \
  --mock-models
```

`--max-cost-usd` is a guardrail for non-mock model calls. The MVP uses conservative per-call estimates to stop before external requests would exceed the cap; mock runs do not consume that budget.

Resume or inspect schemas:

```bash
cargo run -- resume --run runs/teacher_student
cargo run -- schema --print
```

## Output Layout

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
  final/
    figure.pptx
    figure.pdf
    figure.png
    figure_plan.json
    review.json
    status.json
    assets/
```

Open `final/figure.pptx` in WPS Presentation to edit labels, boxes, arrows, colors, and small image assets. The renderer intentionally uses ordinary PPTX objects and avoids SmartArt, animations, 3D effects, glow, heavy shadows, cloud fonts, and full-figure raster images.

## Development Checks

```bash
cargo fmt --check
cargo test
cd renderer && npm run build
```

The mock end-to-end test verifies that the loop fails once, writes a patch plan, passes the next review, and creates final artifacts without real API calls.

Acceptance is intentionally conservative. A figure is accepted only when the vision review clears the score thresholds and the local quality gate finds no collapsed components, major component overlap, degenerate edges, or obvious edge crossings in `layout_map.json`. Low color semantics or aesthetic scores also force rejection.
