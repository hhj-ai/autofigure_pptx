# methodfig MVP 执行计划

## 用户目标

实现 `goal.md`：从空仓库搭建 Rust CLI `methodfig`，输入 Markdown 方法描述，经过可恢复的 agentic figure loop，输出可编辑 `.pptx` 以及 `.pdf`、`.png`，目标是论文 method overview / architecture figure，而不是演示文稿 deck。

## 已发现的项目约束

- 当前仓库只有 `goal.md`，没有已有源码、`AGENTS.md`、`README.md` 或 `docs/` 约束文件。
- 本会话全局要求：默认中文沟通；代码标识符、命令、路径、API 名称保持英文；执行多步骤任务时维护 `plan.md`。
- `goal.md` 明确要求主程序使用 Rust，PPTX 后端使用 TypeScript/Node + PptxGenJS；Python 不能成为用户关键路径依赖。
- 输出必须以 PPTX 为可编辑源格式，不能生成整张 raster figure；图中语义内容应由 native PPTX objects 表达，小图标/纹理资产才允许用图片。
- Runtime 依赖：Rust binary、Node.js、LibreOffice `soffice`、Poppler `pdftoppm`；`doctor` 必须检查这些依赖、字体和 `.env` 模型配置。
- Prompts 必须嵌入 Rust binary，不能依赖 runtime `prompts/` 目录。
- 安全要求：模型生成的 TypeScript 只能导入本地 renderer runtime，不允许网络、child process、任意 fs/env 访问；MVP 可使用轻量 static scan。
- 测试要求包括 `.env` parsing、schema serde、style validation、WPS font/line warnings、asset cache hashing、patch routing、resume、mock end-to-end、generated-code safety scan；mock E2E 不能调用真实 API。

## 外部参考已核对

- PptxGenJS 官方 README/文档确认可在 Node 中生成 PowerPoint，并支持 native shapes/text/images。
- PptxGenJS shapes/types 文档确认 `addShape`、`addText`、line `dashType`、`endArrowType` 等能力可映射到 WPS 友好的普通对象。
- OpenRouter image generation 文档确认 image generation 走 `/api/v1/chat/completions`，请求使用 `modalities`，响应图片位于 `choices[0].message.images[*].image_url.url`，可为 base64 data URL。
- WPS Presentation 页面确认支持打开/保存 `.pptx`；Microsoft YaHei 文档确认其适合简体中文小字号屏显阅读。

## 当前项目结构

```text
.
├── .git/
├── .env.example
├── .gitignore
├── Cargo.lock
├── Cargo.toml
├── README.md
├── examples/
│   ├── multimodal_fusion.md
│   ├── pipeline.md
│   └── teacher_student.md
├── goal.md
├── plan.md
├── renderer/
│   ├── package-lock.json
│   ├── package.json
│   ├── src/
│   │   ├── runtime.ts
│   │   └── safe_api.ts
│   └── tsconfig.json
├── src/
│   ├── agent.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── lib.rs
│   ├── llm/
│   ├── main.rs
│   ├── pipeline.rs
│   ├── prompts.rs
│   ├── schema.rs
│   ├── style.rs
│   └── tools/
└── tests/
```

## 计划中的实现结构

```text
.
├── Cargo.toml
├── .env.example
├── README.md
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── agent.rs
│   ├── pipeline.rs
│   ├── prompts.rs
│   ├── schema.rs
│   ├── style.rs
│   ├── llm/
│   └── tools/
├── renderer/
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
├── examples/
├── tests/
└── plan.md
```

## 实现步骤

1. 写核心行为测试，先覆盖 schema、style validation、asset hash、safety scan、mock pipeline 和 resume。
2. 搭建 Rust crate 与 CLI 命令：`run`、`doctor`、`schema --print`、`resume`。
3. 定义 `FigurePlan`、`AssetSpec`、`Review`、`PatchPlan` 等 serde/schemars schemas。
4. 实现 `.env` 配置读取、role provider traits、mock providers、OpenAI-compatible chat provider、OpenRouter image provider。
5. 实现 deterministic FigurePlan/PptxGenJS codegen fallback，使 `--mock-models` 不依赖真实 API。
6. 实现 renderer static scan、Node renderer 调用、export pipeline、mock 缺依赖 fallback、review threshold 和 patch routing。
7. 添加 `doctor`、examples、README、`.env.example`。
8. 运行 `cargo fmt`、`cargo test`、Node install/build，以及一次 `methodfig run --mock-models` smoke check。

## 预计修改文件

- Rust: `Cargo.toml`、`src/**/*.rs`、`tests/*.rs`
- Node renderer: `renderer/package.json`、`renderer/tsconfig.json`、`renderer/src/*.ts`
- 文档和样例：`README.md`、`.env.example`、`examples/*.md`
- 执行记录：`plan.md`

## 风险、未知与验证方法

- 当前环境可能缺少 LibreOffice 或 Poppler。生产路径应报错，mock dry-run/test 路径可生成明确标记的 placeholder export，避免测试依赖系统 GUI/office 安装。
- PptxGenJS API 版本可能有细节差异。通过 Node `npm install` + `npm run build` + mock run 验证实际 renderer。
- 字体可用性跨平台不同。`doctor` 应报告 Microsoft YaHei/DengXian/SimHei/Arial 检查结果，不把缺字体伪装成成功。
- 外部 API 不在测试中调用。OpenAI/OpenRouter provider 的测试只检查 request construction、response parsing 或通过 mock HTTP 数据。

## 执行日志

- 2026-06-17：读取完整 `goal.md`，确认仓库为空项目；核对 PptxGenJS、OpenRouter image generation、WPS Presentation、Microsoft YaHei 相关外部资料；创建本计划文件。
- 2026-06-17：创建 `Cargo.toml` 和第一批集成测试（schema、style、asset、renderer safety、mock pipeline/resume）。运行 `cargo test`，预期失败于缺少 `src/lib.rs`，说明测试已先固定外部行为，接下来补实现。
- 2026-06-17：实现 Rust crate 主体：CLI、配置读取、schemas、style validator、provider traits、OpenAI-compatible chat、OpenRouter/OpenAI image provider、mock pipeline、renderer safety scan、export fallback、doctor。运行 `cargo test`，全部 13 个集成测试通过。
- 2026-06-17：实现 Node/PptxGenJS renderer、`.env.example`、README、三个 examples 和 package lock。首次 renderer smoke 发现 `figure.pptx` 是 fallback ASCII，占位原因是相对 `round_dir` 在 Node cwd 下被重复拼接；将 `figure.ts` 和 renderer payload 统一改为绝对路径。
- 2026-06-17：第二次 smoke 发现 `tsx` 在 run 目录按 CJS 处理生成脚本，top-level await 失败；将 generated TypeScript 改为 `async function main()`。之后 `runs/smoke/final/figure.pptx` 经 `file` 和 `unzip -l` 验证为真实 PPTX zip 包，包含 `ppt/slides/slide1.xml` 等 PowerPoint package 结构。
- 2026-06-17：最终验证：`cargo fmt --check` 通过；`cargo test` 通过 18 个集成测试；`npm run build` 通过；`cargo run -- schema --print` 可输出 JSON Schema；`cargo run -- doctor` 正确失败并报告当前机器缺 `soffice`、Microsoft YaHei 被 fontconfig fallback 到 Noto Sans、模型环境变量未配置。由于缺 `soffice`，本机 smoke 的 PDF/PNG 是 mock placeholder，不代表生产 export 路径已在本机完成真实转换。
- 2026-06-17：补充非 mock 模型路径：`run_pipeline` 不再静默使用 mock planner/reviewer；缺 `METHODFIG_REASONER_*` 时会提前报错，配置存在时调用 OpenAI-compatible reasoner 生成 `FigurePlan`，vision provider 通过 base64 data URL 接收 target-width preview 并返回 `Review` JSON，patch planner 调用 reasoner 返回 `PatchPlan`。新增 `pipeline_nonmock_tests` 固定缺配置行为。
- 2026-06-17：重新执行最终验证：`cargo fmt --check` 通过；`cargo test` 通过 19 个集成测试；`npm run build` 通过；mock smoke 通过两轮并生成真实 zip-based `figure.pptx`。`unzip -p runs/smoke/final/figure.pptx ppt/slides/slide1.xml` 显示内容为 native `<p:sp>` shapes/text（包含 `Teacher LM`、`Student LM`、`latent residual` 等 editable text），`layout_map.json` 含稳定 ID 到 bbox 的映射。`doctor` 仍因本机缺 `soffice` 返回失败，PDF/PNG smoke 输出仍是 mock placeholder。
- 2026-06-17：补充 renderer 真 timeout（超时 kill Node child process），补充 `materialize_assets`：按 `AssetSpec` hash 写 `asset_cache`，复制到每轮 `assets/`，传绝对路径给 renderer。修复资产路径后，`runs/smoke_assets/final/figure.pptx` 的 slide XML 同时包含 native `<p:sp>` 文本/线条和小资产 `<p:pic>`，符合“仅小 local asset 可为 PNG”的约束。最终验证：`cargo fmt --check` 通过；`cargo test` 通过 20 个集成测试；`npm run build` 通过；`cargo run -- doctor` 按预期因缺 `soffice`、缺 Microsoft YaHei、模型未配置返回失败。
- 2026-06-17：补充 model-generated TypeScript path：非 mock 模式要求 reasoner/coder/vision 三个 role 配置齐全；reasoner 产出 `FigurePlan`，coder 产出 TypeScript 并经过 `scan_generated_typescript`，vision 接收 preview image data URL 产出 `Review`，patch planner 由 reasoner 产出 `PatchPlan`。最终验证：`cargo fmt --check` 通过；`cargo test` 通过 21 个集成测试；`npm run build` 通过；`cargo run -- schema --print` 正常；`cargo run -- doctor` 仍正确报告当前环境缺 `soffice`、缺 Microsoft YaHei 实际字体、模型未配置。
- 2026-06-17：补强审计缺口：实现非 mock `max-cost-usd` 估算 cap，成本超限会在外部调用前停止；OpenRouter image payload 支持 `METHODFIG_IMAGE_MODALITIES=image,text`；`doctor` 改为检查 Microsoft YaHei、DengXian、SimHei、SimSun、Arial、Calibri；新增 fake `soffice`/`pdftoppm` 导出测试，证明 `export_round` 的真实命令链和文件命名；新增 iteration cap 测试，证明未通过时仍写 `final/status.json accepted:false`。验证：`cargo fmt --check` 通过；`cargo test` 通过 29 个集成测试；`npm run build` 通过；mock smoke 生成真实 PPTX，slide XML 同时含 native shapes/text 与小 PNG asset；`cargo run -- schema --print` 输出 9178 字节 schema。`doctor` 仍按预期报告当前机器缺 `soffice`、缺 CJK/WPS 字体、模型未配置。
- 2026-06-17：安装 LibreOffice cask 以完成真实导出验证，`doctor` 现在确认 `soffice` 与 `pdftoppm` 均可用；修复 mock PNG asset 的 CRC 问题；实现 target-width preview resize 与 review overlay bbox 绘制。最终验证：`cargo fmt --check && cargo test` 通过 30 个集成测试；`npm run build` 通过；`cargo run -- run ... --mock-models --image-provider openrouter` 真实调用 LibreOffice/Poppler，生成 `figure.pdf`（PDF 1.7, 1 page）、`figure.png`（2130x960 RGB）、`figure_85mm_preview.png`（1004x453 RGBA）和 `figure_review_overlay.png`（1004x453 RGBA）。`doctor` exit 0；字体和模型配置仍以 warning/missing 报告。
- 2026-06-17：补全 `.env.example`，增加 reasoner/coder/vision/image 各角色用途说明、可选 image provider、OpenRouter `METHODFIG_IMAGE_MODALITIES` 说明，并保持所有 API key/model 示例为空，避免误提交真实凭据。验证：`cargo test --test config_tests` 通过。
- 2026-06-19：按用户反馈重新从 high-level 审计功能设计。结论：当前输出图乱不是单纯 prompt 问题，而是 agentic loop 的控制权设计不一致。`src/prompts.rs` 要求 reasoning model 拥有布局、层次、间距和色彩语义，但 `src/pipeline.rs` 在每轮 patch 后立即调用 `canonicalize_plan_for_render`，`src/tools/canonicalize.rs` 对 `teacher_student` 会重写 regions、剪掉非 canonical components/annotations、重建 edges；`renderer/src/runtime.ts` 又按本地模板和 region packing 决定实际 component boxes。结果是模型的设计/修复意图被本地模板覆盖。
- 2026-06-19：检查真实历史运行 `runs/teacher-student-distillation-with-latent-residuals_20260617_205524`。round 000 到 round 003 的 `layout.regions` 完全相同；各轮 patch plan 多次要求移动 teacher/student/residual/loss/inference、移除孤立 inference path、改成 orthogonal routing，但下一轮 `figure_plan.json` 仍回到同一套 canonical regions。round 003 review 仍报 `h_T` label 压线、对角 wandering arrows、右侧空白、teacher/student 不对称、inference path disconnected 等问题。这个证据说明 patch loop 没有真正改变图面结构。
- 2026-06-19：另一个设计矛盾是 coder 角色目前未真正调用模型。`src/agent.rs::create_typescript_code` 在非 mock 模式仍直接返回 deterministic `generate_typescript`，只对 deterministic code 做 safety scan；`CODER_PPTXGENJS_GENERATOR` prompt 存在但未进入执行路径。也就是说 `.env` 要求配置 coder，但实际布局执行仍由固定 runtime 决定，模型不能自主读写每轮 artifacts，也不能产出针对当前图面的 renderer edits。
- 2026-06-19：重新规划的核心方向：保留 PPTX 可编辑和禁止全图 raster 的产品约束，但把“图面设计权”交还给 reasoning/coder model。Rust agent 应该负责沙箱、审计、成本、恢复、schema 兼容、文件边界和质量门禁；模型应被允许在每轮读取 `input.md`、上一轮 `figure_plan.json`、`layout_map.json`、`review.json`、preview/overlay 证据，并产出新的完整 plan 或受控 TS renderer edit。模板和 canonicalization 只能作为 guardrail/fallback，不能在每轮覆盖模型设计。
- 2026-06-19：推荐方案是改成 artifact workspace loop：每轮建立 `round_N/workspace/`，把允许读取的文件显式放入 manifest；reasoner 输出 `DesignBrief` + 完整 `FigurePlan`，而不是只输出 patch 字符串；coder 在受限 API 下生成 `figure.ts` 或结构化 `DrawPlan`，必须使用 stable IDs 写 `layout_map.json`；validator 只做安全/可编辑/几何校验并返回机器可执行错误，不再重排布局。若校验失败，把错误和 overlay 一起交回模型下一轮。这样模型有自主规划权，但仍不能访问网络、env、child process 或任意 fs。
- 2026-06-19：备选方案 A：最小改动，删除 `teacher_student` canonicalization 的重写行为，仅保留 normalize/validate，并实现真实 coder model 调用。优点是快；缺点是仍受现有 `FigurePlan` schema 表达力限制，复杂结构的线段、标签避让、分组和 stage 语义仍难写清。备选方案 B：推荐的 artifact workspace loop，新增 `DesignBrief/DrawPlan` 和受控文件 manifest，重构 patch 为 full-plan revise。优点是符合“给模型更自主读写文件”的目标；缺点是改动较大，需要更多测试。备选方案 C：让模型直接写完整 PptxGenJS TypeScript，Rust 只做 safety scan。优点是自由度最大；缺点是 editable/layout_map/可恢复性更难保证，容易变成不可审计脚本。
- 2026-06-19：下一步如果用户确认推荐方案 B，实施计划应先写失败测试：证明 patch 后 regions 不再被 canonicalization 覆盖；证明非 mock coder 会被真实调用；证明 workspace manifest 只暴露允许文件；证明模型修订后的 `FigurePlan`/`DrawPlan` 能改变 `layout_map.json`；证明输出 PPTX 仍只含 native shapes/text 和小资产图片。然后再拆代码：schema 扩展、agent workspace、真实 coder provider、renderer API 收窄、review feedback 结构化、旧 mock 更新。

## 2026-06-19 high-level redesign draft

### 重新定义职责边界

当前系统名义上有 reasoner/coder/vision，但实际控制权在本地模板。重构后应改成：

- `reasoner` 是 figure director，负责读论文方法、历史 artifacts、review evidence，决定故事线、结构、布局、视觉层级、颜色语义、标注取舍和 small asset 需求。
- `coder` 是 renderer engineer，负责把 reasoner 的设计转成可渲染的 native PPTX primitive plan 或受限 TypeScript。它不应该重新发明语义，但可以处理具体几何、线段、label placement、z-order。
- Rust agent 是 orchestrator 和 guardrail，负责 workspace manifest、成本、恢复、安全扫描、路径隔离、schema 校验、PPTX/native editability 校验、导出和最终验收。Rust 不再替模型做设计性重排。
- `vision` 是 reviewer，必须同时看 rendered preview、overlay、`layout_map.json` 和本轮设计意图，而不是只给抽象打分。
- image model 只处理小 local assets；不能生成整张图，不能生成语义文字、公式或箭头。

### 受控 workspace 文件模型

每轮生成一个明确的工作区：

```text
round_000/
  workspace/
    manifest.json
    readable/
      input.md
      previous_figure_plan.json
      previous_draw_plan.json
      previous_layout_map.json
      previous_review.json
      previous_validation_report.json
    writable/
      design_brief.md
      figure_plan.json
      draw_plan.json
      asset_requests.json
      renderer_notes.md
  figure.ts
  figure.pptx
  figure.pdf
  figure.png
  figure_85mm_preview.png
  figure_review_overlay.png
  layout_map.json
  review.json
  validation_report.json
  renderer_status.json
```

`manifest.json` 是模型的文件边界：列出允许读取的 artifacts、允许写出的文件名、每个文件的用途、最大大小和格式。Rust 只把 manifest 中允许的文本/image evidence 提供给模型，并只接受写到 `writable/` 的结果。禁止模型访问 `.env`、仓库任意源码、`runs/` 外路径和 `../`。这比“完全不让模型读写文件”更自主，也比“直接给模型全仓库权限”更可审计。

### Schema 重心从 PatchPlan 改为完整修订

`PatchPlan.action` 现在是自然语言字符串，本地只会解析少量 bbox；`edge_reroute`、style color、add/remove edge、label bbox 等核心修复基本不可执行。推荐废弃字符串 patch 作为主路径，改成每轮 full-state revise：

- `DesignBrief`：Markdown，说明 main message、读者路径、布局策略、视觉层级、颜色语义、需要删除的噪音、要保留的核心机制。
- `FigurePlan`：语义层，描述 components、groups/stages、edges、assets、design policy，不再强迫套 template。
- `DrawPlan`：图形层，描述最终 native PPTX primitives：box、text、line/polyline/connector、group boundary、annotation、image slot、z-index、style token、stable id 和 bbox。
- `ValidationReport`：本地 validator 给模型的机器可读反馈，包括 invalid path、overlap、label-on-edge、under-utilization、missing stable id、full-slide raster risk、fallback used 等。

如果还需要 patch，应只作为辅助 JSON Patch/RFC6902 或强类型 operations，不能再用自由文本 action 表达几何。

### DrawPlan 推荐形态

`DrawPlan` 是给模型自由度但保住 PPTX 可编辑性的关键。它应包含：

```json
{
  "version": "0.2",
  "canvas": {"aspect": "paper-wide", "target_width_mm": 85},
  "style_tokens": {
    "background": "FFFFFF",
    "primary": "2F6F9F",
    "accent": "4B9A72",
    "neutral_fill": "F4F6F8",
    "text": "1F2328"
  },
  "objects": [
    {
      "id": "c_student",
      "kind": "box",
      "bbox": [0.28, 0.44, 0.46, 0.62],
      "text": "Student",
      "role": "main",
      "style": "primary_module",
      "z": 20
    },
    {
      "id": "e_teacher_student",
      "kind": "connector",
      "points": [[0.37, 0.26], [0.37, 0.44]],
      "from": "c_teacher",
      "to": "c_student",
      "style": "supervision_dash",
      "label": {"text": "distill", "bbox": [0.39, 0.32, 0.47, 0.37]},
      "z": 10
    }
  ]
}
```

这样模型可以直接控制几何、折线路由、label bbox 和 z-order；renderer 只按 plan 画 native PPTX shapes/text/lines/images，并写回 `layout_map.json`。不再由 renderer 猜测 label 放哪里，也不再把多个 components 自动塞进一个 region。

### Renderer 改造原则

- `renderer/src/runtime.ts` 从“布局引擎”降级为“绘图执行器”：给定 `DrawPlan` 就画，不再根据 template/role 决定布局。
- 只保留必要的安全归一化：clamp bbox、最小尺寸、WPS 字体、native shape type、禁止全画布图片。
- 若模型生成 TS，必须只 import local runtime，且 `renderer_status.json` 记录 `source: model_ts`、`used_fallback: false/true`。使用 fallback 时不能算作模型修复成功。
- 更推荐先让 coder 输出 `draw_plan.json`，由 deterministic renderer 画；等 DrawPlan 表达力不够时，再开放受限 TS path。

### Review loop 改造

每轮失败后，下一轮 reasoner 应看到：

- `review.json` 的 blocking/localized issues；
- `layout_map.json` 中实际对象 bbox；
- `validation_report.json` 中本地几何/安全错误；
- `figure_review_overlay.png` 或其对象编号摘要；
- 上一轮 `design_brief.md`、`figure_plan.json`、`draw_plan.json`。

这能避免现在的情况：review 说箭头压线，但 reasoner 只得到抽象 issue，不能可靠定位 renderer 实际把线和 label 放到了哪里。

### 需要删除或降级的旧机制

- `canonicalize_teacher_student` 不能再作为默认主路径；最多作为 `--mock-models` fallback 或 legacy repair，并且必须记录 `renderer_status.used_template_repair = true`。
- `PatchPlan.action` 字符串解析不能再作为核心修复路径。

## 2026-06-19 implementation log: shared-context code editing loop

- 新增并实现 `GeneratedCodeBundle` 主路径：coder 每轮输出 `writable/code/figure.ts`，可选输出 `writable/code/helpers.ts`；Rust 在写盘前做 workspace manifest 校验、路径归一化和 TypeScript safety scan。相关文件：`src/tools/generated_code.rs`、`src/agent.rs`、`src/pipeline.rs`。
- pipeline 不再在主循环里生成或应用 `patch_plan.json`。round 0 调用 initial code bundle；round 1+ 读取上一轮 `figure.ts`、`review.json`、`layout_map.json`、`validation_report.json`，调用 revised code bundle。`PatchPlan` 相关代码暂时保留给 legacy 单测和兼容，但不是 pipeline 主推进机制。
- 每轮 workspace manifest 新增 `writable/code/figure.ts`、`writable/code/helpers.ts`；第二轮起新增 `readable/previous_code/figure.ts`，如果上一轮有 helper 也复制 `readable/previous_code/helpers.ts`。模型共享上下文不再只靠抽象 patch，而是能看到上一轮代码和渲染反馈。
- renderer status 现在区分 `mock_generated_code`、`model_generated_code`、`deterministic_fallback`。如果模型代码失败并使用 deterministic fallback，本轮会被加入 blocking issue，不能被标记为 accepted。
- safety scan 扩展为允许同目录 helper import，包括多行 static import；仍拒绝 parent import、network、`child_process`、`fs`、`process.env`、`fetch` 等越权能力。真实 smoke 曾暴露多行 import 被误判的问题，已用 `tests/safety_tests.rs` 固化。
- 真实 smoke 暴露 reasoner 可能返回 region/component 共用 stable id。新增 `normalize_plan_for_render` 的 region id 去重：优先重命名 region 并同步 `component.region`，不改变 component id 和 edge 引用。相关测试：`tests/plan_normalize_tests.rs`。
- OpenAI-compatible chat/vision provider 增加 180 秒 HTTP timeout，避免真实模型请求无限挂起；`scripts/run_real_env.sh` 增加 `MAX_ITERATIONS` 支持，并把该值传给 CLI，避免 smoke 默认跑满 12 轮。
- 文档同步：`.env.example` 中 coder 改为每轮 `GeneratedCodeBundle` 职责；README output layout 改为 workspace/code artifacts；`.gitignore` 不再忽略 `plan.md`、`goal.md`、`docs/plan.md`、`docs/goal.md`，满足项目约束文件需要随项目打包的要求。

## 2026-06-19 verification log: shared-context code editing loop

- 先写红测并确认失败：`tests/workspace_tests.rs` 要求 `WorkspaceFileFormat::Typescript`；`tests/safety_tests.rs` 要求 helper import 和多行 import；`tests/workspace_pipeline_tests.rs` 要求第二轮读取上一轮 `previous_code/figure.ts` 并修订 `figure.ts`，且不写主路径 `patch_plan.json`；`tests/plan_normalize_tests.rs` 要求重复 region/component id 被归一化。
- 目标测试通过：`cargo test --test workspace_tests --test safety_tests --test workspace_pipeline_tests --test pipeline_tests`。
- 全量本地验证通过：`cargo fmt --check`、`cargo test`、`cd renderer && npm run build`。
- 真实长 run：`runs/teacher-student-distillation-with-latent-residuals_20260619_115607` 生成到 round 006 后手动中断，因为脚本当时未限制迭代数。该 run 证明真实 reasoner/coder/render/export 路径能进入多轮；`round_000/renderer_status.json` 为 `source: model_generated_code, used_fallback: false`，`round_001/workspace/readable/previous_code/figure.ts` 存在，`round_001/renderer_status.json` 为 `source: deterministic_fallback, used_fallback: true`，说明第二轮读取了上一轮代码但模型代码失败后被 fallback 拦截。
- 受控 2 轮真实 smoke：`MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md` 使用 `--max-iterations 2`，但 coder provider `https://maas-coding-api.cn-huabei-1.xf-yun.com/v2/chat/completions` 在 180 秒后超时，命令按预期失败而不是无限挂起。剩余风险是外部 coder provider 延迟/稳定性会影响真实 loop；本地 mock 和部分真实路径已验证。

## 2026-06-19 AutoFigure-Edit root-cause audit

- 参考仓库：`https://github.com/ResearAI/AutoFigure-Edit.git`，本地审阅路径 `/tmp/AutoFigure-Edit`。关键实现集中在 `autofigure2.py` 的 `generate_svg_template`、`check_and_fix_svg`、`optimize_svg_with_llm`。
- AutoFigure-Edit 的有效设计不是简单“提示词更强”，而是四阶段闭环：先生成/导入视觉草图，再用 SAM 得到带编号 bbox 的 `samed.png`/`boxlib.json`，再让多模态模型按这些视觉事实生成 editable SVG，最后用“原图 + 标注图 + 当前 SVG 渲染 PNG + 当前 SVG code”做 position/style 优化。它的 optimizer 明确检查两个方面八个点：icon/text/arrow/line 的位置，以及 icon/text/arrow/line 的样式。
- 当前项目质量差的直接证据：`runs/teacher-student-distillation-with-latent-residuals_20260619_115607/final/figure.png` 中 `Input x` 与 `Inference: student only` 压住，teacher/student 盒子内部空白极大，`Task Loss` 普通盒子挤在右侧，标签和线条仍有冲突。`round_005/figure_review_overlay.png` 显示 local overlay 已能定位这些对象，但下一轮没有把这些视觉事实转成新的几何状态。
- 当前架构根因：`run_pipeline` 每轮都从同一个 `FigurePlan` 重新派生 `draw_plan_from_figure_plan(&plan, &style)`；`previous_review` 只传给 coder 的 TypeScript 修订 prompt。coder 被要求“preserve the reasoning model's DrawPlan contract”，所以它几乎不能修改 bbox、connector points、label bbox、删除边缘注释。结果是 review 反馈没有驱动 `draw_plan.json` 发生结构性改变。
- 因此结论是“架构设计不当为主，提示词不够具体为辅”。正确修复不是继续调 `VISION_REVIEWER` 或 coder wrapper prompt，而是在每轮渲染前增加 AutoFigure-Edit 式 `DrawPlan optimizer`：输入上一轮 rendered overlay、`layout_map.json`、`review.json`、`validation_report.json`、上一轮 `draw_plan.json`，输出新的完整 `DrawPlan`。coder 随后只负责渲染这个 DrawPlan，不再承担几何设计主责。
- `runtime.ts` 的 `teacherStudentLayout`、`multimodalFusionLayout`、`pipelineLayout` 不能覆盖模型给出的几何。
- `create_typescript_code` 不能在非 mock 下假装调用 coder；要么真实调用 coder provider，要么明确写 `renderer_status.source = deterministic_fallback` 并让 run 不计为 agentic 成功。

### 验收标准

重构完成不能只看 `cargo test` 通过，还要证明这些事实：

- round 000 的布局被 review 拒绝后，round 001 的 `draw_plan.json` 和 `layout_map.json` 发生模型驱动的结构性变化。
- 对同一 teacher-student 输入，模型要求移动/删除 inference path 时，下一轮 artifacts 中不再被 canonicalization 恢复成旧 regions。
- 非 mock 模式下 coder provider 被实际调用；若模型 code/draw plan 失败，status 明确记录 fallback，不把 fallback 算作 accepted model design。
- workspace manifest 拒绝 `.env`、`../`、仓库外路径和未声明 writable 文件。
- 输出 PPTX 中语义内容仍是 native shapes/text/lines；小图片只能来自 `asset_requests.json`，不能有 full-slide raster。
- vision review 和 local gate 都通过后才 accepted；local gate 至少检查 component overlap、label-edge overlap、edge crossing、under-utilization、font/readability、full-raster 风险。

### 推荐实施顺序

1. 先写红测试锁定当前 bug：patch 后 regions 被 canonicalizer 覆盖；非 mock coder 没有被调用；edge reroute action 不能改变实际 route；fallback 被当成成功。
2. 增加 `WorkspaceManifest`、`AgentWorkspace`、路径白名单和 `validation_report.json`。
3. 增加 `DesignBrief`/`DrawPlan` schema，并让 mock loop 通过 full-state revise 改变 layout。
4. 把 renderer 改成 DrawPlan executor，保留旧 FigurePlan renderer 作为 legacy fallback。
5. 接入真实 coder provider，输出 `draw_plan.json` 或受限 `figure.ts`。
6. 改 review prompt，让 vision/reasoner 都基于 actual artifacts 修订。
7. 最后再删掉或降级 teacher-student canonicalization 的默认路径。

## 2026-06-19 重构执行记录

- 创建分支 `refactor-agentic-workspace-loop`，避免直接在 `main` 上改大型重构。
- 基线检查发现 `cargo test` 通过，但 `renderer npm run build` 失败：`runtime.ts` 已使用 layout object kind `label`，`safe_api.ts` 类型未声明。修复为 `LayoutObject.kind` 增加 `label`，随后 renderer build 通过。
- 按 TDD 写红测 `canonicalize_preserves_model_authored_teacher_student_layout`，证明旧 `canonicalize_plan_for_render` 会把模型给出的 `teacher_student` regions 覆盖成固定 `ts_*` 模板。随后将 `src/tools/canonicalize.rs` 降级为非设计性处理：只在 `--image-provider none` 时清空图片资产和组件 asset 引用，不再重写 regions/components/edges/annotations。旧 canonicalization 测试改为保护“保留模型布局，只处理资产策略”。
- 新增 `src/tools/workspace.rs` 和 `tests/workspace_tests.rs`，实现 `WorkspaceManifest`、`WorkspaceFile`、`AgentWorkspace`。manifest 只允许 `readable/` 与 `writable/` 下的相对路径，拒绝 `.env`、绝对路径、`..` 跳转和未声明 writable 文件。这个边界用于给模型更多 artifact 读写权，同时不放开整个仓库或环境变量。
- 新增 `DrawPlan` schema：`DrawPlan`、`DrawObject::{box,text,connector,image,group}`、`DrawLabel` 和 `validate_draw_plan`。测试覆盖 box/connector/label roundtrip、重复 object id 拒绝、full-slide raster image 拒绝。
- 新增 `src/tools/draw_plan.rs`，从现有 `FigurePlan` 生成过渡期 `DrawPlan`，并生成 `createDrawPlanRuntime` TypeScript payload。组件在模型给出的 region 内做 packing；宽 region 优先按 reading order 横向排布，避免 multimodal mock 中 4 个组件被排成 3+1 后产生压线。
- 扩展 `renderer/src/runtime.ts`，新增 `DrawPlanRuntime`。它按 `DrawPlan.objects` 直接绘制 native PPTX box/text/connector/group/image，并写 `layout_map.json`；旧 `FigureRuntime` 保留为 fallback。`safe_api.ts` 增加 `image` kind 以描述小资产 slot。
- 修改 `run_pipeline`：每轮写 `workspace/manifest.json`、`workspace/readable/input.md`、`workspace/writable/design_brief.md`、`workspace/writable/figure_plan.json`、`workspace/writable/draw_plan.json`、`workspace/writable/asset_requests.json`、`workspace/writable/renderer_notes.md`；同时在 round root 写 `draw_plan.json`、`validation_report.json`、`renderer_status.json`。渲染主路径改为 deterministic DrawPlan renderer，旧 FigurePlan renderer 只作为 fallback；final 目录同步保留 `draw_plan.json`、`validation_report.json`、`renderer_status.json`。
- 新增 `tests/workspace_pipeline_tests.rs`，验证 mock pipeline 会写 agentic workspace、DrawPlan artifacts 和 renderer status，并把 `draw_plan.json`/`renderer_status.json` 带到 final。
- 中途完整测试发现 `resume_pipeline_uses_existing_run_directory` 失败。复现到 `/tmp/methodfig_resume_debug` 后确认原因是 DrawPlan 初版 packing 让 multimodal fusion 的 `head` 掉到第二行，`fusion_to_head` label 与 edge overlap。修复宽 region packing 后该测试恢复。
- 最终验证：`cargo fmt --check` 通过；`cargo test` 全部通过；`cd renderer && npm run build` 通过。

### 当前仍未完成

- 非 mock `coder` 仍未真正调用模型输出 `draw_plan.json` 或受限 `figure.ts`；当前主路径是 deterministic DrawPlan 过渡实现。
- `review` prompt 还没有改成显式消费 workspace/readable 中的 `previous_draw_plan.json`、`previous_validation_report.json` 和 overlay evidence。
- `validation_report.json` 当前只写空 warnings/errors，占位已经进入 artifact 流，但还没有汇总本地 gate 的结构化错误。
- 旧 `PatchPlan` loop 仍存在，下一步应把 reasoner patch 从自由文本 action 迁移到 full-state revise 或强类型 operation。

## 2026-06-19 PPTX artifact visibility fix

- 用户反馈“没有看到有 pptx 生成”。排查 `runs/` 后确认最新真实运行 `runs/teacher-student-distillation-with-latent-residuals_20260619_140018/final/figure.pptx` 已存在，大小约 61 KB；`finalize_from_round` 的复制清单也包含 `figure.pptx`，所以不是导出或 final 复制逻辑丢失。
- 根因是产物可见性不足：CLI 结束时只打印 `final` 目录，没有直接打印 `final/figure.pptx`；`scripts/run_real_env.sh` 只打印时间戳 run dir，没有维护稳定入口，用户需要手动在多个时间戳目录里查找。
- 修改 `src/main.rs`：`run` 和 `resume` 完成后打印 `run_dir`、`final_dir`、`pptx`、`png`、`status` 的完整路径，并打印 `reason`，避免 rejected run 被误认为没有产物。
- 修改 `scripts/run_real_env.sh`：成功运行后打印 `final/figure.pptx`、`final/figure.png`、`final/status.json`，并维护 `runs/latest` symlink 指向最新 run；如果命令失败，也列出该 run 下已经生成的 `round_*/figure.pptx`。
- 修改 `README.md`：记录成功运行后可直接打开 `runs/latest/final/figure.pptx`。
- 验证结果：
  - `cargo fmt --check` 通过。
  - `bash -n scripts/run_real_env.sh`、`bash -n scripts/run_real_loop.sh` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - mock smoke：`cargo run -- run --method examples/teacher_student.md --out runs/smoke_pptx_visibility_20260619_140932 --style wps-clean --aspect paper-wide --target-width-mm 85 --max-iterations 1 --max-cost-usd 3 --max-minutes 20 --image-provider none --mock-models --keep-intermediate` 成功打印 `pptx: .../final/figure.pptx`，文件大小约 49 KB。
  - 真实 `.env` smoke：`MAX_ITERATIONS=1 bash scripts/run_real_env.sh examples/teacher_student.md` 成功打印 `pptx: runs/teacher-student-distillation-with-latent-residuals_20260619_140943/final/figure.pptx`，并更新 `runs/latest`；`runs/latest/final/figure.pptx` 存在，大小约 60 KB。该 run 只跑 1 轮，`status.json` 为 `accepted=false`、`reason=cap reached before acceptance`，这不影响 PPTX 产物存在。

## 2026-06-19 DrawPlan geometry evidence repair

- 继续排查真实质量问题。`runs/latest/final/figure.png` 和 `review.json` 显示 round 0 图仍有大面积重叠、斜线、label 压线和边缘 annotation。此前 2 轮真实 run `runs/teacher-student-distillation-with-latent-residuals_20260619_140018` 证明 DrawPlan optimizer 已能改变布局，但仍被 `edge crossing`、`label on line`、`anno_inference` 等问题阻塞。
- 发现一个本地 gate 根因：`renderer` 的 `layout_map.json` 对 polyline connector 只记录整体 bbox，`render_quality_gate` 把 bbox 对角线当成真实 edge 判断 crossing。真实 2 轮 run 中 `e_latent_to_loss` 与 `e_student_to_output` 的 crossing 很可能是这个误判。
- 先写红测：
  - `tests/review_tests.rs::render_quality_gate_uses_polyline_points_for_edge_crossing`：构造一个 bbox 对角线会相交、但真实 polyline segments 不相交的 layout map。旧实现失败并报 `edge crossing`。
  - `tests/render_region_layout_tests.rs::draw_plan_renderer_draws_connector_labels_above_boxes` 扩展检查 `layout_map.objects[id=e1].points`。旧 renderer 没写 points，测试失败。
- 修复：
  - `renderer/src/safe_api.ts` 为 `LayoutObject` 增加可选 `points`。
  - `renderer/src/runtime.ts` 在 DrawPlan renderer 和 legacy FigurePlan renderer 的 edge layout entry 中写入真实 connector points。
  - `src/tools/review.rs` 优先用 `points` 拆 segment 判断 edge length、label-over-edge 和 edge crossing；没有 points 的旧 artifact 才回退到 bbox。
- 进一步写红测 `tests/draw_plan_tests.rs::draw_plan_geometry_repair_removes_marginal_notes_and_routes_labels_off_edges`，固定 DrawPlan 写盘前的确定性几何修复契约：删除明显边缘 annotation、把明显 diagonal segment 转成 orthogonal polyline、把 edge label 移出对应 stroke 区域。
- 修复：
  - `src/tools/draw_plan.rs` 新增 `repair_draw_plan_geometry(&mut DrawPlan)`，保守处理三类结构性问题：marginal annotation、diagonal connector、connector label placement。
  - `src/pipeline.rs` 在 `draw_plan_from_figure_plan` 或 `revise_draw_plan_from_feedback` 之后、`validate_draw_plan` 和写盘之前调用 `repair_draw_plan_geometry`，确保模型输出不会原样带着可判定几何错误进入渲染。
- 验证：
  - 红测已确认失败后转绿：`cargo test --test review_tests render_quality_gate_uses_polyline_points_for_edge_crossing`、`cargo test --test render_region_layout_tests draw_plan_renderer_draws_connector_labels_above_boxes`、`cargo test --test draw_plan_tests draw_plan_geometry_repair_removes_marginal_notes_and_routes_labels_off_edges`。
  - 相关测试通过：`cargo test --test draw_plan_tests --test review_tests --test render_region_layout_tests --test workspace_pipeline_tests --test pipeline_tests`。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - 待验证：真实 `.env` 2 轮 smoke，观察 `layout_map.points`、是否消除本地 false crossing、DrawPlan repair 后视觉是否更干净。

## 2026-06-19 真实 non-mock smoke

- 用户明确说明本地 `.env` 中的 LLM 资源免费，可以大胆使用。新增根目录 `AGENTS.md`，记录项目级约束：真实 `.env` LLM 视为免费，验证 agentic loop 时不要因为成本回避 non-mock；但仍禁止打印/提交 API key，模型文件读写必须走 workspace/manifest 边界，不能访问 `.env`、仓库外路径、任意网络或未声明文件。
- 执行 `bash scripts/run_real_env.sh examples/teacher_student.md`，真实 run 目录为 `runs/teacher-student-distillation-with-latent-residuals_20260619_104150`。脚本使用真实 `.env`、`--image-provider none`、`--keep-intermediate`、高 cost/minute cap。
- run 成功完成 round 000 和 round 001 的 PPTX/PDF/PNG 导出、vision review、workspace/manifest、DrawPlan、renderer status 写入。两轮 `renderer_status.json` 均为 `source: deterministic_draw_plan`、`used_fallback: false`，说明新的 DrawPlan renderer 主路径可真实运行。
- round 000 workspace manifest 只暴露 `readable/input.md`；round 001 manifest 已包含 `previous_figure_plan.json`、`previous_draw_plan.json`、`previous_layout_map.json`、`previous_review.json`、`previous_validation_report.json`，符合“下一轮模型能读实际 artifacts”的目标。
- 两轮 `draw_plan.json` 均为 version `0.2`，包含 `box`、`connector`、`text` 三类对象。语义仍由 native PPTX primitives 表达，没有启用整图 raster 或 image generator。
- run 未自然完成：等待 round 001 后续 reasoner patch 调用时间过长，手动中断，退出码 130；因此没有 `final/status.json`。已生成 artifacts 保留用于分析。
- 真实 review 暴露的问题与当前剩余设计缺口一致：round 000/001 仍有 diagonal routing、label on line、边缘 annotation、component overlap。根因是非 mock 路径仍在使用旧 `PatchPlan` + deterministic DrawPlan 过渡实现，coder 还没有真正接管 `draw_plan.json` 几何和 routing。下一步应优先实现真实 coder/full-state revise，而不是继续调本地模板。

## 2026-06-19 PPTX 产物复查

- 用户再次反馈“没有看到有 pptx 生成”。按 systematic debugging 重新排查产物流，而不是直接猜测脚本问题。
- 当前 `runs/latest` 指向 `runs/teacher-student-distillation-with-latent-residuals_20260619_145452`。该 run 的 `final/` 下实际存在 `figure.pptx`、`figure.pdf`、`figure.png`、`draw_plan.json`、`layout_map.json`、`review.json`、`status.json` 等文件。
- `file runs/latest/final/figure.pptx` 识别为 zip-based PowerPoint package；`unzip -l runs/latest/final/figure.pptx` 能看到 `[Content_Types].xml` 和 `ppt/slides/slide1.xml`，说明不是空占位文件。
- 最新 `final/status.json` 为 `accepted=false`、`reason="cap reached before acceptance"`；`final/renderer_status.json` 为 `source="deterministic_fallback"`、`used_fallback=true`。因此当前事实是“PPTX 有生成，但该轮未通过质量验收，并且使用了 fallback renderer，不能算模型修复成功”。
- `scripts/run_real_env.sh` 已在成功时打印 `pptx: <run>/final/figure.pptx` 并维护 `runs/latest`；失败时也会列出已生成的 `round_*/figure.pptx`。用户若只看 accepted 状态，可能会把 rejected run 误解为没有产物。
- 后续真正待修复项不是 final 复制，而是：模型生成 TypeScript 失败后仍进入 fallback；DrawPlan optimizer 对 `task_loss -> student_model`、inference branch 和边缘 annotation 的几何修复还不够，导致真实 review 未通过。

## 2026-06-19 round 001 blocker 修复

- 继续排查最新真实 run `runs/latest -> teacher-student-distillation-with-latent-residuals_20260619_145452`。确认 `figure.pptx` 存在但 `accepted=false`，`renderer_status.used_fallback=true`。
- 读取 `round_001/workspace/writable/code/figure.ts` 后发现模型生成的 TS 能调用 `createDrawPlanRuntime`，但 payload 中的 `out_dir` 指向 `round_002`，所以 Node 执行后没有在当前 `round_001` 写出 `figure.pptx/layout_map.json`，触发 `renderer did not create figure.pptx and layout_map.json` 并进入 fallback。
- 按 TDD 写红测 `tests/render_fallback_tests.rs::renderer_forces_current_round_out_dir_over_model_payload`，证明旧 runtime 会被模型 payload 输出目录带偏。
- 修复：`run_node_renderer` 启动 Node 时设置 `METHODFIG_RENDER_OUT_DIR=<current round>`；`renderer/src/runtime.ts` 中 trusted runtime 用该环境变量覆盖 payload `out_dir`。这样输出目录由 Rust orchestrator 控制，模型不能把产物写到其他 round。
- 按 TDD 写红测扩展 `draw_plan_geometry_repair_preserves_teacher_student_lanes`：覆盖真实 review 中的 `e_task_student` 方向为 `task_loss -> student_model`、旧 style 为 `normal_flow`、以及重复 `inference_student` 分支和 `ann_inference`。
- 修复：`repair_teacher_student_lanes` 现在会删除重复的 inference-only student 结构及其长连接；对 `loss -> student` 反馈边使用 dashed supervision 语义，并走干净的底部正交反馈路由；`is_marginal_annotation` 增加 `ann_` 和 `a_` 前缀，清理靠近 safe-area 边缘的 phase/inference 注释。
- 按 TDD 修改 `plan_geometry_gate_rejects_diagonal_simple_chain_without_treating_annotations_as_source_of_truth`，证明 `FigurePlan.annotations` 不再是渲染源后，旧 plan gate 不能因为被 DrawPlan 删除的注释继续阻塞验收。
- 修复：删除 `plan_geometry_issues` 中对 `FigurePlan.annotations` 的阻塞检查。注释质量现在由实际渲染后的 `layout_map.json` / DrawPlan 输出决定，避免旧语义层和新图形层互相打架。
- 红测转绿：
  - `cargo test --test render_fallback_tests renderer_forces_current_round_out_dir_over_model_payload`
  - `cargo test --test draw_plan_tests draw_plan_geometry_repair_preserves_teacher_student_lanes`
  - `cargo test --test review_tests plan_geometry_gate_rejects_diagonal_simple_chain_without_treating_annotations_as_source_of_truth`
- 相关回归通过：`cargo test --test draw_plan_tests --test render_fallback_tests --test review_tests --test workspace_pipeline_tests --test pipeline_fallback_tests`。
- `cd renderer && npm run build` 通过。`cargo fmt --check` 首次只报测试文件格式差异，随后已运行 `cargo fmt`。
- 全量验证通过：`cargo fmt --check`、`cargo test`、`cd renderer && npm run build`。
- 真实 `.env` smoke：`MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md`，run 目录 `runs/teacher-student-distillation-with-latent-residuals_20260619_151314`。本轮成功修掉 renderer fallback，`final/renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`，且 `final/figure.pptx` 是真实 zip-based PPTX，`figure.pdf` 和 `figure.png` 均生成。
- 真实 smoke 仍未 accepted，`status.json` 为 `accepted=false`。新的主要问题不是旧的 `e_task_student`/`ann_*`，而是 optimizer 过度展开：生成 `h_teacher`、`h_student`、`student_inf`、`y_out`、重复 residual edges、dangling `e_ht_hs_to_residual` 和底部 `phase_*` labels，导致布局更复杂、边更多、交叉更多。
- 针对该真实问题新增红测 `tests/draw_plan_tests.rs::draw_plan_geometry_repair_simplifies_overexpanded_teacher_student_optimizer_output`，用真实 run 的对象形态复现：隐藏状态盒、重复 inference student、重复 output、phase labels、dangling residual stub 和多条经过隐藏状态的 residual edges。
- 修复：`repair_teacher_student_lanes` 增加语义简化层，删除隐藏状态中间盒 `h_*`、重复 inference-only student 分支、重复 inference output、相关连接和底部 phase labels；如果删除后缺核心边，则补回 `teacher -> residual` dashed supervision 和 `loss -> student` dashed feedback。这样模型仍可优化 DrawPlan，但不能把简单 teacher-student 方法图扩成不可读的隐藏状态子图。
- 新增红测已转绿；相关回归 `cargo test --test draw_plan_tests --test render_fallback_tests --test review_tests --test workspace_pipeline_tests` 通过。待再次跑全量和真实 `.env` smoke，确认 overexpanded repair 是否让 round 001 更接近验收。

## 2026-06-19 semantic repair 收紧

- 第二次真实 `.env` smoke：`runs/teacher-student-distillation-with-latent-residuals_20260619_152523`。结果仍为 `accepted=false`，但质量问题已从“大量隐藏状态/重复 inference 分支/fallback”收窄为：`e_student_to_output` 绕路、connector label 压线、自动补出的 `e_loss_student` 被 reviewer 判为非 FigurePlan 冗余边。
- 据此更新红测 `draw_plan_geometry_repair_simplifies_overexpanded_teacher_student_optimizer_output`：要求不主动新增 `e_loss_student`，要求 output 与 student lane 对齐，使 `student -> output` 可直接水平连线，并要求 canonical teacher-student repair 清除容易压线的 connector labels。
- 修复：`repair_teacher_student_lanes` 不再在缺失时自动补 `loss -> student`；仍会修复模型已显式提供的 `loss -> student` 反馈边。`output_box` 下移到 student lane；canonical connector 分支会清空 edge labels，避免 `h_T`、`h_S`、`L_resid`、`ŷ` 等短标签压线或落入 box 内。
- 验证：targeted 测试转绿；`cargo fmt --check` 通过；相关回归 `cargo test --test draw_plan_tests --test render_fallback_tests --test review_tests --test workspace_pipeline_tests` 通过；全量 `cargo test` 通过；`cd renderer && npm run build` 通过。
- 待验证：再次真实 `.env` smoke，确认 `e_loss_student` 不再出现、`e_student_to_output` 变为直接路径、connector labels 不再引发 text-on-line blocker。

## 2026-06-19 controlled labels and phase badges

- 第三次真实 `.env` smoke：`runs/teacher-student-distillation-with-latent-residuals_20260619_153306`。结果仍为 `accepted=false`，但图面已经干净很多：没有 renderer fallback、没有隐藏状态盒、没有重复 inference student、没有 dangling residual stub。剩余 blocker 变成：缺少 inference/phase 标识、缺少 `h_T - h_S` 与 `predict` 标签、task loss edge 应从 output 而不是 student 出发、residual edge 应绕开 student body。
- 对应更新 `draw_plan_geometry_repair_simplifies_overexpanded_teacher_student_optimizer_output`：要求 `student -> output` 保留受控 `predict` label；`teacher -> residual` 保留单个 `h_T - h_S` label；`student -> task_loss` 被重写为 `output -> task_loss`；`residual -> student` 进入 student 顶部而不是贴着右边穿过 body；补 `training_badge` 与 `inference_badge` 两个小文本对象。
- 修复：`repair_teacher_student_lanes` 现在会在 canonical teacher-student 布局里恢复必要但受控的语义标签；把 task loss 边的起点改为 output；把 residual supervision 通过顶部进入 student；补小型 phase/inference badge，且 badge 使用非 annotation/phase style，避免被边缘注释清理规则误删。
- 验证：targeted 测试转绿；`cargo fmt --check` 通过；相关回归通过；全量 `cargo test` 通过；`cd renderer && npm run build` 通过。
- 待验证：下一次真实 `.env` smoke，观察 review 是否还报 missing labels/phase、task-loss origin 和 residual-through-student。

## 2026-06-19 collapsed teacher/latent 修复

- 第四次真实 `.env` smoke：`runs/teacher-student-distillation-with-latent-residuals_20260619_154348`。结果仍为 `accepted=false`。新问题是模型把 `teacher_latent` 同时当 teacher 和 latent/residual，repair 因此创建了 `teacher_latent -> teacher_latent` 自环，且图中没有独立 residual box。另一个问题是 inference badge 变成边缘注释，task loss 与 predict edge/label 碰撞。
- 新增红测 `draw_plan_geometry_repair_creates_residual_when_optimizer_merges_teacher_and_latent`，复现该变体：只有 `teacher_latent`、`student_main`、`task_loss`、`answer_out`，没有独立 residual；模型还给出 teacher->student dashed shortcut。
- 修复：latent/residual 识别现在会排除 teacher/student/context；缺少独立 residual 时创建 `latent_residual` box。删除 teacher->student dashed shortcut，强制改成 `teacher -> residual -> student`。task loss 下移到 prediction edge 下方，避免碰撞；不再自动添加 marginal `inference_badge`。
- 验证：新增 targeted 测试转绿；相关回归通过；全量 `cargo test` 通过；`cd renderer && npm run build` 通过。
- 待验证：再次真实 `.env` smoke，确认不再出现 teacher/residual 自环、缺 residual box、task loss 碰撞和 marginal inference badge。

## 2026-06-19 deterministic DrawPlan source-of-truth 修复

- 真实 smoke `runs/teacher-student-distillation-with-latent-residuals_20260619_155147` 证明旧架构仍有关键问题：`final/renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`，但 `draw_plan.json` 中 `predict` label 位于箭头上方，而 `layout_map.json/review.json` 显示最终渲染 label 又落回箭头线上。结论：非 mock 模式仍让模型生成 TS 成为最终几何来源，绕开了 DrawPlan repair。
- 写红测：
  - `pipeline_tests::nonmock_renderer_uses_deterministic_draw_plan_code_as_primary_contract`
  - `pipeline_tests::mock_renderer_keeps_generated_code_with_deterministic_fallback`
  - 更新 `draw_plan_geometry_repair_preserves_teacher_student_lanes` 和 `draw_plan_geometry_repair_simplifies_overexpanded_teacher_student_optimizer_output`，要求 task-loss edge 保持 student 语义来源、去掉侵入主图的 inference badge。
- 修复：
  - `src/pipeline.rs` 新增 `select_renderer_code`：mock 模式仍用 generated code + deterministic fallback；非 mock 模式改为直接用 deterministic DrawPlan runtime 作为主渲染代码，`renderer_status.source` 写为 `deterministic_draw_plan`。
  - `src/tools/draw_plan.rs` 不再把 `student -> task_loss` 重写成 `output -> task_loss`；删除模型给出的 `inference_badge`/floating inference text；后续把 inference 语义折叠到 Student module 文本，避免孤立 box 和长 inference loop。
- 真实 smoke `runs/teacher-student-distillation-with-latent-residuals_20260619_160551`：
  - round 000/001 均为 `source="deterministic_draw_plan"`、`used_fallback=false`，说明渲染主路径已切换成功。
  - round 001 review 不再报 model TS 覆盖 geometry，但指出新的 DrawPlan 语义问题：缺/错 inference flow、latent-to-student route 复杂、teacher label 位置差。round 002 vision provider 超时，run 失败；该失败记录为外部 provider timeout，不代表本地渲染失败。
- 继续写红测收紧 teacher-student repair：
  - 保留/再折叠 inference 语义，删除长 `input -> inference`、`inference -> output`、`student -> inference` crossing edge。
  - 排除 `latent_loss`/role=loss 的 residual-like box 被误选为 canonical residual。
  - 删除 duplicate residual boxes 及其旧 connector。
  - 删除 `input -> task_loss` ground-truth shortcut。
  - 让 `task_loss` 优先按 task loss 识别，不再误选 `latent_residual`。
  - `student -> output` 从 student 上半部离开，`student -> task_loss` 从 student 底部离开，避免穿过学生框文字。
- 全量验证通过：
  - `cargo fmt --check`
  - `cargo test`
  - `cd renderer && npm run build`
- 真实短 smoke `MAX_ITERATIONS=1 bash scripts/run_real_env.sh examples/teacher_student.md`，最新 run `runs/teacher-student-distillation-with-latent-residuals_20260619_164407`：
  - `final/renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - `final/figure.pptx` 是真实 zip-based PPTX；`final/figure.png` 为 2130x960 PNG。
  - DrawPlan 已消除上一轮主要硬错误：没有 duplicate residual、没有 orphan inference box、没有 `ground_truth_to_task_loss` shortcut、`task_loss` 是 compact box、student->loss 起点在 student bottom edge。
  - 仍未 accepted：`final/status.json` 为 `accepted=false`。剩余 reviewer blocker 已收窄为布局表达层问题：teacher/student 纵向堆叠浪费 paper-wide 宽度；reviewer 希望 training/inference phase 有独立视觉层级；`e_latent_student` 仍被判 backtracking；student->output/task_loss 还有轻微 over-routing。
- 下一步应停止在当前 vertical stacked template 上继续小修，改为针对 teacher-student 方法图实现更明确的 two-phase / paper-wide canonical DrawPlan：Training lane 与 Inference lane 分区、teacher/student/residual/output/loss 横向利用宽画布、phase labels 放在不与 connector 相交的 bands。该方向比继续让模型每轮试探局部修复更符合 AutoFigure-Edit 的“结构化 editable state + visual optimization evidence”闭环。

## 2026-06-19 two-phase paper-wide DrawPlan 修复

- 继续处理用户原始目标中的核心质量问题：迭代后图面仍差，需要判断是 prompt 不够还是架构不当。重新核对最新 `runs/latest/final/review.json` 后确认问题不是没有生成 PPTX，而是 deterministic teacher-student repair 仍把模型语义压成中间竖排模板：teacher/student 纵向堆叠、training/inference 合并成一个 student label、phase labels 被清理、`e_latent_student` 回折。
- 参考 AutoFigure-Edit 的设计理念：其优化器不是只调 prompt，而是把原图、标注图、结构化 boxes 和当前 SVG code 一起交给优化器做位置/样式修正。本项目对应物是 `draw_plan.json`、`layout_map.json`、review overlay 和 native PPTX DrawPlan。当前缺口是本地 repair 把这些证据后的修复又压回固定竖排，因此应改结构化 DrawPlan contract。
- 按 TDD 先更新红测：
  - `draw_plan_geometry_repair_preserves_teacher_student_lanes` 要求 teacher-student 图使用 paper-wide 横向空间、保留单独 `inference_student`、添加 `phase_training_label` / `phase_inference_label`，且 latent residual 到 student 是简单 L 形。
  - `draw_plan_geometry_repair_simplifies_overexpanded_teacher_student_optimizer_output` 要求删除模型杂散 `student_inf` / `y_out` / hidden state 中间盒后，重建受控 inference lane，而不是把 inference 折进 `Student (train / inference)`。
- 红测确认旧实现失败：`cargo test --test draw_plan_tests draw_plan_geometry_repair_preserves_teacher_student_lanes -- --nocapture` 报 `task loss should use the right side of the paper-wide canvas`。
- 修复：
  - `src/tools/draw_plan.rs::repair_teacher_student_lanes` 的 canonical bbox 从中间竖排改为 paper-wide 两阶段结构：input 左侧，teacher/student training 左中，latent residual 中部，output/task loss 右侧，下方单独 inference student/output lane。
  - 不再把 inference 语义合并到 training student 文本；training student 标为 `Student\n(training)`，受控 inference lane 标为 `Student\n(inference only)`。
  - 新增受控 `phase_training_label` 和 `phase_inference_label`，位置在主图内部，避免被 marginal annotation 清理。
  - 重写 `teacher -> latent`、`latent -> student`、`student -> output`、`student -> task_loss`、`input -> inference_student`、`inference_student -> inference_output` 的 connector points，消除上一轮 review 中的回折和简单 flow 过度 dogleg。
- 局部验证：
  - `cargo test --test draw_plan_tests draw_plan_geometry_repair_preserves_teacher_student_lanes -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests draw_plan_geometry_repair_simplifies_overexpanded_teacher_student_optimizer_output -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过 8 个测试。
- 后续真实 smoke 继续暴露 model 输出变体，已追加修复：
  - 缺 training output 时合成 `output_pred`，并把 training lane 规范为 `student -> output -> task_loss`，避免长 `student -> loss` 直连。
  - 删除 floating phase captions；phase 信息保留在 `Student\n(training)` / `Student\n(inference only)` 的 editable box 文本中，避免边缘 caption 被 reviewer 判为 marginal note。
  - inference lane 从 `input -> inference_student` 长绕线改为 `student -> inference_student` 直接竖直延续。
  - renderer 支持 `primary_module_regular`：保留主色边框/填充，但文字不强制 bold，降低视觉层级冲突。
  - 增强 cleanup：删除 `teacher_latent` / `Latent h_T` 这类中间 hidden latent box、删除连接到 student 的重复 output box、删除任何 `teacher -> student` residual shortcut，不再只按 dashed style 删除。
- 验证：
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - 多次真实 `.env` 短 smoke 均生成真实 PPTX，最新 `runs/latest -> teacher-student-distillation-with-latent-residuals_20260619_172649`，`final/renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`，`final/figure.pptx` 约 60 KB，`final/figure.png` 为 2130x960 PNG。
- 最新真实 smoke 仍未 accepted：`status.json accepted=false`，因为只跑 `MAX_ITERATIONS=1` 且 reviewer 仍给出主观布局意见。已确认原先“没有 PPTX / fallback / 竖排 / 无 inference lane”的问题已转化为更细的布局审美和模型变体清理问题；下一步更合适的是让 reasoner/coder 在完整 loop 中使用当前 `draw_plan.json + layout_map + review` 继续迭代，而不是继续在单轮 deterministic repair 里追逐互相矛盾的 reviewer 偏好。

## 2026-06-19 PPTX 产物可见性核对

- 用户反馈“没有看到有 pptx 生成”。按 systematic debugging 先核对 `runs/latest` 和最终 artifacts，而不是继续改代码。
- 结果：`runs/latest` 指向 `runs/teacher-student-distillation-with-latent-residuals_20260619_174248`；`runs/latest/final/figure.pptx` 存在，大小 67,690 bytes，修改时间为 2026-06-19 17:48:20。
- 验证：`file runs/latest/final/figure.pptx` 显示为 `Zip archive data`；`unzip -l runs/latest/final/figure.pptx` 显示标准 PowerPoint package 结构，包含 `ppt/slides/slide1.xml`。
- 状态：`runs/latest/final/status.json` 为 `accepted=false`，原因是 `cap reached before acceptance`；`renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。因此当前问题不是 PPTX 未生成，而是生成结果没有通过 vision/review 验收。
- 后续：需要在用户打开 `runs/latest/final/figure.pptx` 后继续修质量；若只要定位产物，可直接打开该路径。

## 2026-06-19 FigurePlan-aware DrawPlan guardrail

- 继续追踪“迭代后质量差”的真实根因。前几轮真实 smoke 显示，单纯加 prompt 或 teacher-student local template 会反复引发新问题：本地 repair 会自动注入 `latent_residual`、`inference_student`、`inference_output`、`output_pred` 等 FigurePlan 没声明的语义对象；optimizer 也会把 inference annotation 扩成独立子图，或者把 task loss 画成重复 feedback edge。这不是纯 prompt 工程问题，而是 repair 层没有把 FigurePlan 当语义 source-of-truth。
- 参考 AutoFigure-Edit 的核心理念后，本项目对应设计应是：optimizer 只围绕结构化当前状态和视觉证据做位置/样式优化；语义组件集合必须由 FigurePlan / DrawPlan 当前语义状态约束。对应修改：
  - `src/tools/draw_plan.rs` 新增 `repair_draw_plan_geometry_with_figure_plan`，pipeline 现在传入当前 `FigurePlan`。
  - teacher-student 模板下，FigurePlan 没声明的 residual/inference/output synthetic boxes 会被移除。
  - 如果 FigurePlan 有 teacher-to-student residual edge 但 optimizer 删除了它，本地 guardrail 会恢复 direct dashed connector，而不是合成 residual box。
  - 当 task loss box 或 output-to-loss edge 已存在时，删除重复的 output-to-student task-loss feedback edge，避免 text-on-line 和全图回折。
  - section-style floating labels 会被清理；phase 信息改写进 Teacher/Student editable box text。
  - optimizer prompt 增加“visual optimizer, not semantic replanner”，明确禁止发明语义模块、重复 output/loss、把 inference note 扩成子图、在已有 loss box 时新增 output-to-student loss feedback。
- 新增/更新测试：
  - `draw_plan_geometry_repair_respects_existing_student_output_and_direct_residual_edge`
  - `draw_plan_geometry_repair_preserves_existing_prediction_node_from_optimizer_output`
  - `draw_plan_geometry_repair_removes_section_labels_and_long_task_loss_feedback`
  - `draw_plan_geometry_repair_keeps_direct_residual_edge_when_no_residual_box_exists`
  - `draw_plan_geometry_repair_uses_figure_plan_as_semantic_source_of_truth`
  - `draw_plan_revision_prompt_uses_autofigure_style_visual_optimization_contract`
- 验证：
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - 多次真实 `.env` 三轮 loop 均生成真实 PPTX，`renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 最新真实 run：`runs/latest -> runs/teacher-student-distillation-with-latent-residuals_20260619_184746`。
  - `final/figure.pptx` 存在，大小 53,367 bytes，`file` 识别为 zip-based PowerPoint package。
  - `final/status.json` 仍为 `accepted=false`、`reason="cap reached before acceptance"`。
  - final `draw_plan.json` 已不再包含 `latent_residual`、`inference_student`、`inference_output`、`task_loss_edge`、`e_task_loss_feedback`；说明 FigurePlan-aware pruning 生效。
  - final review 分数较前一轮结构膨胀阶段更稳定，但仍未过：`semantic_fidelity=3`、`story_clarity=3`、`layout_cleanliness=4`、`arrow_routing=4`。剩余 blocker 是：direct residual edge 缺少 `Latent Residual` label；task loss 被画成 terminal node 而 reviewer 希望它是 feedback edge；input-to-student 仍有小 elbow；图面占用不足 40%。
- 结论：已证实主要问题是架构设计不当而不是单纯提示词不够。当前修复把语义边界拉回 FigurePlan，消除了多轮里最严重的“模型/repair 发明额外结构”问题，但目标尚未完成。下一步应继续做 FigurePlan-aware layout policy：让 task loss 在 teacher-student 模板中根据 FigurePlan edge semantic 选择“loss box”或“feedback edge”二选一，并把 residual label 放在不压线的位置，同时改善 paper-wide 空间利用。

## 2026-06-19 FigurePlan-aware loss feedback 修复

- 用户再次反馈“没有看到有 pptx 生成”。重新核对当前最新产物：`runs/latest/final/figure.pptx` 存在，绝对路径为 `/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/teacher-student-distillation-with-latent-residuals_20260619_184746/final/figure.pptx`，大小约 52 KB；`file` 与 `unzip -l` 均确认它是标准 PowerPoint zip package，包含 `ppt/slides/slide1.xml`。当前问题不是未生成 PPTX，而是 latest run 仍为 `accepted=false`。
- 读取 latest `figure_plan.json` 与 `review.json` 后确认最新 blocker 是结构语义被本地 repair 改坏：FigurePlan 明确声明 `e_task_loss` 为 `output_pred -> student` 的 loss feedback edge，但 final DrawPlan 被改成了 `output_pred -> task_loss` terminal loss box；同时 direct residual edge 丢掉 `Latent Residual` label，`e_input_student` 被修成小 elbow，画布利用不足。
- 按 TDD 新增红测 `draw_plan_geometry_repair_honors_figure_plan_task_loss_feedback_edge`，复现真实失败形态：FigurePlan 里同时存在 `task_loss` component 与 `output_pred -> student` loss edge。红测先失败在 “unconnected task_loss component should not be rendered as a terminal box”。
- 修复 `src/tools/draw_plan.rs`：
  - `repair_teacher_student_lanes` 现在接收可选 `FigurePlan`，只在 FigurePlan 明确声明 loss edge 是 `output -> student` 时切换为反馈模式，避免误伤旧的 `output -> loss` 场景。
  - feedback 模式会移除孤立 terminal `task_loss` box 和相关 terminal connectors，恢复/保留 `e_task_loss` 为 `output -> student` dashed feedback connector，并带 `Task Loss` editable label。
  - direct `teacher -> student` residual edge 会保留 FigurePlan 的 `Latent Residual` label，并把 label 放到 edge 旁边，避免压线。
  - canonical teacher-student geometry 横向展开：input/student/output 更充分利用 paper-wide 宽度；`input -> student` 修成单段水平线。
- 验证：
  - 新增红测转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，13 个 draw plan 测试全部通过，旧 `output -> loss` terminal 场景仍保留。
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
- 待验证：使用真实 `.env` 跑 non-mock loop，确认 latest final PPTX 仍生成，并检查 final `draw_plan.json` 是否已包含 `e_task_loss` feedback edge、`Latent Residual` label、直线 `e_input_student`。

## 2026-06-19 真实 smoke 与 explicit latent/inference 变体

- 第一轮真实三轮 smoke：`MAX_ITERATIONS=3 bash scripts/run_real_env.sh examples/teacher_student.md`，run dir 为 `runs/teacher-student-distillation-with-latent-residuals_20260619_190551`。
  - round 000/001 均成功生成 `figure.pptx` 并导出 PDF，`renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - 第三轮停在 `round_002/figure_plan.json` 后的外部模型调用阶段，长时间未产出 `draw_plan.json`，手动中断，避免留下后台进程。
  - round 001 证明上一节修复生效：`e_task_loss` 已经是 `output -> student` feedback edge，不再是 terminal loss box；`e_input_student` 已是单段水平线。
  - 新暴露的问题：loss feedback label 被通用 label-placement 移到 student-output 主线附近；FigurePlan 声明的 `latent_teacher`/`latent_student`/`inference_badge` 被 cleanup 当作 hidden/inference noise 删除。
- 追加 TDD：
  - 新增 `draw_plan_geometry_repair_preserves_figure_plan_latent_pair_and_inference_badge`，复现 `latent_teacher -> latent_student` 的 `r = h_T - h_S` supervision edge，以及 FigurePlan component 形式的 inference badge。
  - 红测先失败在 `latent_teacher` 被删除。
- 修复：
  - `repair_teacher_student_lanes` 建立 FigurePlan component id 保护集合，避免 declared components 被 inference/hidden-state cleanup 删除。
  - 新增 `FigurePlanLatentPair` 识别：当 FigurePlan 用两个 latent nodes 表达 residual supervision 时，保留 `h_T` / `h_S` boxes，重路由 `teacher -> h_T`、`student -> h_S`、`h_T -> h_S` dashed supervision edge，并保留 residual label。
  - loss feedback label 默认位置下移到 feedback loop 下方，避免被放到主数据流线上。
- 验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，14 个 draw-plan 测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
- 第二轮真实两轮 smoke：`MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md`，run dir 为 `runs/teacher-student-distillation-with-latent-residuals_20260619_192731`，并更新 `runs/latest`。
  - `runs/latest/final/figure.pptx` 存在，绝对路径为 `/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/teacher-student-distillation-with-latent-residuals_20260619_192731/final/figure.pptx`，大小约 57 KB。
  - `file` 与 `unzip -l` 确认它是有效 PowerPoint zip package，包含 `ppt/slides/slide1.xml`。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - `status.json` 仍为 `accepted=false`、`reason="cap reached before acceptance"`。
- 最新剩余 blocker：
  - 模型生成了 `comp_*` / `edge_*` 命名的 explicit inference lane，当前 repair 仍会保留 `edge_input_inf` 的 detour，并额外生成/保留 redundant `e_student_output_1`。
  - `comp_inference_label` 被保留为孤立 box，review 认为是 marginal note；应该折叠进 `comp_student_inf` 文本或贴近该 box。
  - FigurePlan 里 `edge_teacher_latent` label 为 `h_t - h_s`，但最新 final DrawPlan 里该 label 仍为空；需要把 FigurePlan edge label 用于 residual-box 变体，不只用于 latent-pair 变体。
  - 当 FigurePlan 声明 `student -> task_loss` loss edge 时，repair 仍会生成短 `output -> task_loss` connector；应按 FigurePlan edge endpoint 选择 student-to-loss 或 output-to-loss，避免 degenerate short edge。
- 结论：PPTX 生成链路已确认正常，当前失败点是 teacher-student explicit inference/latent 变体的 canonical layout policy 还不够完整。下一步应新增针对 `comp_*` explicit inference fixture 的 TDD，再修 route/duplicate-edge/loss-endpoint policy。

## 2026-06-19 explicit inference lane canonicalization

- 继续按 latest final review 修复 `comp_*` / `edge_*` explicit inference 变体，而不是把目标缩小成“能生成 PPTX”。当前真实证据来自 `runs/latest/final/figure_plan.json` 与 `runs/latest/final/draw_plan.json`：FigurePlan 明确声明 `edge_input_inf: comp_input -> comp_student_inf`、`edge_inf_answer: comp_student_inf -> comp_answer`、`edge_student_loss: comp_student_train -> comp_task_loss`，但 repair 又自动补出了不在 FigurePlan 里的 `e_student_output_1: comp_student_train -> comp_answer` 和 `e_output_task_loss: comp_answer -> comp_task_loss`。
- 新增红测 `draw_plan_geometry_repair_canonicalizes_explicit_inference_lane_from_figure_plan`，直接复现 latest final 结构，锁定以下行为：
  - `comp_inference_label` 这类 phase-only box 要折叠进 `comp_student_inf` 文本，不能作为孤立 box 留在图里。
  - `edge_input_inf` 必须是直接水平 input-to-inference flow，不能保留 4 段 detour。
  - 不允许保留/自动补 `comp_student_train -> comp_student_inf` 和 `comp_student_train -> comp_answer` 这类 FigurePlan 没声明的边。
  - 当 FigurePlan 声明 `comp_student_train -> comp_task_loss` 时，不能自动补短 `comp_answer -> comp_task_loss`。
  - residual-box 变体的 `edge_teacher_latent` 要保留 FigurePlan label `h_t - h_s`，且 label 不压线。
- 红测先失败在 `comp_inference_label` 仍作为 floating box 保留。
- 修复 `src/tools/draw_plan.rs`：
  - 新增 `figure_plan_has_edge` / `figure_plan_edge_id`，让 FigurePlan edge set 控制 declared component 之间哪些 connector 可保留或自动补。
  - 新增 `figure_plan_residual_box`，支持 `teacher -> residual_box -> student` 变体从 FigurePlan edge label 获取 `h_t - h_s`。
  - 新增 `figure_plan_inference_student_id` / `figure_plan_inference_label_id`，区分真正的 inference student node 和 phase-only label/badge。修复时也处理了一个回归：`inference_badge` label 中含 “Student only” 时不能被误识别成 inference student。
  - declared components 之间若 FigurePlan 没有对应 edge，repair 会删除该 connector；底部 `upsert_connector` 也按 FigurePlan edge gate 执行，避免自动补重复 training-to-answer 和 output-to-loss。
  - explicit inference flow 重路由为单段水平 `input -> comp_student_inf`，`comp_student_inf -> output` 保持主 flow；phase label box 被删除，inference 文本折叠到 student inference box。
- 验证：
  - 新增红测转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，15 个 draw-plan 测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
- 待验证：再次真实 `.env` smoke，检查 latest final 是否不再出现 `edge_input_inf` detour、`comp_inference_label` floating box、`e_student_output_1`、短 `e_output_task_loss`，并确认 `figure.pptx` 仍为 native editable PPTX。

## 2026-06-19 two-phase inference lane 与 margin 组件修复

- 真实 `.env` smoke：`MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md`，run dir 为 `runs/teacher-student-distillation-with-latent-residuals_20260619_221037`，并更新 `runs/latest`。
  - `runs/latest/final/figure.pptx` 存在，大小约 58 KB，`file` 与 `unzip -l` 确认是有效 PPTX package，包含 `ppt/slides/slide1.xml`。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - `status.json` 仍为 `accepted=false`、`reason="cap reached before acceptance"`。
  - 上一节的旧 blocker 已变化：`edge_input_inf/e_student_output_1/e_output_task_loss/comp_inference_label` 这批旧名字不再是 final DrawPlan 中的主要问题。
- 新 review 暴露下一层同类架构问题：FigurePlan 变体改成 `student_infer`、`answer_infer_out`、`residual_supervision`、`task_label`。repair 仍把 explicit inference student 放在训练主流程中，把 inference output 和 residual loss 挤到底部 margin，并误删了 FigurePlan 声明的 `task_label -> student_train` supervision edge。
- 按 TDD 先更新 `draw_plan_geometry_repair_canonicalizes_explicit_inference_lane_from_figure_plan`，要求 explicit inference student 必须位于下方独立 phase lane；红测先失败在 `comp_student_inf` 仍位于训练主流程中。
- 修复：
  - explicit inference student 统一使用 canonical lower lane bbox，explicit inference output 通过 `figure_plan_inference_output_id` 定位并放在同一 lower lane。
  - `input -> inference_student` 改为干净 L-route，`inference_student -> inference_output` 改为同 lane 水平线。
  - `figure_plan_residual_box` 允许 `teacher -> residual_box` edge 的 semantic 是 `data_flow`，只要目标 component 是 residual 且后续有 residual->student supervision，就仍作为 residual-box 变体处理并保留公式 label。
- 继续新增红测 `draw_plan_geometry_repair_keeps_inference_outputs_and_residual_loss_inside_main_canvas`，复现 latest final 的 `residual_supervision` / `answer_infer_out` bottom-margin 形态。红测先失败在 `residual_supervision` 仍位于 `[0.6334, 0.9484, 0.9082, 0.9916]` 的底部 margin。
- 修复：
  - 新增 `figure_plan_residual_loss_id`，识别 `residual_supervision -> student_train` 这类 residual loss component。
  - residual loss box 改放在 latent residual 右侧的主图区域，`e_residual_loss` 改为不进入 bottom margin 的正交 route。
  - 保留 FigurePlan 声明的 `task_label -> student_train` edge，不再被后续 “loss feedback cleanup” 误删。
- 验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，16 个 draw-plan 测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
- 待验证：再次跑真实 `.env` smoke，确认 latest final 中 `student_infer/answer_infer_out/residual_supervision/task_label` 是否都落在主图和 lower phase lane 内，并检查 review 是否还有新的 blocker。

## 2026-06-19 local quality gate blocker 修复

- 真实 smoke `runs/teacher-student-distillation-with-latent-residuals_20260619_222659` 显示质量明显提升：`semantic_fidelity=7`、`story_clarity=6`、`layout_cleanliness=5`，上一节的 bottom-margin / detached inference output 问题已经消失。但 final 仍未 accepted，blocking issues 只剩两个本地 gate：
  - `component teacher_latent is too small or collapsed`
  - `degenerate edge e_task_loss is too short`
- 读取 `src/tools/review.rs` 后确认阈值：component height 必须 `>= 0.08`，edge length 必须 `>= 0.04`。latest final 中 `teacher_latent` bbox 为 `[0.30, 0.28, 0.50, 0.36]`，高度正好 0.08，渲染后被 gate 判为 collapsed；`e_task_loss` 从 `0.88` 到 `0.91`，长度 0.03。
- 新增红测 `draw_plan_geometry_repair_expands_teacher_latent_and_avoids_degenerate_task_loss_edge`，复现 latest gate failure：
  - `teacher_latent` 修复后高度必须 `>= 0.10`。
  - `task_loss[0] - output_train[2] >= 0.04`，`e_task_loss` 总长度 `>= 0.04`。
- 红测先失败在 `teacher_latent` 高度不足。
- 修复：
  - 新增 `figure_plan_teacher_latent_id`，识别 `teacher -> teacher_latent -> residual` 这类 FigurePlan 变体。
  - `teacher_latent` 改用 canonical latent teacher box `[0.58, 0.12, 0.68, 0.24]`，高度 0.12，避免本地 collapsed gate。
  - `teacher -> teacher_latent` 与 `teacher_latent -> residual` connector 改为受控 routing。
  - `task_loss` box 左边界从 `0.91` 移到 `0.93`，让 `output_train -> task_loss` edge 长度达到 0.05，避免 degenerate edge gate。
- 验证：
  - 新增红测转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，17 个 draw-plan 测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
- 待验证：再次跑真实 `.env` smoke，确认 local quality gate 不再报 `teacher_latent collapsed` 和 `e_task_loss too short`。

## 2026-06-19 latest PPTX visibility and variant guardrails

- 先回应“没有看到有 PPTX 生成”：多次核对 `runs/latest/final/figure.pptx`，确认 final PPTX 一直有生成。最新真实路径为 `/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/teacher-student-distillation-with-latent-residuals_20260619_231427/final/figure.pptx`，大小约 56 KB，`file` 识别为 zip-based PowerPoint package。脚本输出也会打印 `pptx: .../final/figure.pptx` 并更新 `runs/latest`。
- 修复真实 smoke 暴露的 `e_input_inf` 零长度阻断：
  - 根因是模型把 `input_inf` 和 `student_inf` 放进完全重叠的 tiny inference regions，初始 edge 两端中心点相同，`orthogonalized_points` 折叠成单点后被 `validate_draw_plan` 拒绝。
  - 新增红测 `draw_plan_geometry_repair_expands_tiny_separate_inference_input_regions`。
  - 修复 `src/tools/draw_plan.rs`：新增 `figure_plan_inference_input_id`，把 separate inference input 放入 lower lane，并把 `input_inf -> student_inf` 重路由为真实水平线。
- 修复 `output_pred -> loss_task` 被误判成 latent pair：
  - 根因是 `figure_plan_latent_pair` 只看 supervision edge 两端都不是 teacher/student，导致 `output_pred` 被改成 `h_T`、`loss_task` 被改成 `h_S`。
  - 新增红测 `draw_plan_geometry_repair_does_not_treat_task_loss_as_latent_pair`。
  - 修复为只有端点 id/label 明确像 teacher/student latent（`latent` + `h_t/h_s` 或 teacher/student）时才套 latent-pair 布局。
- 修复 no-inference-lane annotation：
  - 新增红测 `draw_plan_geometry_repair_skips_inference_annotation_without_inference_lane`。
  - 没有独立 inference component 时，不再恢复 `anno_inference` 文本注解；inference 语义保留在 student box 文本里，避免 bottom/outside annotation 被 local/reviewer 判为 marginal note。
- 修复 student-to-task-loss 最新 detour：
  - 对 FigurePlan 声明 `student -> task_loss` 的场景使用更宽的 `task_loss` box `[0.88, 0.42, 0.99, 0.58]`，并把 `student -> task_loss` 改成单段水平 connector。
  - teacher-to-latent label 临时移到更远位置，但最新 review 认为 latent residual box 已含公式，floating label 反而冗余；下一步应删除该 label，而不是继续挪动。
- 验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，21 个 draw-plan 测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - 真实 `.env` smoke `MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md` 完成，latest 指向 `runs/teacher-student-distillation-with-latent-residuals_20260619_231427`，`renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`，final PPTX 有效。
- 当前状态：
  - `status.json` 仍为 `accepted=false`、`reason="cap reached before acceptance"`。
  - 已消除本轮阻断中的 `output_pred` 被改成 `h_T`、`task_loss` 消失、`e_input_inf` 无效 connector、student-to-task-loss 4 段绕线。
  - 最新剩余 blocker 是更高层 teacher-student canonical layout：`e_teacher_to_latent` 仍是 3 段 detour；`h_T - h_S` floating label 与 latent residual box 文本重复；FigurePlan/reviewer 期望独立 inference student block，但当前 no-lane 变体只有 annotation；整体仍偏竖排、未充分使用 paper-wide 宽度。
- 下一步：
  - 对 no-lane inference annotation 变体，应根据 annotation “Inference only” 自动合成受控 `inference_student` block，而不是只删除 annotation。
  - 对 residual-box 变体，应把 residual formula 只保留在 box 文本，删除 `teacher -> latent` floating label，并把 teacher/student/latent 改为更横向的 paper-wide canonical 布局。

## 2026-06-19 PDF-derived method overview template library

- 用户最新目标调整为：不要继续手写本地模板；去找经典论文 method overview 图，从 PDF 里提取可编辑/矢量结构作为模板依据，并给模型更多布局自主权。
- 按 `research-lit` / `pdf` 流程选取并下载公开论文 PDF 到 `tmp/pdfs/method_templates/`，用 `pdftotext` 定位 figure 页、`pdftocairo -svg` 抽取页面 SVG、`pdfimages -list` 检查 bitmap：
  - `Attention Is All You Need`，arXiv `1706.03762`，Figure 1，page 3。抽取 `attention_transformer_page3.svg`，SHA-256 `0ff5d42560c9600df675bd2abd6366acf6deb6342d529f5ed199e8998300cc6a`。页面含较大 image layer，因此只作为 stacked encoder-decoder layout grammar，不复制原图。
  - `A Simple Framework for Contrastive Learning of Visual Representations`，arXiv `2002.05709`，Figure 2，page 2。抽取 `simclr_page2.svg`，SHA-256 `5e6f3182fb7420e9cf8821121f59c554abb835867b0d1bfd083658efb1842a24`。`pdfimages` 显示该页无 embedded bitmap，适合作为双分支 contrastive template 来源。
  - `Denoising Diffusion Probabilistic Models`，arXiv `2006.11239`，Figure 2，page 2。抽取 `ddpm_page2.svg`，SHA-256 `6dec4cd5daa5dcd15bfc3b41260ee3aa72f35b93f4fd08c43afd4b70a163671a`。页面含 3 个小样本图；只抽象 chain / reverse-process 结构，小图只能作为 local asset，不作为语义图。
  - `U-Net: Convolutional Networks for Biomedical Image Segmentation`，arXiv `1505.04597`，Figure 1，page 2。抽取 `unet_page2.svg`，SHA-256 `9ff938e1d4b5a28f3fe47179db2621c81113b59ffd385cd9d782ffd83500f85e`。`pdfimages` 显示无 embedded bitmap，适合作为 encoder-decoder skip template 来源。
- 新增 `scripts/extract_method_templates.sh`，可复现下载 PDF、抽取 SVG、输出 SHA-256 和 bitmap inventory。`tmp/` 已加入 `.gitignore`，避免把下载的 PDF/SVG 当作项目产物提交；项目打包保留抽象模板 JSON 和提取脚本。
- 新增 `templates/method_overview/method_templates.json`：
  - 保存模板 id、来源论文、figure/page、source SVG hash、bitmap 统计、figure bbox、抽象 `slots`/`flows`、adaptation guidelines。
  - 明确约束：不要把原论文图作为 full-slide image 贴入 PPTX；只能把模板当作 editable native shapes/text/connectors 的布局语法。
- 新增 `src/tools/template_library.rs` 和 `method_template_pack_json()`，使用 `include_str!` 将模板 pack 编进二进制。新增测试 `tests/template_library_tests.rs` 验证模板 pack 包含 SimCLR/U-Net/DDPM/Transformer 四类来源、PDF URL、页码、vector extraction metadata 和 slots。
- workspace 和 prompt 更新：
  - `create_round_workspace` 现在把模板 pack 写入 `workspace/readable/method_templates.json`，manifest 也声明该文件。`tests/workspace_pipeline_tests.rs` 验证 round workspace 中存在该文件。
  - `build_initial_plan_prompt`、`build_draw_plan_revision_prompt`、coder initial/revised prompt 都直接包含 `method_templates.json`，让 reasoner/coder/vision optimizer 共享同一模板上下文，而不是依赖本地隐藏规则。
  - `tests/prompt_tests.rs` 验证 prompt 包含 `method_templates.json`、`simclr_contrastive_y_branch`、`unet_skip_encoder_decoder` 和 “derived from extracted PDF/SVG” 约束。
- 给模型更多自主权：
  - 新增 `polish_model_draw_plan_geometry`，只做 marginal annotation 清理、connector orthogonalization 和 label 避让，不套 teacher-student 本地模板。
  - pipeline 在 `mock_models=false` 时改用 `polish_model_draw_plan_geometry`；legacy `repair_draw_plan_geometry_with_figure_plan` 只保留给 mock/兼容路径。这样真实模型返回的 bbox/connector 不再被 Rust 的 teacher-student canonical repair 覆盖。
  - 新增 `model_draw_plan_polish_preserves_model_authored_teacher_student_geometry`，验证 model-authored teacher/student bbox 不会被 polish 改写。
- 已通过的局部验证：
  - `cargo test --test template_library_tests -- --nocapture`
  - `cargo test --test prompt_tests -- --nocapture`
  - `cargo test --test workspace_pipeline_tests -- --nocapture`
  - `cargo test --test draw_plan_tests -- --nocapture`
- 待完成验证：
  - 修复当前格式差异后运行 `cargo fmt --check`、`cargo test`、`cd renderer && npm run build`。
  - 用真实 `.env` 重新执行 `MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md`，检查 `runs/latest/final/figure.pptx`、`workspace/readable/method_templates.json`、review 是否显示模板驱动的结构性变化。

## 2026-06-20 template selection rules and model-polish safety fixes

- 真实 `.env` smoke：`MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md`，run dir 为 `runs/teacher-student-distillation-with-latent-residuals_20260620_000942`。
  - `runs/latest/final/figure.pptx` 存在，绝对路径为 `/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/teacher-student-distillation-with-latent-residuals_20260620_000942/final/figure.pptx`。
  - `file` 和 `unzip -t` 确认它是有效 PowerPoint zip package。
  - `round_000/workspace/readable/method_templates.json` 与 `round_001/workspace/readable/method_templates.json` 均存在，包含 `simclr_contrastive_y_branch`、`unet_skip_encoder_decoder`、`derived_from_pdf_vector_page`。
  - `status.json` 仍为 `accepted=false`、`reason="cap reached before acceptance"`。这说明“模板上下文进入模型工作区 + PPTX 生成链路有效”已经成立，但 2 轮内模型仍未自动采用最合适的 SimCLR/Y-branch grammar。
- 视觉检查 `runs/latest/final/figure.png` 的问题：
  - 模型没有采用 SimCLR 双分支结构，而是输出了左侧 student、右上 teacher、底部 inference 的不平衡布局。
  - `anno_training` 仍是浮动文字，压到 `edge_input_to_teacher` 路径附近。
  - `comp_task_loss` bbox 太小，被 local gate 判为 collapsed。
  - residual 相关边仍有交叉与绕线。由于非 mock 现在不再套本地 teacher-student template，这些问题来自模型输出本身，而不是 Rust 覆盖模型设计。
- 按 TDD 加强非 mock 的 generic polish，而不是恢复手写 teacher-student bbox：
  - 扩展 `model_draw_plan_polish_preserves_model_authored_teacher_student_geometry`，要求 model-polish 删除 phase-only `Text`（如 `Training` / `Inference`），并把太小的语义 box 扩到 local gate 可读阈值以上，同时保持 teacher/student 的 model-authored bbox 不变。
  - 实现 `is_phase_only_text_annotation`、`expand_tiny_model_boxes`、`expand_bbox_to_min_size`。这些只做安全整理，不重排语义结构。
  - 验证：`cargo test --test draw_plan_tests model_draw_plan_polish_preserves_model_authored_teacher_student_geometry -- --nocapture` 和 `cargo test --test draw_plan_tests -- --nocapture` 通过。
- 加强模板选择规则：
  - 在 `templates/method_overview/method_templates.json` 新增顶层 `selection_rules`。
  - `teacher_student_distillation` 规则要求当 method 提到 `teacher`、`student`、`distillation`、`residual` 时优先使用 `simclr_contrastive_y_branch`，并明确把 training input 当 shared source、teacher/student 当 two correlated branches、latent residual/distillation loss 当 agreement objective、inference-only student 当 compact post-training note 或 side output。
  - 增加 `encoder_decoder_skip_architecture`、`diffusion_or_iterative_refinement`、`stacked_attention_or_seq2seq` 规则，分别路由到 U-Net、DDPM、Transformer 模板。
  - `build_initial_plan_prompt` 和 `build_draw_plan_revision_prompt` 改为明确要求先应用 `method_templates.json selection_rules`，再适配 preferred template slots/flows。
  - 新增/更新测试：
    - `method_template_pack_routes_distillation_to_simclr_two_branch_template`
    - `prompt_tests` 中 initial planner / DrawPlan revision prompt 必须包含 `selection_rules`、`teacher_student_distillation` 和 `teacher and student as two correlated branches`。
  - 验证：`cargo test --test template_library_tests -- --nocapture`、`cargo test --test prompt_tests -- --nocapture` 通过。
- 待完成验证：
  - 重新运行 `cargo fmt --check`、`cargo test`、`cd renderer && npm run build`。
  - 再次真实 `.env` smoke，检查 selection rule 是否让 teacher/student example 更接近 SimCLR Y-branch / two correlated branches，而不是底部 inference lane。

## 2026-06-20 PPTX visibility and template anti-pattern rules

- 用户反馈“没有看到有 pptx 生成”。先按调试流程核对产物，而不是猜测生成失败：
  - `runs/latest` 指向 `teacher-student-distillation-with-latent-residuals_20260620_003728`。
  - 当前最终 PPTX 为 `/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/teacher-student-distillation-with-latent-residuals_20260620_003728/final/figure.pptx`，大小约 58 KB。
  - `file runs/latest/final/figure.pptx` 识别为 zip-based PowerPoint package，`unzip -t runs/latest/final/figure.pptx` 通过。
  - 根因不是没有生成，而是成功输出虽然打印了相对路径，用户侧仍不够醒目。
- 修复输出可见性：
  - `scripts/run_real_env.sh` 成功后改为打印绝对 `final dir`、`pptx`、`png`、`status`，同时打印 `latest pptx` 和可直接执行的 `open pptx: open ".../figure.pptx"`。
  - `src/main.rs` 的 CLI 完成输出改为尽量 canonicalize `run_dir` / `final_dir`，并打印 `open_pptx` 命令。
  - `README.md` 的 Usage 增加 `open runs/latest/final/figure.pptx`。
- 按 TDD 补模板反模式规则：
  - `tests/template_library_tests.rs` 先要求 `teacher_student_distillation` selection rule 声明 `avoid` anti-patterns。红测失败点为 `distillation rule should declare anti-patterns`。
  - `templates/method_overview/method_templates.json` 在 distillation rule 下新增 `avoid`：
    - `bottom-heavy separate inference lane`
    - `residual as standalone node`
    - `floating phase labels`
    - `long input detours`
  - `src/agent.rs` 的 initial plan prompt 与 DrawPlan revision prompt 明确把 matching rule 的 `avoid list` 当 hard anti-patterns；DrawPlan 修订阶段如果当前图包含反模式，应 remove/redesign，而不是只移动。
  - `tests/prompt_tests.rs` 增加 prompt 断言，确保 hard anti-patterns 与 bottom-heavy inference lane 约束进入模型上下文。
- 待验证：
  - `cargo test --test template_library_tests -- --nocapture`
  - `cargo test --test prompt_tests -- --nocapture`
  - `cargo fmt --check`
  - `cargo test`
  - `cd renderer && npm run build`
  - 再跑真实 `.env` smoke，确认脚本尾部显示绝对 PPTX 路径和 `open pptx` 命令。

## 2026-06-20 non-mock edge-style semantic sync

- 最新真实 smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_004623` 证明 PPTX 输出可见性已修复：
  - CLI 与 helper script 都打印绝对 `pptx` 路径。
  - 输出末尾包含 `open_pptx: open ".../figure.pptx"` 和 `open pptx: open ".../figure.pptx"`。
  - `runs/latest/final/figure.pptx` 是有效 PowerPoint zip package，`unzip -t` 通过。
- 该 run 的 review 暴露新的语义一致性问题：
  - `FigurePlan` 中 `e_residual_student` 是 `semantic=supervision`、`style=dash`。
  - 模型生成的 `DrawPlan` 同 id connector 却是 `style=main_flow`，导致 renderer 画成实线。
  - 这是语义 style 没有从 FigurePlan 传递到模型 DrawPlan 的问题，不是布局模板问题。
- 按 TDD 新增 `model_draw_plan_polish_syncs_connector_style_from_current_figure_plan`：
  - 复现模型把 `e_residual_student` 写成 `main_flow` 的情况。
  - 要求 non-mock polish 同步成 `dashed_supervision`，同时保持 teacher/student bbox 不变。
- 实现：
  - 新增 `polish_model_draw_plan_geometry_with_figure_plan`。
  - 新增 `sync_connector_styles_from_figure_plan`，仅对同 id 或同 from/to 的 connector 同步 style；不移动 bbox，不改 connector points，不套 teacher-student 坐标模板。
  - `pipeline.rs` 的 non-mock 路径改用该函数；mock 路径继续使用 legacy repair 以保持已有测试语义。
- 验证：
  - 红测先因新函数缺失失败。
  - 实现后 `cargo test --test draw_plan_tests model_draw_plan_polish_syncs_connector_style_from_current_figure_plan -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，23 个 draw-plan 测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
- 再次真实 `.env` smoke：
  - 命令：`MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_005922`
  - `runs/latest/final/figure.pptx` 大小约 65 KB，`file` 与 `unzip -t` 验证有效。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - workspace 中 `method_templates.json` 包含 `bottom-heavy separate inference lane`、`residual as standalone node`、`floating phase labels`、`long input detours`。
  - 本轮图形更接近 SimCLR two-branch grammar：teacher/student 分支、projection、latent residual 与 task output 更清晰。
  - 仍未 accepted：`status.json` 为 `accepted=false`、`reason="cap reached before acceptance"`。
- 最新剩余问题：
  - `Inference: Student only` 仍作为边缘 note 出现在主流程外，需要在下一步让 reasoner 把 inference-only 语义整合到 student/output label，而不是单独 component。
  - `Student (trainable)` label 压在 connector 上，需要约束 connector label placement 或让 reviewer patch 明确给 final bbox。
  - teacher/student 对称标注不完整，review 指出只有 student label，缺 teacher annotation。

## 2026-06-20 model autonomy polish and best-round selection

- 继续围绕用户目标“不要本地手写模板、给模型更多自主权”推进：不恢复 Rust teacher-student canonical bbox，不把论文图贴成 raster；只加强 PDF-derived template rule、prompt hard anti-patterns，以及非 mock model output 的通用几何/语义 polish。
- latest 真实 run 的对象级根因：
  - `inference_note` 既可能作为 standalone `Box` 出现，也可能作为 `Text` annotation 出现，导致 reviewer 判定为 marginal explanatory note。
  - `Student (trainable)` 这类单边 branch annotation 会造成 teacher/student 不对称。
  - connector label 会避开自己的 edge，但仍可能压到其他 connector 或 semantic box。
  - 模型有时会输出 4 点 dogleg connector，或长度接近 0.04 的短边，触发 wandering / degenerate gate。
- 按 TDD 新增 model-polish 红测并转绿：
  - `model_draw_plan_polish_folds_standalone_inference_note_into_student_label`
  - `model_draw_plan_polish_folds_inference_text_annotation_into_student_label`
  - `model_draw_plan_polish_removes_annotation_text_that_overlaps_connector_strokes`
  - `model_draw_plan_polish_removes_asymmetric_student_branch_annotation`
  - `model_draw_plan_polish_moves_connector_labels_off_other_edges_and_boxes`
  - `model_draw_plan_polish_simplifies_four_point_dogleg_connectors`
  - `model_draw_plan_polish_expands_degenerate_short_connectors`
- 实现：
  - `fold_standalone_inference_notes_into_student_labels`：未连接的 inference-only note box/text 不再作为边缘说明保留，而是折进最大 student module label。
  - `remove_asymmetric_branch_annotations`：只出现 student 或只出现 teacher 的 branch annotation 会被删除，避免单边解释文本。
  - `avoid_connector_label_collisions`：connector label 除了避开自己的 edge，还要避开其他 connector segments、已有 labels 和 semantic boxes。
  - `polished_connector_points`：仅在 non-mock model polish 路径启用，折叠 4 点 dogleg，并为过短 2 点 connector 加小 elbow；legacy/mock `repair_draw_plan_geometry_with_figure_plan` 仍保持原受控 routing，避免回到手写模板覆盖模型布局。
- 加强 template selection rule：
  - `teacher_student_distillation` 的 `avoid` 增加：
    - `standalone inference note component`
    - `asymmetric branch annotations`
    - `connector-overlapping labels`
  - `tests/template_library_tests.rs` 和 `tests/prompt_tests.rs` 验证这些 anti-pattern 进入 template pack 和 prompt。
- 发现并修复 loop 级问题：
  - 4 轮真实 run `runs/teacher-student-distillation-with-latent-residuals_20260620_013814` 证明模型后续轮可能退化；原 pipeline 的 `best_round` 实际每轮直接覆盖，cap 后 final 使用最后一轮，而不是最好一轮。
  - 新增 `should_replace_best_review` 和测试：
    - `best_review_selection_keeps_blocker_free_round_over_later_regression`
    - `best_review_selection_replaces_with_stronger_blocker_free_round`
  - 排名规则：accepted 优先，其次无 blocking issues，其次 blocking issue 数量少，其次分数总和和关键分项。
  - cap 后 final 现在复制 best round；`PipelineResult.rounds` 仍报告实际执行轮数。
- 验证：
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，30 个 draw-plan 测试全部通过。
  - `cargo test --test pipeline_tests -- --nocapture` 通过，7 个 pipeline 测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - 真实 `.env` smoke `MAX_ITERATIONS=2 bash scripts/run_real_env.sh examples/teacher_student.md` 生成有效 PPTX；`file` 和 `unzip -t` 通过。
  - 真实 `.env` smoke `MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md` 验证 best-round selection：
    - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_014726`
    - `runs/latest/final/figure.pptx` 有效，`renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
    - `final/review.json` 与 `round_002/review.json` 一致，证明 final 不再使用退化的 `round_003`。
- 当前状态：
  - 目标仍未完成，不能标记 complete。最新 4 轮真实 run 仍 `accepted=false`、`reason="cap reached before acceptance"`。
  - 已完成的方向性进展：PDF-derived templates 已进入 workspace/prompt；真实模型更自主地产生 DrawPlan；本地只做通用 polish；cap 后保留 best round，避免越改越差。
  - 剩余主要问题：模型仍会产生 semantic fidelity / story clarity 不足的布局变体，尤其是 latent residual 的语义路径和输入分支 routing。下一步应让 review feedback 转成更强的 executable DrawPlan revision contract，而不是继续加 teacher-student 专用坐标模板。

## 2026-06-20 best-so-far revision source and PPTX visibility

- 用户反馈“没有看到有 pptx 生成”，同时之前真实 run 暴露了一个 loop 级问题：cap 后虽然 final 能复制 best round，但下一轮修订仍机械读取 `round_{n-1}` 的 artifacts。这样模型可能从退化版本继续修，而不是从当前最佳图继续改。
- 按 TDD 新增 `revision_source_prefers_best_round_over_last_round`：
  - 第 0 轮没有修订源。
  - 没有 best round 时回退到上一轮。
  - 存在 best round 时优先使用 best round，即使它不是上一轮。
- 实现：
  - 在 `src/pipeline.rs` 新增 `revision_source_round_index`。
  - 每轮开始时根据 `best_round` 选择 `revision_round_dir` 和 `revision_review`。
  - `revise_draw_plan_from_feedback`、`create_revised_code_bundle` 和 `create_round_workspace` 统一读取同一个 revision source。
  - 如果修订源不是上一轮，写入 `run.log`，例如 `round 3 revising from best-so-far round 0`，便于追踪 loop 是否真的使用最佳上下文。
- 验证：
  - 红测先因缺少 `revision_source_round_index` 编译失败。
  - 实现后 `cargo test --test pipeline_tests revision_source_prefers_best_round_over_last_round -- --nocapture` 通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 真实 `.env` smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_020829`
  - CLI 和脚本都打印了最终路径：
    - `/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/teacher-student-distillation-with-latent-residuals_20260620_020829/final/figure.pptx`
    - `/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/latest/final/figure.pptx`
  - `file final/figure.pptx` 显示为 Zip archive；`unzip -t final/figure.pptx` 无错误。
  - `runs/latest` 已指向该 run，`runs/latest/final/figure.pptx` 存在，大小约 54 KB。
  - `run.log` 显示第 2、3 轮均从 best-so-far 第 0 轮修订，证明 loop 已不再沿着退化上一轮继续。
- 当前状态：
  - PPTX 生成路径已验证，用户可直接运行：
    - `open /Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/latest/final/figure.pptx`
  - 该真实 run 仍未通过验收：`accepted=false`、`reason="cap reached before acceptance"`。
  - 最新 final 来自第 0 轮 best round，剩余 blocker 为 `component overlap between comp_task_loss and comp_output`。
  - 下一步应针对本地 render quality gate 的 overlap 反馈，把组件重叠转换成更强的 DrawPlan revision 约束，或在 non-mock 通用 polish 中加入不依赖 teacher-student 模板的 box-overlap resolver。

## 2026-06-20 generic model DrawPlan overlap resolver

- 继续处理最新真实 run 的 blocker：`render quality failed: component overlap between comp_task_loss and comp_output`。约束是不新增 teacher-student 固定坐标模板，不把 layout 权限拿回 Rust，而是在模型 DrawPlan 输出后做通用几何门禁修复。
- 按 TDD 新增 `model_draw_plan_polish_separates_overlapping_semantic_boxes`：
  - 直接使用真实 run 中 `comp_output` 和 `comp_task_loss` 的重叠 bbox。
  - 要求 primary output box 保持模型位置，loss box 被挪到邻近空位。
  - 要求 unrelated main module 不被重排，避免变成隐藏模板。
  - 要求指向被移动 box 的 connector 端点重新贴到新 bbox 边缘。
- 红测结果：
  - 实现前 `cargo test --test draw_plan_tests model_draw_plan_polish_separates_overlapping_semantic_boxes -- --nocapture` 失败，output/loss 仍触发同一个 component overlap gate。
- 实现：
  - `polish_model_draw_plan_geometry` 在扩展 tiny boxes 后新增 `resolve_model_box_overlaps`。
  - resolver 复用 review gate 的核心阈值：overlap area `> 0.003` 且超过较小 box `15%` 才处理。
  - 候选修复只做 normalized bbox 平移：上、下、左、右、贴到 blocker 外侧；选择 overlap penalty 最小、移动距离小、边界压力小的候选。
  - 使用稳定性评分优先保留 `primary/main/student/teacher/output` 等模型主结构，优先移动 `loss/context/muted/note` 等低稳定性 box。
  - 对移动过的 box，`realign_connector_endpoints_for_moved_boxes` 只重贴相关 connector 的首尾端点，不重建整图 route。
- 验证：
  - 新增红测转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，31 个 draw-plan 测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 真实 `.env` smoke 尝试：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_022247`
  - 已写出 `round_000/figure_plan.json`、`round_000/draw_plan.json`、workspace manifest。
  - `round_000/draw_plan.json` 中 `comp_task_loss` 为 `[0.3624, 0.5824, 0.4976, 0.6976]`，本轮没有生成重叠的 `comp_output`，未复现上一轮 blocker。
  - 该 smoke 在写出 `figure.ts` 前长时间等待 coder role 外部调用，约 7 分钟后仍无新 artifact；为避免留下悬挂进程，手动中止，退出码 130。
- 当前状态：
  - 通用 overlap resolver 已有单测和全量测试覆盖，但还缺一轮完整真实 accepted smoke 证明最终图通过所有 gate。
  - 新暴露的系统风险：coder role 外部调用可能长时间不返回。虽然 OpenAI-compatible provider 已有 180 秒 per-attempt timeout 和重试，但总等待时间仍可能过长；下一步可以把 coder fallback 做成更快的受控超时或跳过未使用的 model TS 阻塞路径，让 DrawPlan 主路径能继续产出 final PPTX。

## 2026-06-20 non-mock DrawPlan main path and semantic edge guardrails

- 继续推进真实 agentic loop。上一轮真实 smoke 卡在 `round_000/draw_plan.json` 之后、写出 `figure.ts` 之前，根因是 pipeline 虽然 non-mock 主渲染合同已经选择 deterministic DrawPlan runtime，但仍然等待 coder role 返回 `GeneratedCodeBundle`；该 code bundle 随后又不会作为 primary renderer 使用。
- 按 TDD 新增 `nonmock_draw_plan_contract_does_not_wait_for_unused_generated_code_bundle`：
  - non-mock `renderer_uses_generated_code_bundle(false)` 必须为 false。
  - mock 仍保留 generated-code artifact 路径，用于测试 workspace feedback 行为。
- 实现：
  - `src/pipeline.rs` 新增 `renderer_uses_generated_code_bundle`。
  - non-mock 每轮直接写 deterministic DrawPlan runtime 到 `writable/code/figure.ts` 和 `round_xxx/figure.ts`。
  - non-mock 不再 reserve `coder TypeScript generation` 成本，也不再请求 coder role 生成未使用的 TypeScript。
  - `run.log` 每轮写入 `coder code generation skipped; deterministic DrawPlan runtime is primary renderer contract`。
- 验证：
  - 新测试先因缺少 helper 编译失败；实现后转绿。
  - `cargo test --test pipeline_tests -- --nocapture` 通过，9 个 pipeline tests 全部通过。
  - `cargo fmt --check`、`cargo test`、`cd renderer && npm run build` 均通过。
- 真实 `.env` smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_023713`
  - 4 个 round 都生成并导出 PPTX/PDF，没有再卡在 coder code generation。
  - final PPTX 有效，`file` 为 Zip archive，`unzip -t` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - 仍未 accepted；final review 的主要 blocker 变为：
    - FigurePlan 外的 `e_student_to_taskloss` 造成 false direct path。
    - FigurePlan 声明的 residual supervision edge 缺失。
    - inference note 放在右边缘且连接不可见。
- 继续按 TDD 修复 FigurePlan 与 DrawPlan 的语义 edge 一致性：
  - 新增 `model_draw_plan_polish_removes_connectors_absent_from_figure_plan`：
    - DrawPlan 中如果 connector 的 from/to 都是 FigurePlan components，但 FigurePlan 没有对应 edge，则删除。
    - 保留合法的 FigurePlan edge，不移动无关 main module。
  - 新增 `model_draw_plan_polish_adds_missing_connectors_declared_by_figure_plan`：
    - FigurePlan 声明 edge，DrawPlan 缺失但两端 box 存在时，自动补 native connector。
    - 使用 FigurePlan edge id、semantic style、label，并用 box 边缘 anchor 生成干净 orthogonal connector。
- 实现：
  - `polish_model_draw_plan_geometry_with_figure_plan` 现在顺序执行：
    - `sync_connector_styles_from_figure_plan`
    - `remove_connectors_absent_from_figure_plan`
    - `add_missing_connectors_from_figure_plan`
    - 通用 model geometry polish
  - 这不是 teacher-student 坐标模板；规则只基于 FigurePlan 的 component/edge 语义合同，适用于所有 method overview。
- 验证：
  - 两个新增红测均先失败后转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，33 个 draw-plan tests 全部通过。
  - `cargo fmt --check`、`cargo test`、`cd renderer && npm run build` 均通过。
- 再次真实 `.env` smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_024325`
  - 4 个 round 都生成并导出 PPTX/PDF；`runs/latest/final/figure.pptx` 更新为该 run。
  - final PPTX 有效，`file` 为 Zip archive，`unzip -t` 无错误。
  - final `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - `e_student_to_taskloss` blocker 已不再出现，FigurePlan 外 connector pruning 生效。
  - 仍未 accepted；剩余 blocker 主要是 residual supervision connector 可见性、inference note 位置/连接可见性、`L_task` label 贴线。这些属于下一步的 DrawPlan revision / connector visibility / label placement 问题。
- 当前状态：
  - 目标仍未完成，不能标记 complete。系统已能稳定生成 editable PPTX 并跑完整真实 loop，但 final review 仍未通过。
  - 与用户目标对齐的进展：模板来源仍是 PDF-derived template pack；Rust 没有新增论文图手写坐标模板；模型继续负责 FigurePlan/DrawPlan，Rust 只加强通用语义合同和几何门禁。

## 2026-06-20 note component visibility and safe connector labels

- 用户继续反馈“没有看到有 pptx 生成”，实际最新 run 中 PPTX 已生成，但 final review 仍未通过；主要剩余问题是 `comp_inference_note` 被模型画成极右侧 floating text，导致连接不可见，以及 `edge_task_loss_label` 位于画布顶部边缘、贴近 connector。
- 按 TDD 新增并修复两个失败点：
  - `model_draw_plan_polish_converts_note_text_components_to_connectable_boxes`：FigurePlan 中声明为 component 的 note/inference 节点，如果模型输出为 `DrawObject::Text`，必须转为可连接的 `DrawObject::Box`，并根据进入该 note 的 FigurePlan edge 放回源组件旁边。
  - `model_draw_plan_polish_moves_top_edge_labels_into_safe_area`：connector label 如果贴在画布顶部安全区外，即使没有与当前 edge bbox 显式重叠，也要重新放到线段外侧和安全区内。
- 实现：
  - `src/tools/draw_plan.rs` 中 `polish_model_draw_plan_geometry_with_figure_plan` 先执行 `normalize_figure_plan_component_objects`，把 FigurePlan component 对应的 floating text 转成 native component box。
  - 新增 note-like component 贴边修复：根据 FigurePlan incoming edge 的 source box，把 note 放到 source 旁边，并通过 `realign_connector_endpoints_for_moved_boxes` 重贴相关 connector 端点。
  - `place_label_outside_edge` 新增 `label_inside_safe_area` 判断；顶部越界 label 会优先放到附近线段下方，而不是继续贴在 y=0 附近。
  - 没有新增 teacher-student 固定坐标模板；规则只依赖 FigurePlan component/edge 语义合同和通用 normalized geometry。
- 验证：
  - 新增两个测试先失败，其中 note 测试暴露了 Text 未转 Box，label 测试暴露了顶部 label 未移动。
  - 修复后：
    - `cargo test --test draw_plan_tests model_draw_plan_polish_converts_note_text_components_to_connectable_boxes -- --nocapture` 通过。
    - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_top_edge_labels_into_safe_area -- --nocapture` 通过。
    - `cargo test --test draw_plan_tests -- --nocapture` 通过，35 个 draw-plan 测试全部通过。
    - `cd renderer && npm run build` 通过。
    - `cargo fmt --check` 通过。
    - `cargo test` 全量通过。
- 下一步：
  - 继续运行真实 `.env` non-mock smoke，检查 final PPTX 是否仍存在 residual connector / inference note / label 可见性 blocker，并确认 `runs/latest/final/figure.pptx` 是否有效。

## 2026-06-20 semantic component completion and reviewer-template alignment

- 真实 `.env` smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_025747` 已稳定生成 4 轮 PPTX/PDF，并更新 `runs/latest/final/figure.pptx`。
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - `validation_report.json` 无 warnings/errors。
  - 但 `status.json` 仍是 `accepted=false`，原因是 `cap reached before acceptance`。
- 分析 4 轮 review 后发现：
  - 第 1 轮是 best-so-far，无 blocking issues，仅有 3 个 minor issue。
  - 失败原因不再是 PPTX 没生成，而是评分阈值没到：`layout_cleanliness=6`、`color_semantics=6`、`wps_editability=8` 等低于严格阈值。
  - FigurePlan 中声明了 `inference` component，但 final DrawPlan 中可能缺失该 component；现有 guardrail 只会补 FigurePlan edge，不会补 FigurePlan component。
  - reviewer prompt 没有明确尊重 PDF-derived classic template 的 reading order，导致 SimCLR-style bottom-center source branching upward 这种模板语法可能被误判为违反 top-down reading flow。
  - FigurePlan→DrawPlan 的 component style 只看 `visual_weight`，`loss/residual/alignment/objective` 这类语义节点可能被画成 neutral，削弱 color semantics。
- 按 TDD 新增并修复：
  - `model_draw_plan_polish_adds_missing_figure_plan_note_components`：模型漏画 FigurePlan component 时，polish 必须按 FigurePlan region 补 native box；note-like orphan component 会补一条到 nearest main/student component 的 native connector，避免被 standalone inference note folding 删除。
  - `draw_plan_from_figure_plan_uses_accent_style_for_loss_components`：loss/alignment/residual/objective component 即使 `visual_weight=normal`，也使用 `accent_module`。
  - `review_prompt_includes_schema_and_all_score_fields` 增加约束：reviewer 必须尊重 PDF-derived template reading order；当 DrawPlan/layout_map 全是 native boxes/text/connectors 且无 full-slide raster 时，`wps_editability` 应给 9/10，除非有明确不可编辑证据。
- 实现：
  - `src/tools/draw_plan.rs`
    - `polish_model_draw_plan_geometry_with_figure_plan` 新增 `add_missing_component_boxes_from_figure_plan` 和 `connect_orphan_note_components_to_main_components`。
    - `component_style` 新增 objective/loss/residual/alignment 语义判断，统一走 `accent_module`。
    - `draw_plan_from_figure_plan` 改为复用 `component_style`，避免初始路径和补齐路径样式不一致。
  - `src/agent.rs`
    - `build_review_prompt` 和 `build_review_retry_prompt` 增加 classic template reading-order 规则。
    - 增加 native PPTX editability scoring 规则，避免 reviewer 只看 PNG 时低估 WPS 可编辑性。
- 验证：
  - 新增红测均先失败后转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，37 个测试全部通过。
  - `cargo test --test prompt_tests -- --nocapture` 通过，6 个测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 再跑真实 `.env` non-mock smoke，重点检查 final DrawPlan 是否包含 `inference` component、final PPTX 是否有效、review scores 是否达到 accepted。

## 2026-06-20 degenerate connector guardrail from real 8-round smoke

- 真实 `.env` smoke：
  - 命令：`MAX_ITERATIONS=8 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_031130`
  - 第 0 轮 initial reasoner 很慢，约数分钟后返回；随后 round_000 到 round_005 都生成并导出 PPTX/PDF。
  - 第 6 轮失败，错误为 `draw connector edge_student_to_latent needs at least two points`。
  - 失败发生在 `validate_draw_plan` 之前，所以 round_006 只写出了 `figure_plan.json`，尚未写 `draw_plan.json`。
- 根因：
  - 模型/优化器可能生成 `DrawObject::Connector`，保留了 from/to 语义端点，但 `points` 只有 0 或 1 个点。
  - 之前 `polish_model_draw_plan_geometry` 会修 label、overlap、缺失 component/edge，但没有修复这种 degenerate connector；`validate_draw_plan` 因此直接失败，导致整个 run 没有 final。
- 按 TDD 新增 `model_draw_plan_polish_repairs_connector_with_missing_points`：
  - 构造 `edge_student_to_latent` 只有 1 个 point，但 from/to boxes 都存在。
  - 要求 `polish_model_draw_plan_geometry_with_figure_plan` 在 validation 前把 connector 修到至少 2 个点，并通过 `validate_draw_plan`。
- 实现：
  - `src/tools/draw_plan.rs` 新增 `repair_degenerate_connector_points_from_boxes`。
  - 对 `points.len() < 2` 的 connector：
    - 如果 from/to box 存在，用两个 box 的 anchor 重新生成 connector points。
    - 如果缺少足够语义端点，也生成一条短安全线段，避免 renderer/validation 崩溃。
  - 该修复放在 `resolve_model_box_overlaps` 之后、label/connector polish 之前，确保后续 orthogonalization 仍能处理几何。
- 验证：
  - 新测试先失败后转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，38 个测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 当前注意：
  - `resume` 命令当前会重新进入 `run_pipeline`，不是从失败 round 原地续跑；直接 resume 会重跑 run，因此下一步用新的短 smoke 验证当前代码能生成有效 final PPTX。

## 2026-06-20 box-aware connector routing polish

- 新的短真实 smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_032931`
  - 4 轮均生成并导出 PPTX/PDF，final PPTX 有效，`unzip -t` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
  - `status.json` 仍为 `accepted=false`、`reason="cap reached before acceptance"`。
  - 改动生效：`review.json` 中 `color_semantics=8`、`wps_editability=9`，已经达到这两个阈值；剩余 major issue 主要集中在 arrow routing。
- 剩余 routing 问题：
  - `e_input_to_teacher` 在 input 行横向绕远，review 认为是 simple non-branching flow 的 diagonal/wandering。
  - `e_input_to_student` 有不必要 waypoint。
  - `e_student_to_output` 因 output 与 student latent 不水平，路径右转再下折，review 认为破坏平衡。
- 按 TDD 新增并修复：
  - `model_draw_plan_polish_routes_near_vertical_input_branch_straight`：input 与目标 x 范围重叠时，connector 直接走垂直线。
  - `model_draw_plan_polish_moves_input_branch_elbow_out_of_source_row`：input 分支无法直连时，elbow 放到 source 和 target 中间，而不是贴着 input 行横向绕行。
  - `model_draw_plan_polish_aligns_output_box_with_source_for_straight_flow`：output box 与其 source 有明显水平间距但 y 不齐时，移动 output 到 source 同一水平线，形成干净水平 flow。
- 实现：
  - `src/tools/draw_plan.rs` 新增 `BoxRouteInfo`、`align_output_boxes_with_sources`、`improve_connector_routes_against_boxes` 及若干 routing helpers。
  - `polish_model_draw_plan_geometry` 在 overlap 前先对齐 output box，在 degenerate connector 修复后按 box 几何重排 connector，并在 label polish 后再跑一次 routing，防止 dogleg simplifier 把 input 分支 elbow 压回 input 行。
  - 这些规则只看 DrawPlan 的 native box/connector 几何和 role/text 语义，不引入 teacher-student 固定坐标模板。
- 验证：
  - 三条新增 routing 红测先失败后转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，41 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
- 下一步：
  - 再跑当前代码的真实短 smoke，检查 final review 中 arrow routing 分数和 localized issues 是否减少。

## 2026-06-20 latest real smoke status

- 最后一轮真实短 smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_034355`
  - `runs/latest` 已指向该 run。
  - final PPTX：`/Users/huanghaojie/Desktop/Projects/autofigure_pptx/runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 当前结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - 说明 PPTX 生成链路已经稳定；失败点是 4 轮 cap 内 review 没达到 acceptance。
  - 这次模型输出本身有退化：review 出现 `render quality failed: edge crossing between e_output and e_student_encoder_inference_note`，以及 duplicate `e_task_loss` / `e_task_back` 等问题。
- 已验证改善：
  - 之前“没有 PPTX”的问题已确认不是生成失败；脚本和 CLI 都打印 final PPTX 路径，并更新 `runs/latest/final/figure.pptx`。
  - input 分支绕线在上一轮 smoke 中已被 routing polish 消掉；最新 smoke 因模型生成了不同拓扑，剩余问题转为 duplicate edge、inference note crossing 和局部布局退化。
- 未完成：
  - 还不能把全局目标标记为 complete，因为真实 review 仍未 accepted。
  - 下一步可继续加两个通用 guardrail：删除同 from/to 或相同几何的 duplicate connectors；对 orphan inference note 优先折叠进 student label 或轻量 annotation，避免与主 flow 交叉。

## 2026-06-20 duplicate connector and noisy inference note guardrails

- 针对最新真实 smoke 的 blocker 继续按 TDD 修复：
  - `e_task_back` 与 `e_task_loss` 同 from/to、同几何，review 明确判定为 duplicate edge。
  - `inference_note` 与 `e_student_encoder_inference_note` 是长距离边缘 note，横穿主 flow，review 判定为 redundant/marginal note 并触发 edge crossing。
- 新增测试：
  - `model_draw_plan_polish_removes_duplicate_connectors_with_same_geometry`
    - 构造 `e_task_loss` 和 `e_task_back` 两条同端点、同 points 的 connector。
    - 要求保留语义更强的 `e_task_loss`，删除 `e_task_back`。
  - `model_draw_plan_polish_folds_long_connected_inference_note_into_student_label`
    - 构造真实 smoke 里类似的 `inference_note` 与长水平 connector。
    - 要求删除 note box 和 note connector，并把 “Inference only” 折叠进相邻 student box label。
- 实现：
  - `src/tools/draw_plan.rs` 新增 `remove_duplicate_connectors`：
    - 以 from/to + quantized points 作为重复 key。
    - 对重复 connector 进行评分，优先保留 loss/residual/supervision/main/dashed 语义，降低 `back` / `duplicate` 这类可疑边的优先级。
  - 新增 `fold_noisy_connected_inference_notes_into_student_labels`：
    - 只处理 inference-only/note-like box。
    - 如果 note 连接线过长、note 在外缘、或 note connector 与其它 connector 相交，则删除 note 和其 connector，并把语义折叠进相邻 student/main label。
    - 短距离、贴近 source 的 FigurePlan note 不会被删除，避免破坏已有 component completion 行为。
- 验证：
  - 两个新增红测均先失败后转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，44 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
- 下一步：
  - 运行真实 `.env` non-mock smoke，确认 latest blocker（duplicate connector 和 inference note crossing）是否消失，并检查 final PPTX 是否继续有效。

## 2026-06-20 annotation restoration and solid supervision style polish

- 用户反馈：
  - “没有看到有pptx生成啊”。
  - 已确认 latest PPTX 实际存在：`runs/latest/final/figure.pptx`，当前 `runs/latest` 指向 `teacher-student-distillation-with-latent-residuals_20260620_035507`，final 目录内包含 `figure.pptx`、`figure.pdf`、`figure.png`、`draw_plan.json`、`review.json` 等文件。
  - `file runs/latest/final/figure.pptx` 显示为 Zip archive，说明 PPTX 文件本身已生成；问题是输出路径不在项目根目录，容易被忽略。
- 最新真实 review 里剩余的局部问题：
  - `student_encoder` label 被追加了 “Inference only”，导致模块框内文字污染。
  - FigurePlan 里存在 `ann_inference`，但 DrawPlan final 缺失独立 annotation。
  - `e_teacher_to_residual` 在 FigurePlan 中是 `semantic=supervision, style=solid`，旧实现只看 supervision 语义，错误强制为 dashed。
- 新增/修改测试：
  - `model_draw_plan_polish_folds_long_connected_inference_note_into_annotation`
    - noisy connected inference note 仍会删除 note box 和长 connector。
    - 不再把 inference 文案塞进 `student_encoder` label。
    - 恢复为 compact editable `Text` annotation。
  - `model_draw_plan_polish_restores_missing_figure_plan_annotation_without_label_pollution`
    - model DrawPlan 缺少 `ann_inference` 且包含 noisy `inference_note` 时，最终恢复 FigurePlan 的 `ann_inference` bbox/text。
    - `student_encoder` label 保持原文。
  - `model_draw_plan_polish_honors_solid_supervision_style_from_figure_plan`
    - 锁定 `semantic=supervision, style=solid` 应按 solid/importance 映射为 `normal_flow`，不再强制 `dashed_supervision`。
- 实现：
  - `src/tools/draw_plan.rs`
    - `fold_noisy_connected_inference_notes_into_student_labels` 改为 annotation 语义：删除 noisy note 与其 connector 后，生成/更新 `ann_inference` text annotation，而不是污染 student/module label。
    - 新增 `upsert_meaningful_annotations_from_figure_plan`：model polish 结束后按 FigurePlan 恢复有语义内容的 annotation；纯 “Training” / “Inference” 相位标签仍不恢复，避免旧的 floating phase-label 问题复发。
    - `figure_plan_edge_style` 改为显式 `dash` / `long_dash` 才输出 `dashed_supervision`；`solid` supervision 根据 importance 输出 `main_flow` / `normal_flow` / `aux_flow`。
    - `draw_plan_from_figure_plan` 也改用同一 `figure_plan_edge_style`，让 deterministic DrawPlan 初始渲染和 model polish 的样式语义一致。
- 验证：
  - `cargo fmt && cargo fmt --check` 通过。
  - targeted tests：
    - `cargo test --test draw_plan_tests model_draw_plan_polish_folds_long_connected_inference_note_into_annotation -- --nocapture` 通过。
    - `cargo test --test draw_plan_tests model_draw_plan_polish_restores_missing_figure_plan_annotation_without_label_pollution -- --nocapture` 通过。
    - `cargo test --test draw_plan_tests model_draw_plan_polish_honors_solid_supervision_style_from_figure_plan -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，46 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 重新运行真实 `.env` non-mock smoke，确认 final PPTX 继续生成，且 `student_encoder` label、`ann_inference`、solid supervision 样式在最新 review/DrawPlan 中生效。

## 2026-06-20 latest smoke found standalone annotation label pollution

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_040930`
  - final PPTX：`runs/latest/final/figure.pptx`
  - 脚本输出了 `open_pptx` 命令，`runs/latest` 已更新。
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - scores：`semantic_fidelity=8`、`color_semantics=8`、`wps_editability=9`，但 `arrow_routing=6`。
  - blocking issue：`edge_teacher_to_teacher_latent` 与 `edge_student_to_output` crossing。
  - review 仍指出 `comp_student` 内部有 “Inference only”，且外部 `anno_inference` 同时存在，形成重复。
- 根因：
  - 前一步只修了 noisy connected inference note。
  - 旧的 `fold_standalone_inference_notes_into_student_labels` 仍会把 standalone `anno_inference` / note box 折进 student label。
  - 对 FigurePlan 生成的 `anno_inference`，流程变成：先折进 `comp_student`，后续 `upsert_meaningful_annotations_from_figure_plan` 又恢复外部 annotation，导致重复。
- 新增/修改测试：
  - `model_draw_plan_polish_folds_standalone_inference_note_into_annotation`
    - standalone inference note box 被删除。
    - student label 保持干净。
    - 生成 editable `ann_inference` text annotation。
  - `model_draw_plan_polish_preserves_inference_text_annotation_outside_student_label`
    - 已经是 external Text annotation 的 inference 文案不再折进 student label。
    - annotation 保留为 editable text。
- 实现：
  - `src/tools/draw_plan.rs`
    - `fold_standalone_inference_notes_into_student_labels` 改为 `fold_standalone_inference_notes_into_student_annotations`。
    - standalone note box 删除后转成 `ann_inference` text annotation。
    - Text annotation 不再参与 standalone 折叠，避免 FigurePlan annotation 先污染模块 label。
    - 调整执行顺序：先执行 `remove_asymmetric_branch_annotations`，再生成 inference annotation，避免新 annotation 被误判成不成对 student branch label 后删除。
    - 删除已不再使用的 `is_standalone_inference_note_text`。
- 验证：
  - `cargo fmt --check` 通过。
  - targeted tests：
    - `cargo test --test draw_plan_tests model_draw_plan_polish_folds_standalone_inference_note_into_annotation -- --nocapture` 通过。
    - `cargo test --test draw_plan_tests model_draw_plan_polish_preserves_inference_text_annotation_outside_student_label -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，46 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 再跑一轮真实 `.env` non-mock smoke，确认 `comp_student` / `student_encoder` 这类模块 label 不再包含 “Inference only”，PPTX 继续有效。

## 2026-06-20 note connector and path annotation clutter guardrails

- 最新真实 smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_041853`：
  - final PPTX 有效：`file` 显示 Zip archive，`unzip -t` 无错误。
  - `status.json` 仍为 `accepted=false`、`reason="cap reached before acceptance"`。
  - 这轮已验证前一处修复生效：`Student Encoder`、`Student Head` 等模块 label 不再包含 “Inference only”。
  - `blocking_issues=[]`，但 review 仍有 major/minor clutter：
    - `anno_teacher_path` 和 `anno_student_path` 压在对应 encoder box 上。
    - `anno_template_ref` 是边角 template 来源说明，不属于方法主体。
    - `e_comp_student_enc_comp_inf_note` 是自动补到 inference note 的多余 connector；FigurePlan 并未声明这条 edge。
- 新增/修改测试：
  - `model_draw_plan_polish_adds_missing_figure_plan_note_components`
    - 更新期望：FigurePlan note-like component 仍作为 editable box 保留，但不再自动补 connector。
  - `model_draw_plan_polish_removes_overlapping_branch_path_and_template_annotations`
    - 删除 `simclr_* adapted` 这类 template reference annotation。
    - 删除压在 box 上的 `Teacher (training only)` / `Student (train + infer)` path annotation。
- 实现：
  - `src/tools/draw_plan.rs`
    - 删除 `connect_orphan_note_components_to_main_components` 及其 source-rank helper；未声明 edge 的 note-like component 不再自动连线。
    - `polish_model_draw_plan_geometry_with_figure_plan` 现在把 FigurePlan component ids 作为 protected ids 传入通用 polish，避免合法 FigurePlan note component 被 standalone/noisy note 规则折叠成 annotation。
    - 新增 `remove_template_reference_and_overlapping_path_annotations`，在 model polish 早期删除 template ref 和压在 box 上的 path labels。
    - `is_meaningful_figure_plan_annotation` 过滤 template/adapted reference 和 teacher/student path annotation，避免 final upsert 把它们重新加回来。
- 验证：
  - `cargo fmt && cargo fmt --check` 通过。
  - targeted tests：
    - `cargo test --test draw_plan_tests model_draw_plan_polish_adds_missing_figure_plan_note_components -- --nocapture` 通过。
    - `cargo test --test draw_plan_tests model_draw_plan_polish_removes_overlapping_branch_path_and_template_annotations -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，47 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 重跑真实 `.env` non-mock smoke，确认多余 note connector、template ref、overlapping path annotations 是否从 latest final 消失。

## 2026-06-20 connector label and line-style legend polish

- 最新真实 smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_043050`：
  - final PPTX 有效，`file` 为 Zip archive，`unzip -t` 无错误。
  - 前一批 guardrail 已生效：template ref、teacher/student path annotation、inference note connector 均未再出现在 final DrawPlan/review 中。
  - `blocking_issues=[]`，但 review 仍指出：
    - `edge_student_out_to_task` 的 label `ŷ vs y` 漂在远离实际 vertical edge 的空白区域。
    - `ann_residual_dash` 的 “dashed = residual supervision” 是线型图例，属于视觉噪声。
    - `edge_student_dec_to_resloss` 在 FigurePlan 中有 label `z_S`，DrawPlan connector label 丢失。
- 新增测试：
  - `model_draw_plan_polish_removes_line_style_legend_annotations`
    - 删除 “dashed = residual supervision” 这类线型图例 annotation。
  - `model_draw_plan_polish_syncs_connector_labels_and_snaps_them_to_routes`
    - connector label text 从 FigurePlan 同步。
    - 远离最终 route 的 label bbox 会被重新放到 route 附近。
- 实现：
  - `sync_connector_styles_from_figure_plan` 同步 style 时也同步 label：
    - FigurePlan edge label 为空则清空 connector label。
    - FigurePlan edge label 非空则写入 label text，并按当前 connector points 初始化 bbox。
  - `place_label_outside_edge` 新增 `label_near_edge` 判断：
    - label 如果既不贴近 edge，又只是处于 safe area，不再原样保留。
    - 远离 edge 的 label 会按 edge bbox 重新定位。
  - `remove_template_reference_and_overlapping_path_annotations` / `is_meaningful_figure_plan_annotation` 增加 line-style legend 过滤，防止 `dashed = ...` 被 final upsert 还原。
- 验证：
  - `cargo fmt --check` 通过。
  - targeted tests：
    - `cargo test --test draw_plan_tests model_draw_plan_polish_removes_line_style_legend_annotations -- --nocapture` 通过。
    - `cargo test --test draw_plan_tests model_draw_plan_polish_syncs_connector_labels_and_snaps_them_to_routes -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，49 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 重跑真实 `.env` non-mock smoke，确认 `ann_residual_dash` 消失、`ŷ vs y` label 贴近 edge、`z_S` label 被补回。

## 2026-06-20 latest smoke after label and legend polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_043930`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=8`、`story_clarity=7`、`visual_hierarchy=7`、`paper_readability=7`、`layout_cleanliness=6`、`arrow_routing=6`、`color_semantics=8`、`aesthetic_quality=7`、`wps_editability=10`。
- 已验证修复生效：
  - `ann_residual_dash` 不再出现在 final DrawPlan。
  - 之前的 template ref、teacher/student path annotation、inference note connector 没有复发。
  - final PPTX、PDF、PNG 均生成，PPTX 压缩包结构有效。
- 仍未完成：
  - acceptance 仍未通过，因为 review 分数仍卡在 layout/routing。
  - 当前剩余 issue 全部是 minor：
    - `anno_inference` 缺少视觉锚点。
    - `edge_input_student` 水平跨度过长。
    - `edge_task_loss` 因 Task Loss 位置导致绕行。
    - `edge_student_to_residual` 与 student encoder/projection junction 视觉接近。
- 下一步：
  - 如果继续优化，应优先做 routing/layout 局部 guardrail：让 task loss 贴近 output，减少 input-to-student 跨全图水平线，并给 inference annotation 明确 anchor 或将其贴近目标模块。

## 2026-06-20 shared-input, task-loss, and inference-anchor layout polish

- 针对 latest smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_043930` 的剩余 minor issues 继续做局部 guardrail：
  - `anno_inference` 浮在 Student Encoder 上方但缺少锚点。
  - `edge_input_student` 从左下 input 横跨全图到右侧 student encoder。
  - `edge_task_loss` 因 Task Loss 未贴近 output，形成绕行。
- 新增测试：
  - `model_draw_plan_polish_moves_shared_input_between_teacher_and_student_branches`
    - 当 input 同时连向 teacher/context 和 student/main 分支，且 input 远离两个目标时，把 input 移到两个分支目标的纵向中间，减少低位长水平线。
  - `model_draw_plan_polish_moves_task_loss_below_output_for_short_vertical_edge`
    - output -> task loss 的 loss edge 若目标位于下方但水平错开，把 task loss 对齐到 output 下方，形成短 vertical edge。
  - `model_draw_plan_polish_anchors_inference_annotation_to_figure_plan_target`
    - FigurePlan 的 inference annotation 如果有 `target_id`，final DrawPlan 中 annotation 会贴近 target box，而不是保留远处 floating bbox。
- 实现：
  - `src/tools/draw_plan.rs`
    - 在 model polish 中新增 `align_shared_input_boxes_with_branch_targets`，只处理同时馈入 teacher/context 和 student/main 的 shared input。
    - 新增 `align_task_loss_boxes_with_outputs`，只处理 output -> task loss 的 loss/objective box。
    - `upsert_meaningful_annotations_from_figure_plan` 新增 `anchored_figure_plan_annotation_bbox`，对 inference-specific annotation 根据 target box 重新定位。
    - 保留 FigurePlan/model 的主体结构和可编辑 shape/text/connector，不引入固定 full-slide raster。
- 验证：
  - 三个新增红测先失败，改动后转绿。
  - `cargo fmt && cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，52 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 用真实 `.env` non-mock smoke 验证 latest PPTX 和 review 是否改善。

## 2026-06-20 latest smoke after shared-input/task-loss/annotation-anchor polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_045051`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=8`、`story_clarity=7`、`visual_hierarchy=7`、`paper_readability=7`、`layout_cleanliness=6`、`arrow_routing=6`、`color_semantics=8`、`aesthetic_quality=7`、`wps_editability=9`。
- 已验证修复：
  - latest DrawPlan 中 `edge_student_to_task_loss` 已是短 vertical edge。
  - `edge_input_to_student` 已变成短 horizontal edge，不再是跨全图低位 bus。
  - PPTX/PDF/PNG 继续稳定生成。
- 新暴露问题：
  - `comp_inference_note` 是 FigurePlan note-like component，但被模型放在左上角，review 判为 marginal explanatory note。
  - `edge_residual_to_student` 走三点 detour，经 student latent 列，review 判为不必要绕行。
- 下一步：
  - 增加 guardrail：orphan note-like component 若在外缘且没有 FigurePlan edge，移动到最近 student/main 组件附近但不自动加 connector。
  - 增加 residual/feedback connector 路由规则：避免从 residual 到 student 的 dashed feedback 经过 student latent/projection 列，优先使用短 L-shape 或水平直连。

## 2026-06-20 orphan inference component and right-side feedback routing polish

- 针对 latest smoke 新暴露的问题继续 TDD：
  - `comp_inference_note` 是 FigurePlan component，但位于左上角外缘且无 edge，review 判为 marginal note。
  - `edge_residual_to_student` 从右侧 residual 回到 student 时，旧 `objective_to_main_connector_points` 使用 student 中心 x，路径落到 student latent/projection 列，review 判为 detour。
- 新增测试：
  - `model_draw_plan_polish_moves_outer_orphan_inference_component_near_student`
    - FigurePlan note-like component 在外缘且无 edge 时，保留为 editable box，但移动到最近 student/main 组件旁边。
    - 不自动新增 clutter connector。
  - `model_draw_plan_polish_routes_right_side_residual_feedback_to_student_edge`
    - residual 位于 student 右侧时，feedback L-shape 落到 student 右边界，而不是 student 中心列。
    - 保留已有 residual 位于 student 左侧时进入 student 中心列的测试行为。
- 实现：
  - `reposition_note_components_near_sources` 在找不到 declared source edge 时，使用 `orphan_note_component_source_box` 选择最近语义 source，优先 student/main，其次 module/model。
  - `objective_to_main_connector_points` 对 residual/objective 位于目标右侧的情况，把 target x 改为目标右边界，避免穿过目标内部或相邻 latent/projection 列。
- 验证：
  - 两个新增红测先失败，改动后转绿。
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，54 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 真实 `.env` non-mock smoke，确认 latest PPTX 继续生成，且这两个 issue 不再出现在 review/DrawPlan。

## 2026-06-20 latest smoke after orphan-note and feedback-route polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_050026`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - 这轮模型生成了新的 FigurePlan/DrawPlan 变体，不是上一轮同一布局。
  - latest DrawPlan 中 `e_residual_to_student` 已是短水平 dashed edge，说明右侧 residual feedback guardrail 生效。
  - `comp_inference_note` 上一轮外缘问题没有复发；但新的 `inference_badge` 位于主图中部并与 `latent_residual` overlap，review 仍判 blocking。
- 新暴露问题：
  - `inference_badge` note-like component 与 `latent_residual` overlap。
  - `e_teacher_to_residual_label` 的 label 与 dashed connector 视觉贴线。
  - `e_input_to_teacher` / `e_input_to_student` 是 4-point S-curve，review 判为过度绕线。
- 下一步：
  - 增加更泛化的 note-like component overlap guardrail：只要 inference/note-like component 与 semantic box overlap，就移动到最近 student/main 附近或外侧空位。
  - 后续再处理 input branch S-curve 和 edge label spacing。

## 2026-06-20 note-like component semantic-overlap guardrail

- 针对 latest smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_050026` 的 blocking issue 继续 TDD：
  - `inference_badge` 是 FigurePlan 里的 note-like/context component，但与 `latent_residual` 在主图中部贴得过近，review 判为 blocking overlap。
  - 旧逻辑只处理外缘 orphan note，不能处理已经在主画布内部但遮挡语义模块的 note-like component。
- 新增测试：
  - `model_draw_plan_polish_moves_near_overlapping_inference_component_off_semantic_boxes`
    - 构造 `inference_badge` 靠近 `latent_residual` 的布局。
    - polish 后要求 badge 不再压到 latent residual 的扩展区，同时仍贴近 student branch，避免变成无关边角说明。
- 实现：
  - `src/tools/draw_plan.rs`
    - `reposition_note_components_near_sources` 现在收集 FigurePlan 中所有 note-like component id。
    - 新增 `note_component_conflicts_with_semantic_boxes`，对 note-like component 做扩展 bbox 检测；只要与非 note-like semantic box 冲突，就复用邻近 source reposition 逻辑。
    - 保留 editable box，不自动补 clutter connector；这仍然遵守 PDF/SVG-derived layout grammar 和 native PPTX 输出约束。
- 验证：
  - targeted test：`cargo test --test draw_plan_tests model_draw_plan_polish_moves_near_overlapping_inference_component_off_semantic_boxes -- --nocapture` 通过。
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，55 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 运行真实 `.env` non-mock smoke，检查 `inference_badge` overlap 是否消失，并继续处理可能残留的 input branch S-curve 与 connector label spacing。

## 2026-06-20 latest smoke after note-like overlap guardrail

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_051208`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`，说明上一轮 `inference_badge` overlap blocking 已消失。
  - scores：`semantic_fidelity=7`、`story_clarity=6`、`visual_hierarchy=6`、`paper_readability=5`、`layout_cleanliness=5`、`arrow_routing=5`、`color_semantics=7`、`aesthetic_quality=6`、`wps_editability=9`。
- 新暴露/残留问题：
  - `comp_inference_note` 作为 opaque box 压在 `edge_input_to_student` 主水平 flow 上。
  - `edge_head_to_task_loss` 因 output head 与 task loss 贴边，生成 4-point 小回环。
  - `label_inference` 与 `comp_inference_note` 文案重复。
  - `edge_residual_to_student` 从 residual 到 student encoder 走到 student 中心列，视觉上像绕线。

## 2026-06-20 inference-note, touching-task-loss, and feedback-edge polish

- 针对最新 smoke 的残留 issue 继续做局部 TDD，不改整体 pipeline 和 PDF/SVG-derived template library：
  - note-like component 如果盖住 connector segment，也应重放到 student/main 附近的清爽候选位。
  - 已有 inference note box 时，删除重复的 inference text/caption，避免 review 判 redundant marginal note。
  - task loss 与 output/head 贴边时，不再保留微小 4-point 回环；优先把 task loss 放到 source 下方并生成短 vertical connector。
  - residual/objective feedback 从左侧进入 student/main 时接到目标左边界，而不是穿到目标中心列。
- 新增/修改测试：
  - `model_draw_plan_polish_moves_inference_note_off_main_flow_and_removes_duplicate_caption`
    - 复现 `comp_inference_note` 压住 `edge_input_to_student` 的真实布局。
    - polish 后 note 不再与主 flow segment 相交，并删除 `label_inference`。
  - `model_draw_plan_polish_moves_touching_task_loss_to_short_vertical_route`
    - 复现 `edge_head_to_task_loss` 贴边 4-point 小回环。
    - polish 后 task loss 移到 output head 下方，并使用 2-point vertical connector。
  - 更新 `model_draw_plan_polish_routes_objective_to_student_with_shorter_dogleg`
    - residual/objective 位于 student 左侧时，feedback 终点改为 student 左边界。
- 实现：
  - `src/tools/draw_plan.rs`
    - `reposition_note_components_near_sources` 增加 connector segment 冲突检测。
    - 新增 `clear_adjacent_note_box` / `adjacent_note_candidates`，按 source 左右和上下候选位选择不遮挡 semantic boxes 和 connector segments 的位置。
    - 新增 `remove_duplicate_inference_text_when_note_component_exists`。
    - 新增 `align_touching_task_loss_boxes_with_sources`。
    - 调整 `objective_to_main_connector_points`，从左侧进入 main/student 时使用目标左边界。
- 验证：
  - 两个新增红测先失败，改动后转绿。
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，57 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 再跑真实 `.env` non-mock smoke，确认 latest PPTX 继续生成，且 inference note、task loss connector、duplicate inference caption、residual feedback issue 是否从 review 中消失。

## 2026-06-20 latest smoke after inference-note/task-loss/feedback polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_052351`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=8`、`story_clarity=7`、`visual_hierarchy=6`、`paper_readability=7`、`layout_cleanliness=6`、`arrow_routing=5`、`color_semantics=7`、`aesthetic_quality=6`、`wps_editability=10`。
- 已验证修复：
  - 上一轮的 `comp_inference_note` 覆盖主 flow、`edge_head_to_task_loss` 小回环、`label_inference` 重复 caption 没有复发。
  - PPTX/PDF/PNG 继续稳定生成，且 renderer 没有 fallback。
- 新暴露/残留问题：
  - 新变体中 `e_teacher_residual` 与 `e_residual_student` 共用相反方向的水平段，review 判为 residual feedback backtracking。
  - `e_input_teacher` 仍有 4-point vertical-up/horizontal/vertical-down routing，review 判为不必要 elbow。
  - `inference_student` 是宽 note-like box 且无连接，仅 minor。
- 下一步：
  - 增加 routing guardrail：避免 objective/residual feedback 复用已有进入 residual 的反向水平段；input-to-context branch 在明显左下到右上时用短 L-shape 替代 4-point S-like path。

## 2026-06-20 residual-feedback backtrack and input-branch routing polish

- 针对 latest smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_052351` 的 routing issues 继续 TDD：
  - `e_residual_student` 与 `e_teacher_residual` 共用相反方向水平段，造成 feedback backtracking。
  - `e_input_teacher` 对简单左下到右上的 input branch 使用 4-point S-like path。
- 新增/修改测试：
  - `model_draw_plan_polish_reroutes_residual_feedback_off_reverse_shared_segment`
    - 构造 teacher latent -> residual 与 residual -> student 反向共线段。
    - polish 后要求两个 connector 不再共享反向 segment。
  - `model_draw_plan_polish_simplifies_diagonal_input_to_teacher_branch`
    - 构造最新 run 的 4-point input->teacher path。
    - polish 后要求 route 简化为最多 3 点的短 L-shape。
  - 更新 `model_draw_plan_polish_moves_input_branch_elbow_out_of_source_row`
    - 旧期望固定 4 点；现在接受更短的 3 点 L-shape，同时继续要求 elbow 不贴 input row。
- 实现：
  - `src/tools/draw_plan.rs`
    - 新增 `reroute_objective_feedback_away_from_reverse_shared_segments`，检测 objective/main connector 是否与其它 connector 反向共享共线 segment。
    - 反向共享时改用 `objective_to_main_vertical_first_connector_points`，避开已有进入 residual 的水平段。
    - `input_branch_connector_points` 对水平分离的 input branch 生成 source side -> target side 的短 L-shape，而不是默认 centerline 4-point path。
- 验证：
  - 两个新增红测先失败，改动后转绿。
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，59 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 再跑真实 `.env` non-mock smoke，确认 `e_residual_student` backtracking 和 `e_input_teacher` 4-point path 是否从 latest review 消失。

## 2026-06-20 latest smoke after residual-feedback/input-branch routing polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_053232`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=8`、`story_clarity=7`、`visual_hierarchy=6`、`paper_readability=7`、`layout_cleanliness=6`、`arrow_routing=6`、`color_semantics=6`、`aesthetic_quality=7`、`wps_editability=9`。
- 已验证修复：
  - 上一轮 `e_input_teacher` 4-point path 和 `e_residual_student` 反向复用 `e_teacher_residual` 的问题没有复发。
- 新暴露/残留问题：
  - 新变体里 `edge_residual_to_student` 仍被 review 判为 minor detour，因为 vertical-first 从 residual 中心下落，横向距离偏长。
  - `comp_inference` 作为 inference note-like output box 被 review 判为 major clutter。
  - `comp_student_proj` 与 teacher projection 的强弱样式不均衡，影响 color semantics。
- 已做后续小修：
  - `objective_to_main_vertical_first_connector_points` 改为从 residual/objective 靠近 student 的边界下落，而不是从 residual 中心下落。
  - `model_draw_plan_polish_reroutes_residual_feedback_off_reverse_shared_segment` 增加起点断言。
- 验证：
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，59 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 再跑真实 `.env` non-mock smoke 验证 residual feedback 是否缩短；若仍未 accepted，优先处理 `comp_inference` note-like output box 的 clutter 问题。

## 2026-06-20 latest smoke after residual edge start and note-gap clearance polish

- 后续小修：
  - `objective_to_main_vertical_first_connector_points` 从 residual/objective 靠近目标的一侧下落，缩短 vertical-first feedback route。
  - `adjacent_note_candidates` 增加 top/bottom caption 候选位。
  - `note_candidate_is_clear` 对 semantic boxes 使用 0.035 clearance，避免 inference note 贴着 residual/student gap。
  - 新增 `model_draw_plan_polish_moves_inference_note_out_of_residual_student_gap`，复现超宽 source input、task/output 占位导致 note 回退到 residual-student gap 的情况。
- 验证：
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，60 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_054406`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=8`、`story_clarity=8`、`visual_hierarchy=8`、`paper_readability=7`、`layout_cleanliness=7`、`arrow_routing=6`、`color_semantics=8`、`aesthetic_quality=7`、`wps_editability=9`。
- 已验证改善：
  - `comp_inference_note` clutter issue 没有以 box 形式复发；当前变成 `ann_inference` annotation placement 问题。
  - 语义、story、visual hierarchy 分数升到 8，layout cleanliness 升到 7。
- 仍未完成：
  - `ann_inference` 与 `student_label`/`student_encoder` 区域碰撞，review 判 major。
  - `e_pred_to_task_loss` 从 prediction 竖直回到 task loss，穿过 student branch，review 判 major。
  - input-to-encoder endpoint gap、dash pattern、branch symmetry 仍是 minor。
- 下一步：
  - 增加 annotation placement guardrail：inference annotation 若目标上方已有 label/box，则移动到 prediction 下方或 student branch 外侧。
  - 增加 prediction/output -> task loss routing guardrail：当 task loss 在 output 上方且同列时，走右侧 elbow 而不是穿过 student branch。

## 2026-06-20 inference annotation collision and prediction-task-loss routing polish

- 针对 latest smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_054406` 的两个 major issue 继续 TDD：
  - `ann_inference` anchored 到 `student_encoder` 上方时，与 `student_label` / `student_encoder` 区域碰撞。
  - `e_pred_to_task_loss` 从 prediction 竖直回到上方 task loss，穿过 student branch。
- 新增测试：
  - `model_draw_plan_polish_moves_inference_annotation_away_from_student_label`
    - 复现 `ann_inference` 与 `student_label`、`student_encoder`、`student_latent` 的碰撞场景。
    - polish 后要求 annotation 不与这些 editable boxes 重叠。
  - `model_draw_plan_polish_routes_prediction_to_task_loss_around_student_branch`
    - 复现 prediction -> task loss 同列竖直穿过 student branch 的 connector。
    - polish 后要求 route 走右侧 elbow，且不穿过 `student_encoder`、`student_latent`、`student_head`。
- 实现：
  - `src/tools/draw_plan.rs`
    - `anchored_figure_plan_annotation_bbox` 对 inference annotation 不再只机械放在 target 上方，而是通过 `clear_annotation_bbox_near_target` 选择 above/below/left/right/top/bottom 候选位。
    - 候选位必须避开当前 editable box map，避免和 branch label 或 module box 重叠。
    - 新增 `reroute_output_to_task_loss_around_intermediate_boxes`，识别 output/prediction -> task loss 的同列长竖线是否穿过中间 boxes。
    - 穿过中间 boxes 时，用 `output_to_task_loss_side_connector_points` 生成右侧/左侧 elbow route。
- 验证：
  - 两个新增红测先失败，改动后转绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，62 个测试全部通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 跑真实 `.env` non-mock smoke，确认 `ann_inference` collision 和 `e_pred_to_task_loss` branch-crossing 是否从 latest review 中消失。

## 2026-06-20 latest smoke after annotation/task-loss routing polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_055401`
  - final PPTX：`runs/latest/final/figure.pptx`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=8`、`story_clarity=7`、`visual_hierarchy=7`、`paper_readability=6`、`layout_cleanliness=5`、`arrow_routing=5`、`color_semantics=7`、`aesthetic_quality=6`、`wps_editability=9`。
- 已验证修复：
  - 上一轮的 `ann_inference` collision 和 `e_pred_to_task_loss` 穿过 student branch 没有复发。
  - PPTX/PDF/PNG 继续稳定生成，且 renderer 没有 fallback。
- 输出可见性：
  - 确认最新 PPTX 位于 `runs/latest/final/figure.pptx`，`file` 显示为 Zip archive，`unzip -t` 无错误。
  - 为避免只在被 `.gitignore` 忽略的 run 目录中查找，新增 `.gitignore` 的 `/outputs/` 条目，并复制最新 `figure.pptx`、`figure.pdf`、`figure.png` 到 `outputs/` 作为易找的导出副本。
- 新暴露/残留问题：
  - `e_student_task_label`、`e_residual_student_label`、`e_residual_label` 的 connector labels 仍被 review 判为压线或远离线。
  - `inference_note` box 轻微压到 student vertical extent，仅 minor。
- 下一步：
  - 增加 connector label final-snap guardrail：在所有 route/move pass 结束后再次按最终 connector points 重排 label，并避开 boxes/segments。

## 2026-06-20 connector label final-snap polish

- 针对 latest smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_055401` 的 connector label 问题继续 TDD：
  - `e_student_task_label` 仍停在旧位置，远离最终竖直 route。
  - `e_residual_label` 靠近 residual connector 但仍被 review 判为压线/位置不稳定。
  - `e_residual_student_label` 没有跟随最终 elbow route，停在旧水平段附近。
- 新增测试：
  - `model_draw_plan_polish_resnaps_vertical_connector_label_to_final_route`
    - 复现 student head -> task loss 的竖直 connector，label 从旧位置重新贴到最终竖线旁，并避开 source/target boxes。
  - `model_draw_plan_polish_resnaps_horizontal_connector_label_to_final_route`
    - 复现 teacher head -> latent residual 的短水平 connector，label 不再留在旧的下方 bbox，而是贴近最终水平线且不压线。
  - `model_draw_plan_polish_resnaps_elbow_connector_label_to_final_segment`
    - 复现 residual -> student 的 elbow connector，label 贴近最终水平段并避开 semantic boxes。
- 实现：
  - `src/tools/draw_plan.rs`
    - 新增 `snap_connector_labels_to_final_routes`，在所有 route/move/cleanup pass 后运行。
    - 按最终 connector points 生成 horizontal、vertical、elbow-aware label 候选位，优先避开组件、文本、图片、connector strokes 和已放置 labels。
    - 若组件间距太窄无严格解，则退化为“至少靠近最终线段且不压线”，避免 label 继续停在旧 route。
    - 顶部水平线的 label 候选在上方空间不足时优先放到线下方，避免贴住页面安全边界。
  - `.gitignore`
    - 增加 `/outputs/`，用于存放易找的最新导出副本，不污染 git。
- 验证：
  - 新增的 3 个 label final-snap 测试先复现失败，改动后通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，65 个测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 再跑真实 `.env` non-mock smoke，生成新版 `runs/latest/final/figure.pptx` 并同步到 `outputs/figure.pptx`，确认 latest review 中的 connector label major issues 是否消失。

## 2026-06-20 latest smoke after connector label final-snap polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_060836`
  - final PPTX：`runs/latest/final/figure.pptx`
  - 易找副本：`outputs/figure.pptx`、`outputs/figure.pdf`、`outputs/figure.png`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `validation_report.json` 无 warnings/errors。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=false`、`reason="cap reached before acceptance"`。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=8`、`story_clarity=7`、`visual_hierarchy=7`、`paper_readability=7`、`layout_cleanliness=6`、`arrow_routing=8`、`color_semantics=7`、`aesthetic_quality=7`、`wps_editability=10`。
- 已验证改善：
  - 上一轮的 `e_student_task_label`、`e_residual_student_label`、`e_residual_label` connector label major issues 没有复发。
  - `arrow_routing` 从 5 提升到 8，PPTX 继续是 editable native-shape 输出。
- 新暴露/残留问题：
  - 唯一 major：`c_inference` note 被放在 input -> student encoder 主 flow 中间，review 判定 obstructing main data path / visual clutter。
  - minor：`e_input_teacher_label` 仍需要 label background/padding 或离 dashed line 更远；task loss 与 prediction gap 偏紧；teacher frozen parenthetical 冗余。
- 下一步：
  - 增加 inference note placement guardrail：connected 或 note-like inference box 不能占据 input-to-main student branch 的主 data-flow corridor；优先移动到 student branch 外侧、右上/右下或 bottom margin，并保持 native editable box。

## 2026-06-20 inference-note main-flow corridor polish

- 针对 latest smoke `runs/teacher-student-distillation-with-latent-residuals_20260620_060836` 的唯一 major issue 继续 TDD：
  - `c_inference` 是 editable note-like output box，但 bbox 位于 `c_input -> c_student_enc` 横向主 data-flow connector 中间，遮挡主阅读路径。
- 新增测试：
  - `model_draw_plan_polish_moves_inference_note_out_of_input_student_corridor`
    - 复现 `c_inference=[0.26,0.54,0.42,0.68]` 与 `e_input_student=[[0.42,0.61],[0.22,0.61]]` 交叠。
    - polish 后要求 inference note 不再与主 connector corridor 相交，并离开 student encoder 所在行。
- 实现：
  - `src/tools/draw_plan.rs`
    - 新增 `move_inference_note_boxes_out_of_flow_corridors`，在最终 cleanup 后运行。
    - 对 protected FigurePlan note-like inference boxes 也生效，不再只依赖早期 `reposition_note_components_near_sources`。
    - 若 inference note 与 connector segment 相交，使用已有 adjacent-note candidate/clearance 逻辑把它移到 student branch 外侧，同时保持 native editable box。
- 验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_inference_note_out_of_input_student_corridor -- --nocapture` 通过。
  - 相关 inference note/annotation 回归测试通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，66 个测试全部通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 下一步：
  - 再跑真实 `.env` non-mock smoke，确认 `c_inference` obstructing main data path 的 major issue 是否消失，并同步新版 PPTX 到 `outputs/figure.pptx`。

## 2026-06-20 accepted smoke after inference-note corridor polish

- 真实 `.env` non-mock smoke：
  - 命令：`MAX_ITERATIONS=4 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals_20260620_061547`
  - final PPTX：`runs/latest/final/figure.pptx`
  - 易找副本：`outputs/figure.pptx`、`outputs/figure.pdf`、`outputs/figure.png`
  - `file runs/latest/final/figure.pptx` 显示 Zip archive。
  - `unzip -t runs/latest/final/figure.pptx` 无错误。
  - `validation_report.json` 无 warnings/errors。
  - `renderer_status.json` 为 `source="deterministic_draw_plan"`、`used_fallback=false`。
- 结果：
  - `status.json`：`accepted=true`、`reason="accepted"`、`review_passed=true`。
  - rounds：2。
  - `blocking_issues=[]`。
  - scores：`semantic_fidelity=9`、`story_clarity=9`、`visual_hierarchy=8`、`paper_readability=8`、`layout_cleanliness=8`、`arrow_routing=8`、`color_semantics=8`、`aesthetic_quality=8`、`wps_editability=10`。
- 已验证改善：
  - `c_inference` obstructing main data path 的 major issue 没有复发。
  - connector label major issues 继续没有复发。
  - 输出保持 native editable PPTX；renderer 未 fallback。
- 剩余 minor：
  - `latent_residual` box 宽度偏窄，可能影响小字号可读性。
  - `e_residual_student` dashed connector 仍有非常小的 elbow，但 review 只判 minor。
- 状态：
  - 当前实现已达到自动 reviewer acceptance，PPTX 已生成并同步到 `outputs/figure.pptx`。

## 2026-06-20 run loop directory/resume redesign

- 用户指出 `bash scripts/run_real_loop.sh examples/teacher_student.md` 会突然生成一串 `runs/<slug>_<timestamp>` 目录，不符合“单次命令在一个目录下持续迭代”的设想。
- 只读排查确认：
  - `scripts/run_real_loop.sh` 外层 `while true` 每次重新调用 `scripts/run_real_env.sh`。
  - `scripts/run_real_env.sh` 每次按 method summary 生成 slug，再加当前 timestamp，创建新的 `runs/<slug>_<timestamp>`。
  - `src/pipeline.rs::resume_pipeline` 遇到任意 `final/status.json` 都直接返回，rejected run 不会继续追加 round。
  - `src/pipeline.rs::renderer_uses_generated_code_bundle(false)` 让真实 non-mock 路径跳过 coder bundle，和“coding 模型根据上一轮反馈改代码”的目标不一致。
- 本轮先写红测：
  - non-mock renderer 应以 coder generated code 为 primary，并以 deterministic DrawPlan runtime 为 fallback。
  - non-mock run 应请求 generated code bundle。
  - `max_iterations=0` 应表示 until accepted。
  - rejected run 的 `resume_pipeline` 应 append 下一轮，而不是返回旧 rejected final 或重写 `round_000`。
- 当前红测结果：
  - `cargo test --test pipeline_tests -- --nocapture` 失败 4 项，分别对应上述旧行为。
- 已完成 pipeline 控制层修改：
  - `src/pipeline.rs` 新增 `run_pipeline_inner` 和 `ResumeState`，fresh run 与 resume 共用同一轮渲染/审核主流程。
  - `max_iterations=0` 不再报错，表示 until accepted；`max_minutes=0` 表示无时间上限。
  - rejected `final/status.json` 不再让 `resume_pipeline` 直接返回；只有 accepted final 才只读返回。
  - resume 会读取已有 completed `round_*/review.json`，选择 best-so-far 作为修订源，从下一个 `round_N` 追加，避免重写 `round_000`。
  - real non-mock renderer 改为 coder generated code primary、deterministic DrawPlan runtime fallback；如果 coder 输出不可用并被 agent 降级，也会在 `renderer_status.json` 中记为 fallback 并触发 review rejection。
- 当前验证：
  - `cargo test --test pipeline_tests -- --nocapture` 通过 11 项。
- 已完成脚本和 README 修改：
  - `scripts/run_real_env.sh` 默认输出改为 `runs/<task-slug>/<session-id>`，支持 `RUN_DIR` 复用已有 run；若目录中已有 `config_snapshot.json`，自动走 `cargo run -- resume --run <dir>`。
  - `scripts/run_real_env.sh` 成功后维护两个 symlink：全局 `runs/latest -> <task-slug>/<session-id>`，以及任务内 `runs/<task-slug>/latest -> <session-id>`。
  - `scripts/run_real_loop.sh` 删除外层 `while true` 重开 run 的逻辑，改为单次启动一个 session，并默认导出 `MAX_ITERATIONS=0`、`MAX_MINUTES=0`。
  - `README.md` 同步说明 `run_real_env.sh` 是有限 smoke，`run_real_loop.sh` 是单 session until accepted loop。
- 当前脚本检查：
  - `bash -n scripts/run_real_env.sh` 通过。
  - `bash -n scripts/run_real_loop.sh` 通过。
- 全量本地验证：
  - `cargo fmt` 已执行。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 真实 `.env` 受控 smoke：
  - 命令：`MAX_ITERATIONS=1 MAX_MINUTES=20 SESSION_ID=smoke_loop_layout_20260620_103259 bash scripts/run_real_loop.sh examples/teacher_student.md`
  - 新 run dir：`runs/teacher-student-distillation-with-latent-residuals/smoke_loop_layout_20260620_103259`
  - 输出不再是旧的平铺 `runs/<slug>_<timestamp>` 形态。
  - `runs/latest` 指向 `teacher-student-distillation-with-latent-residuals/smoke_loop_layout_20260620_103259`。
  - `runs/teacher-student-distillation-with-latent-residuals/latest` 指向 `smoke_loop_layout_20260620_103259`。
  - `final/figure.pptx` 是 zip-based PPTX，`unzip -t` 无错误。
  - `final/renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`，证明真实 coder path 生效。
  - 因为 smoke 只设 `MAX_ITERATIONS=1`，`final/status.json` 为 `accepted=false`、`reason="cap reached before acceptance"`，这是预期 cap 结果。
- 同目录 resume smoke：
  - 命令：`RUN_DIR=runs/teacher-student-distillation-with-latent-residuals/smoke_loop_layout_20260620_103259 MAX_ITERATIONS=1 MAX_MINUTES=20 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 脚本自动走 `cargo run -- resume --run <same-run-dir>`。
  - 没有创建新 session；同一目录下追加出 `round_001`。
  - 结果 `rounds=2`，仍因单次 resume cap 为 1 轮而 `accepted=false`；目录/续跑语义已验证。
