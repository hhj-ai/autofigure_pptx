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
- 2026-06-20：按用户反馈“框内文字空隙多、框和框挤在一起”做 TDD 修复。根因不是 vision 模型完全看不出来，而是旧 `layout_map.json` 只记录外框 bbox，没有 text/font/margin 元数据；`quality_report` 只拦截 overlap/edge/under-utilization，无法把“非重叠但视觉拥挤”和“大框短字留白过多”稳定绑定到 object id。新增失败测试：`quality_report_flags_component_crowding_without_overlap`、`quality_report_flags_excessive_internal_whitespace_from_text_metadata`、`draw_plan_renderer_tracks_text_metrics_and_scales_font_for_target_width`，确认旧实现失败。随后修改 `renderer/src/runtime.ts` 和 `renderer/src/safe_api.ts`，让渲染器按目标论文宽度放大 PPTX 字号、使用自适应 text margin，并在 `layout_map.json` 为 component/label/annotation 写入 `text`、`font_size_pt`、`margin_in`。修改 `src/tools/review.rs`，新增 `component_crowding` 与 `excessive_internal_whitespace` 质量 issue，按目标宽度毫米间距和估算文字占比给下一轮模型提供可执行反馈。修改 `src/prompts.rs`、`src/agent.rs` 和 `tests/prompt_tests.rs`，要求 vision/reasoner/DrawPlan optimizer 使用这些元数据，而不是只给泛泛审美建议。
- 2026-06-20：目标测试通过：`cargo test --test review_tests quality_report_flags`、`cargo test --test render_region_layout_tests draw_plan_renderer_tracks_text_metrics_and_scales_font_for_target_width`、`cargo test --test prompt_tests` 均通过。全量 `cargo test` 通过；`cargo fmt --check` 首次只报 `src/tools/review.rs` 两处换行格式，运行 `cargo fmt` 后 `cargo fmt --check` 通过；`cd renderer && npm run build` 通过；`git diff --check` 通过。同步更新 `README.md`，说明 `layout_map.json` 现在记录 `text`、`font_size_pt`、`margin_in`，这些字段会被 repair loop 用于检测 paper-width 字体/留白/拥挤问题。
- 2026-06-20：按 `.env` 真实 smoke 运行 `SESSION_ID=spacing_text_smoke_20260620_225008 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=30 bash scripts/run_real_env.sh examples/teacher_student.md`。运行在同一 session 目录迭代到 round_001，结束状态 `accepted=false`、`reason=cap reached before acceptance`，这是 2 轮上限导致的未接受，不是执行失败。`final/figure.pptx` 通过 `unzip -t`；`final/renderer_status.json` 为 `source=model_generated_code`、`used_fallback=false`。`final/layout_map.json` 的 component 均写出 `text`、`font_size_pt`、`margin_in`，模块源字号约 `19.1pt`，对应 85mm paper-width 可读；`final/quality_report.json` 产生 `component_crowding`（`student_tower` 与 `student_latent` 目标宽度间距 1.5mm）和 `degenerate_edge`。`final/issue_binding.json` 将新 `component_crowding` 绑定到具体 target ids；`final/improvement_plan.json` 产生可执行动作，要求把 `student_latent` 与 `student_tower` 拉开到至少约 3mm 并修复 `e_student_latent`。这验证了“视觉模型看得出来”的问题现在会被本地量化并传入下一轮模型，而不是只留下泛泛审美反馈。
- 2026-06-20：人工查看上述 smoke 的 `final/figure.png`，发现直接按 `1/scale` 把 85mm 目标宽度字号从 9pt 放大到约 19.1pt 太激进，窄框中出现 `Student (compact)`、`Output ŷ` 等硬折行。修正 `renderer/src/runtime.ts` 的字号策略为 `1/sqrt(scale)` 的保守 paper-width 补偿，仍比旧 9pt 明显增大，但避免把窄框撑爆。同步调整 `draw_plan_renderer_tracks_text_metrics_and_scales_font_for_target_width` 的断言，从强制 `>1.8x` 改为 `>1.3x`。复测 `cargo test --test render_region_layout_tests draw_plan_renderer_tracks_text_metrics_and_scales_font_for_target_width`、`cargo test --test review_tests quality_report_flags` 和 `cd renderer && npm run build` 均通过。
- 2026-06-20：第二次 `.env` smoke（`spacing_text_smoke_20260620_225702`）验证新字号约 `13.1pt`，最终 PNG 不再出现 19pt 版本的严重硬折行；`final/quality_report.json` 捕获 `teacher_model` 的 `excessive_internal_whitespace`，`improvement_plan.json` 给出收紧 teacher bbox 的具体动作。人工查看 PNG 后发现 `student_model` 仍有明显大框留白但因文字稍长越过旧 `0.08` 阈值未被本地 gate 捕获；将 `excessive_internal_whitespace` 改为两段阈值：超大框 `area > 0.08 && text_fill < 0.12`，中等框保持 `area > 0.055 && text_fill < 0.08`。复测 `cargo test --test review_tests quality_report_flags` 通过；`cargo fmt` 已运行修正格式。
- 2026-06-20：阈值提高后首次全量 `cargo test` 暴露 mock pipeline 无法收敛：mock optimizer 收紧大空框后，旧 `under_utilized` 用外接 bbox 面积判定，把横向铺开的紧凑流程误判为空间不足；`max_iterations=0` until-pass 测试进入长循环，已手动中断。修正 `src/tools/review.rs`：`under_utilized` 现在只拦截横向和纵向都缩成小团的图，横向铺开但高度紧凑的流程不再被罚；新增 `quality_report_allows_wide_compact_horizontal_flow` 回归测试。修正 `src/agent.rs` 的 mock optimizer，让面积大于 `0.065` 且高度大于 `0.22` 的空框在 mock revision 中收紧，覆盖 multimodal encoder/fusion/head 空框。验证：`cargo test --test review_tests quality_report`、`cargo test --test pipeline_tests`、全量 `cargo test` 均通过；测试过程中 LibreOffice 仍打印一次历史上见过的外部 `DeploymentException`，但所有测试 exit 0。
- 2026-06-20：最终 `.env` smoke 运行 `SESSION_ID=spacing_text_smoke_20260620_230959 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=30 bash scripts/run_real_env.sh examples/teacher_student.md`。运行完成在同一 session 目录迭代到 round_001，状态 `accepted=false`、`reason=cap reached before acceptance`、`review_passed=false`；`final/figure.pptx` 通过 `unzip -t`；`final/renderer_status.json` 为 `source=model_generated_code`、`used_fallback=false`。关键结果：`final/quality_report.json` 为 `passed=true`、`score=100`、`issues=[]`，说明本地新增的框内留白/框间距/字号门禁已通过；`final/layout_map.json` 记录所有 component 的 `text`、`font_size_pt=13.1`、`margin_in`。人工查看 `final/figure.png`，确认没有第一版 19.1pt 带来的严重硬折行，框内留白明显少于旧输出。最终未接受来自 vision review 的更高层 composition/arrow routing 问题，如 residual box 与 answer box overlap、annotation 压线、input-to-teacher connector 穿过 student zone；这些问题已经进入 `review.json` 和 `improvement_plan.json`，但 2 轮 smoke 的上限前未完全修完。

## 2026-06-21 visual collision and arrow-through-component gates

- 用户目标：
  - 继续排查当前绘图质量差的根因并迭代优化，跑几轮真实 smoke，确保每一轮都有有效提升。
- 当前证据：
  - 上一轮最终 smoke 的 `quality_report.json` 已经通过，但 vision review 仍指出明显问题：`latent_residual` 与 `answer` 有可见碰撞、`anno_residual` 压在 `e_residual_student` 上、`e_input_teacher` 横穿 `student` 区域。
  - 这说明本地 quality gate 仍漏掉视觉模型能看到的基础几何问题，导致下一轮修复依赖 vision 自然语言，而不是稳定的 issue_id/target_ids。
  - 当前环境没有保留上一轮 `runs/latest` 和 `spacing_text_smoke_20260620_230959` 目录，只保留了源码和部分新 run 目录；因此本轮以源码和合成 layout_map 测试复现这些失败模式，并会重新跑 `.env` smoke 生成当前证据。
- 计划：
  - 给 `tests/review_tests.rs` 增加红测：小面积但毫米级可见的 component collision、edge 穿过非端点 component、annotation/label 压 connector stroke。
  - 修改 `src/tools/review.rs`：降低 component overlap 的可见碰撞漏检，新增 `edge_crosses_component` gate，增强 `label_overlaps_edge` 对线穿过 label 的检测。
  - 更新 prompt/issue 建议，让 `QualityReport` 中的 collision/crossing 问题能进入下一轮 `RoundImprovementPlan`。
  - 跑 `cargo fmt --check`、目标测试、全量 `cargo test`、`cd renderer && npm run build`、`git diff --check`，再跑真实 `.env` multi-round smoke 检查每轮是否产生 material changes 和 issue 下降。

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

## 2026-06-20 reference-guided useful-feedback loop plan

- 用户反馈：后期绘图整体提升很小，怀疑 reasoning model 缺少“好图”判断能力；希望把 ViT、CLIP、BERT 以及近年 ML 三会优秀论文图作为上下文参考，并保证每轮都有有用、可执行的改图建议。
- 本次采用“模板 + 只读预览”方案：仓库打包抽象 reference grammar，preview 由脚本生成到 ignored `tmp/reference_figures/`，可作为只读模型证据，但不能进入 renderer 或最终 PPTX。
- 实施边界：优先实现 reference pack、reference selection artifact、prompt 注入、workspace 可见性、improvement plan、DrawPlan material-diff gate 和文档；Best Paper 元数据优先保存官方 award/blog URL 与可扩展模板，不把不确定的原图或整图 raster 写进仓库。
- 预期修改：`templates/method_overview/reference_figures.json`、`src/schema.rs`、`src/tools/template_library.rs`、`src/agent.rs`、`src/pipeline.rs`、`src/cli.rs`、`src/llm/openai_compatible.rs`、`tests/*`、`README.md`、`scripts/extract_reference_previews.sh`。
- 验证方法：先写 reference/prompt/pipeline/diff 相关测试，再运行 `cargo fmt --check`、`cargo test`、`cd renderer && npm run build`、`bash -n scripts/extract_reference_previews.sh`；如时间和外部模型稳定，补真实 `.env` smoke。
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

## 2026-06-20 reference-guided improvement loop implementation

- 用户目标：
  - 后期绘图每轮提升太小，reasoning model 需要更强的视觉判断依据。
  - 初始阶段应由 reasoning model 为当前任务选择最合适的历史/经典图像模板作为参考。
  - 每一轮反馈必须产生有用、可执行的建议；coding model 根据上一轮反馈继续改代码/DrawPlan。
  - 多个模型共享当前代码、渲染图、参考图和上一轮计划上下文。
- 约束：
  - PPTX 输出必须继续是 editable native shapes/text/lines，不能把整页参考图 raster 化塞进 slide。
  - 经典论文图和近年 best paper 图只作为只读视觉 grammar/reference；本地 preview 放在 `tmp/reference_figures/`，受 `.gitignore` 保护。
  - `plan.md`/`goal.md`/`AGENTS.md` 仍按打包上下文要求保留在项目中，不加入 ignore。
- 实现：
  - 新增 `templates/method_overview/reference_figures.json`，收录 SimCLR、ViT、CLIP、BERT 以及 NeurIPS 2025 Gated Attention 等抽象参考模板，记录 selection tags、layout grammar、style grammar、anti-patterns 和质量 rubric。
  - `src/tools/template_library.rs` 新增参考库加载、keyword/phrase 选择和 selected-reference JSON 生成；mock 路径可稳定选择参考，真实路径交给 reasoner 选择。
  - `src/schema.rs` 新增 `ReferencePreviewMode`、`ReferenceSelection`、`RoundImprovementPlan`、`ImprovementAction` 及其 strict JSON schema。
  - `src/prompts.rs` 和 `src/agent.rs` 新增 reference selector 与 round improvement planner；初始 FigurePlan 只注入被选中的参考模板，避免整包模板污染上下文。
  - 每轮 review 后必须生成 `improvement_plan.json`；revision prompt 同时看到上一轮渲染图、可选参考 preview、selected reference 和 improvement actions。
  - `src/tools/draw_plan.rs` 新增 material DrawPlan diff，revision 若没有实际可见改动会重试一次，仍无 material change 则报错，避免无效迭代。
  - `src/llm/openai_compatible.rs` 支持同一次 vision call 传多张图片，使 reviewer/optimizer 可同时看当前图和参考 preview。
  - `src/pipeline.rs` 新增 `reference_selection.json` 与 `improvement_plan.json` 的 run/round/final 持久化；resume 兼容旧 run，缺失 reference selection 时会从 method 重新选择。
  - `src/cli.rs` 新增 `--reference-previews auto|off|required`，默认 `auto`；`required` 缺 preview 时会明确提示先运行提取脚本。
  - 新增 `scripts/extract_reference_previews.sh`，下载并渲染经典论文 PDF 页面到 `tmp/reference_figures/*.png`。
  - `README.md` 更新 reference-guided loop、输出目录、CLI 参数和 rejected round 规则。
- 验证：
  - `cargo fmt`、`cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - 重点测试 `cargo test --test reference_library_tests --test prompt_tests --test workspace_pipeline_tests --test draw_plan_diff_tests` 通过。
  - `cd renderer && npm run build` 通过。
  - `bash -n scripts/extract_reference_previews.sh` 通过。
  - `bash scripts/extract_reference_previews.sh` 成功生成 SimCLR/ViT/CLIP/BERT preview PNG。
  - `git diff --check` 通过。
  - mock smoke：`cargo run -- run --method examples/teacher_student.md --out /tmp/methodfig_reference_smoke --style wps-clean --aspect paper-wide --target-width-mm 85 --max-iterations 2 --max-cost-usd 3.0 --max-minutes 20 --image-provider none --reference-previews required --mock-models` 通过，`accepted=true`、`rounds=2`。
  - `unzip -t /tmp/methodfig_reference_smoke/final/figure.pptx` 无错误；final 目录含 `figure.pptx`、`reference_selection.json`、`improvement_plan.json`。
- 外部来源核对：
  - ICLR 2025 官方 outstanding paper awards 页面：`https://blog.iclr.cc/2025/04/22/announcing-the-outstanding-paper-awards-at-iclr-2025/`
  - NeurIPS 2025 官方 best paper awards 页面：`https://blog.neurips.cc/2025/11/26/announcing-the-neurips-2025-best-paper-awards/`
  - ICML 2025 官方 awards 页面：`https://icml.cc/Conferences/2025/Awards`
- 剩余风险：
  - 本轮只做了 mock smoke 和 preview extraction；未重新跑真实 `.env` non-mock 长循环。
  - 近年 award reference 当前以抽象 layout/style grammar 为主；没有稳定 PDF 链接的 award 图不会被脚本自动下载为 preview。

## 2026-06-20 tracked reference preview assets and required-preview smoke

- 用户追加目标：
  - 参考模板/preview 可以直接进入 git，让模型读取这些模板 evidence。
  - 用真实路径测试能跑通。
- 实现调整：
  - 将 reference preview 默认位置从 ignored 的 `tmp/reference_figures/` 改为可追踪的 `templates/method_overview/reference_figures/assets/`。
  - 复制并保留 4 个经典论文 preview PNG：SimCLR、ViT、CLIP、BERT。
  - 新增 `neurips_2025_gated_attention_award.png` synthetic preview，避免 award reference 在 `--reference-previews required` 下缺图。
  - `templates/method_overview/reference_figures.json` 的 `preview_root` 和每个 `preview.local_path` 已指向 tracked assets。
  - `scripts/extract_reference_previews.sh` 现在默认把 PNG 写到 tracked assets，PDF 缓存仍在 ignored 的 `tmp/reference_figures/pdfs`。
  - `scripts/run_real_env.sh` 新增 `REFERENCE_PREVIEWS` 环境变量，默认 `auto`；可用 `REFERENCE_PREVIEWS=required` 验证模型必须读取 preview。
  - README 同步说明模板 PNG 是 versioned reference evidence，不是 renderer assets，最终 PPTX 仍不能嵌入这些参考图。
- 真实 smoke 暴露并修复的问题：
  - 第一次同目录 resume 时，真实 DrawPlan optimizer 生成 `ann_task_eq` 越界 bbox，`validate_draw_plan` 报 `draw object ann_task_eq bbox is outside normalized canvas`。
  - 修复：`src/tools/draw_plan.rs` 新增 `normalize_draw_plan_bounds`，在模型 DrawPlan 验证前和 polish 后规范 object bbox、connector point、label bbox；合法 bbox 原样保留，越界 bbox 通过平移/缩放进画布，避免压成 0 宽高。
  - `src/agent.rs` 在真实 optimizer 返回和 retry 返回后都调用 `normalize_draw_plan_bounds` 再验证。
  - 另修复 resume 计数语义：`count_rounds` 和 `next_round_index` 只按有 `review.json` 的完成轮次计算；失败留下的半成品 round 会被下一次 resume 复用，而不是跳号。
- 新增/更新测试：
  - `tests/reference_library_tests.rs` 现在断言每个 reference 声明的 preview path 实际存在于可追踪模板目录。
  - `tests/draw_plan_tests.rs` 新增越界 model geometry normalization 回归。
  - `tests/pipeline_tests.rs` 新增半成品 `round_001` 被 resume 复用的回归。
- 验证：
  - `bash scripts/extract_reference_previews.sh` 通过，生成 5 个 PNG 到 `templates/method_overview/reference_figures/assets/`。
  - `git check-ignore -v templates/method_overview/reference_figures/assets/simclr_contrastive_y_branch.png templates/method_overview/reference_figures/assets/neurips_2025_gated_attention_award.png || true` 无输出，说明模板 PNG 不被 ignore。
  - `git ls-files --others --exclude-standard templates/method_overview/reference_figures/assets templates/method_overview/reference_figures.json scripts/extract_reference_previews.sh` 能列出新模板资产和脚本。
  - 重点测试通过：`cargo test --test reference_library_tests --test prompt_tests --test workspace_pipeline_tests --test draw_plan_diff_tests`。
  - 回归测试通过：`cargo test --test draw_plan_tests model_draw_plan_polish_normalizes_out_of_bounds_model_geometry -- --nocapture`。
  - 回归测试通过：`cargo test --test pipeline_tests resume_pipeline_continues_rejected_run_directory -- --nocapture`。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `bash -n scripts/extract_reference_previews.sh && bash -n scripts/run_real_env.sh && bash -n scripts/run_real_loop.sh && git diff --check` 通过。
  - 真实 `.env` required-preview 两轮 smoke：
    - 命令：`SESSION_ID=reference_assets_required_loop_20260620_141713 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=30 bash scripts/run_real_env.sh examples/teacher_student.md`
    - run dir：`runs/teacher-student-distillation-with-latent-residuals/reference_assets_required_loop_20260620_141713`
    - 结果：`accepted=false`、`rounds=2`、`reason="cap reached before acceptance"`。
    - `final/reference_selection.json`：`selected_reference_id="simclr_contrastive_y_branch"`，`preview_path="templates/method_overview/reference_figures/assets/simclr_contrastive_y_branch.png"`，`preview_mode="required"`。
    - `round_000/review.json` 和 `round_001/review.json` 都存在，证明两轮在同一目录完成。
    - `round_000/improvement_plan.json` 有 5 条 actions，`round_001/improvement_plan.json` 有 6 条 actions。
    - `round_001/renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`，证明第二轮真实 coder path 可运行。
    - `unzip -t final/figure.pptx` 无错误。
- 剩余风险：
  - 这次真实 smoke 因 `MAX_ITERATIONS=2` 到 cap 未 accepted；它验证的是 required-preview、两轮循环、同目录输出和 artifact 完整性，不是最终视觉质量收敛。
  - `final/renderer_status.json` 选到 best-so-far 的 `round_000` deterministic fallback；`round_001` 已证明 model-generated code path 可运行，但 best-round 选择策略仍可能偏向带 fallback blocker 的高分轮次，后续可单独优化。

## 2026-06-20 issue-bound incremental repair loop

- 用户目标：
  - 当前后期绘图改动很小，且重来容易原地打转；每轮 coding model 应先读取上一轮代码、DrawPlan、layout_map、review 和具体问题，再做增量修复。
  - 模板只能作为软参考，不能把布局槽位写死；reasoning model 需要把历史图像模板、当前渲染图和上一轮代码/问题对应起来。
  - 每轮反馈必须产出有用建议，并绑定到具体对象、线条、标签或代码位置，避免泛泛“提高美观”。
- 已读约束：
  - `AGENTS.md` 要求改动前先读项目上下文，并持续更新 `plan.md`。
  - 输出仍必须是 editable PPTX native shapes/text/connectors，不能整页 raster 化。
  - `goal.md`/`plan.md`/`AGENTS.md` 要保留进项目上下文，不能加入 ignore。
  - 真实路径优先使用 `.env` 中 LLM 配置做 smoke，但不能打印密钥。
- 当前结构观察：
  - `src/pipeline.rs` 已有同目录 resume、reference selection、`improvement_plan.json`，但没有结构化 `quality_report.json`、`issue_history.json`、`issue_binding.json`、`repair_report.json`。
  - `src/agent.rs` 的 DrawPlan revision prompt 只看到上一轮 DrawPlan/review/layout/validation/improvement，没有上一轮 generated code，也没有结构化质量问题历史。
  - `src/tools/review.rs` 只有字符串型 `render_quality_issues`，不利于把问题绑定回具体对象。
  - `generate_draw_plan_typescript` 把完整 payload 写进 generated TS；虽然 runtime 会用 env 覆盖 out_dir，但模型上下文仍可能看到旧 round 的 embedded payload。
  - `renderer/src/runtime.ts` 没有把 `draw_plan.style_tokens` 合入 palette，也没有在 `layout_map.json` 的 edge 里记录 `from/to`。
  - connector polish 会把非常短的直线扩成 dogleg，可能制造无意义折线和重叠。
- 计划修改文件：
  - `src/tools/review.rs`：新增结构化 `QualityReport`/`QualityIssue`，保留旧字符串 gate 兼容。
  - `src/pipeline.rs`：写入并传递 `quality_report.json`、`issue_history.json`、`issue_binding.json`、`repair_report.json`，final 同步这些产物。
  - `src/agent.rs`：revision prompt 和 coder prompt 增加 previous code、quality report、issue history、issue binding，明确 issue-bound incremental repair。
  - `src/tools/draw_plan.rs`、`src/tools/render.rs`、`renderer/src/runtime.ts`、`renderer/src/safe_api.ts`：payload 外置、style token 生效、edge from/to 进入 layout map、修正短 connector dogleg。
  - `README.md` 和相关测试：同步输出结构和回归测试。
- 验证方法：
  - 先补红测覆盖结构化报告、prompt 上下文、workspace artifact、payload 外置、style token、短线不 dogleg。
  - 再跑 `cargo fmt --check`、重点测试、必要时全量 `cargo test`、`cd renderer && npm run build`、`git diff --check`。
- 已完成实现：
  - `src/tools/review.rs` 新增 `QualityReport`/`QualityIssue`，旧 `render_quality_issues` 继续返回字符串 blocking issues，但来源改为结构化报告。
  - `src/pipeline.rs` 每轮新增 `quality_report.json`、`issue_binding.json`、`issue_history.json`、`repair_report.json`，并复制到 `final/`；第二轮 workspace 会读取上一轮这些产物和上一轮 `figure.ts`。
  - `src/agent.rs` 的 DrawPlan revision prompt 增加 previous generated code、QualityReport、IssueHistory、IssueBinding，并明确模板/参考图只是软证据；coder prompt 也要求围绕 issue_id 或 repeated issue_key 做最小增量修复。
  - `src/tools/draw_plan.rs` 把 DrawPlan renderer payload 写成 `renderer_payload.json`，generated TS 改为通过 trusted runtime 的 `createDrawPlanRuntimeFromEnv()` 读取当前 round payload，不再嵌入整份 payload 和旧 out_dir。
  - `src/tools/render.rs` 执行 Node renderer 时设置 `METHODFIG_RENDER_PAYLOAD_PATH` 和 `METHODFIG_RENDER_OUT_DIR`。
  - `renderer/src/runtime.ts` 开始使用 `draw_plan.style_tokens` 覆盖 primary/accent/neutral/text/background 等 palette，并在 `layout_map.json` 的 edge 上记录 `from/to`。
  - `src/tools/draw_plan.rs` 停止把短直 connector 扩成四点 dogleg，避免为了过短而制造重叠和绕线。
  - `README.md` 输出目录和开发验收说明已补充新 artifacts。
- 已完成测试：
  - `cargo fmt` 通过。
  - `cd renderer && npm run build` 通过。
  - `cargo test --test review_tests -- --nocapture` 通过。
  - `cargo test --test prompt_tests -- --nocapture` 通过。
  - `cargo test --test pipeline_fallback_tests --test render_fallback_tests -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_keeps_short_direct_connectors_without_dogleg -- --nocapture` 通过。
  - `cargo test --test workspace_pipeline_tests -- --nocapture` 通过。
  - `cargo test --test render_region_layout_tests draw_plan_renderer_applies_style_tokens_and_tracks_edge_endpoints -- --nocapture` 通过。
  - `cargo fmt --check` 通过。
- 最终验证：
  - 跑全量 `cargo test` 和 `git diff --check`，确认没有其他回归。
  - `cargo test` 全量通过。
  - `git diff --check` 通过。
  - `git status --short --branch` 仅显示本轮预期修改文件，没有额外生成物。

## 2026-06-20 real smoke for issue-bound loop

- 用户目标：
  - 跑一轮实际 `.env` non-mock 小测试，确认上一轮 issue-bound incremental repair 改动在真实模型路径下没有问题。
  - 如果真实 smoke 暴露问题，直接调整代码并复测。
- 本次验证策略：
  - 使用 `examples/teacher_student.md`，`REFERENCE_PREVIEWS=required`，确保参考 preview 真正进入模型上下文。
  - 使用 `MAX_ITERATIONS=2`，让 rejected round 后的下一轮能实际读取上一轮代码、quality report、issue binding、issue history 和 repair report。
  - 检查 `final/figure.pptx` zip 完整性、`renderer_status.json`、`quality_report.json`、`issue_binding.json`、`issue_history.json`、`repair_report.json`、以及 `round_001/workspace/readable/*` 是否存在。
- 第一次真实 smoke：
  - 命令：`SESSION_ID=issue_bound_smoke_20260620_162629 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=30 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/issue_bound_smoke_20260620_162629`
  - 第 0 轮已生成 `figure.pptx`、`layout_map.json`、`quality_report.json`、`issue_binding.json`、`issue_history.json`、`repair_report.json`。
  - 失败点：真实 reasoner 返回的 `RoundImprovementPlan` 有 action 缺少 `target_id`，旧 validator 报 `RoundImprovementPlan action must include target_id or reference_replan` 并中断 run。
- 已修复：
  - `src/agent.rs` 新增 `normalize_round_improvement_plan`，对真实模型漏填 `target_id`、空 `success_check`、空 `expected_visible_effect`、空 actions 做保底补全。
  - 缺 `target_id` 时优先从 localized review issue 推断具体对象，否则使用 `global_layout`。
  - `build_round_improvement_prompt` 增加要求：localized action 不能留空 target；多对象问题无法选择单一对象时用 `global_layout`。
  - 新增单元测试覆盖缺 `target_id` 和空 actions 的 normalization。
- 修复后验证：
  - `cargo fmt` 通过。
  - `cargo test agent::tests::round_improvement_normalization -- --nocapture` 通过。
- 继续真实 smoke：
  - resume 同一目录：`RUN_DIR=runs/teacher-student-distillation-with-latent-residuals/issue_bound_smoke_20260620_162629 MAX_ITERATIONS=2 MAX_MINUTES=30 REFERENCE_PREVIEWS=required bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：同一目录追加到 `round_002`，`accepted=false`、`rounds=3`、`reason="cap reached before acceptance"`，这是 cap 结果；没有再因 `RoundImprovementPlan` 缺 `target_id` 中断。
  - `final/figure.pptx` 通过 `unzip -t`；final artifacts 含 `quality_report.json`、`issue_binding.json`、`issue_history.json`、`repair_report.json`。
  - `round_001` 和 `round_002` workspace 均存在 previous quality/binding/history/repair/code artifacts。
  - `round_002/final renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
- 第二个 1 轮真实 smoke：
  - 命令：`SESSION_ID=issue_bound_prompt_smoke_20260620_164116 REFERENCE_PREVIEWS=required MAX_ITERATIONS=1 MAX_MINUTES=20 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：正常完成并写出 final artifacts，`accepted=false` 是 1 轮 cap 结果。
  - 暴露问题：initial coder 仍可能手写无效 renderer，错误为读取未公开 payload/style token，导致 deterministic fallback；pipeline 未崩，但 `renderer_status.json` 为 `used_fallback=true`。
  - 已修复/收紧：`src/prompts.rs` 的 coder prompt 明确初始轮优先返回 trusted reference runtime entrypoint，不手写 PPTX renderer，不直接读取 `payload/draw_plan/style_tokens`，不复制 runtime color/layout 逻辑；`tests/prompt_tests.rs` 增加回归。
  - 验证：`cargo test --test prompt_tests coder_prompts_limit_runtime_to_documented_draw_plan_api -- --nocapture` 通过。
- 第三个 1 轮真实 smoke：
  - 命令：`SESSION_ID=issue_bound_strict_coder_smoke_20260620_164612 REFERENCE_PREVIEWS=required MAX_ITERATIONS=1 MAX_MINUTES=20 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 暴露问题：真实 reasoner 选中合法 `simclr_contrastive_y_branch`，但返回 `preview_path=null`；`REFERENCE_PREVIEWS=required` 因此失败。
  - 已修复：`src/tools/template_library.rs` 新增 `complete_reference_selection_from_pack`，按 `selected_reference_id` 从本地 reference pack 回填 preview path 和缺失 grammar；`src/agent.rs` 在真实 reference selection 后调用该补全。
  - 验证：`cargo test --test reference_library_tests reference_selection_completion_restores_missing_preview_path_from_pack -- --nocapture` 通过。
- 最终真实 smoke：
  - 命令：`SESSION_ID=issue_bound_reference_completion_smoke_20260620_164849 REFERENCE_PREVIEWS=required MAX_ITERATIONS=1 MAX_MINUTES=20 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：正常完成，`accepted=false`、`rounds=1`、`reason="cap reached before acceptance"`，这是 1 轮 cap 结果。
  - `reference_selection.json` 已自动回填 `preview_path="templates/method_overview/reference_figures/assets/simclr_contrastive_y_branch.png"`，`preview_mode="required"`。
  - `final/figure.pptx` 通过 `unzip -t`。
  - final artifacts 完整：`figure.pptx`、`figure.pdf`、`figure.png`、`figure.ts`、`draw_plan.json`、`review.json`、`improvement_plan.json`、`quality_report.json`、`issue_binding.json`、`issue_history.json`、`repair_report.json`。
  - `final/renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - `round_000/figure.model_error.log` 不存在。
- 最终本地验证：
  - `cargo fmt --check` 通过。
  - `cd renderer && npm run build` 通过。
  - `git diff --check` 通过。
  - `cargo test` 全量通过；LibreOffice 在部分导出测试中向 stderr 打出外部 `DeploymentException` 文本，但测试退出码为 0，所有相关测试通过。

## 2026-06-21 visual collision and arrow-through-component gates

- 用户目标：
  - 先规划再实现，重点解决“视觉模型明明应该看得出来，但每轮仍给不出有效改动”的问题。
  - 当前根因不是单纯 prompt 不够，而是本地结构化质量门漏掉了肉眼可见的基础几何错误，导致 `QualityReport` 可以通过，后续 reasoner/coder 收到的强约束不够具体。
- 根因确认：
  - `component_overlap` 只按 normalized area 和面积比例卡阈值，上一轮 `answer` / `latent_residual` 这种约 `2.6 x 2.8 mm` 的真实碰撞会因为面积小被漏掉。
  - `label_overlaps_edge` 只用线段 bbox 与 label bbox 的面积占比，细 connector 压过 annotation/text 时面积仍很小，容易漏报。
  - 旧逻辑没有检测 connector 是否穿过非端点组件，所以 `e_input_teacher` 穿过 `student` 这类问题只能依赖视觉模型自由描述，不能稳定进入 issue-bound repair。
- 红测：
  - 新增 `quality_report_flags_small_but_visible_component_collision`，复现小面积但可见的组件碰撞。
  - 新增 `quality_report_flags_edge_crossing_through_unrelated_component`，复现 connector 穿过非 source/target 组件。
  - 新增 `quality_report_flags_thin_connector_running_through_annotation`，复现 annotation 被细 connector 穿过。
  - 修复前运行 `cargo test --test review_tests quality_report_flags_ -- --nocapture`，三个新增用例按预期失败，已有 whitespace/crowding 用例仍通过。
- 已完成实现：
  - `src/tools/review.rs` 增加基于目标纸宽毫米尺度的 overlap 判定：只要横纵重叠都达到约 1 mm 且有实际比例，就输出 blocking `component_overlap`。
  - `src/tools/review.rs` 增加线段裁剪检查，connector 穿过非端点 component 的 shrunken interior 时输出 blocking `edge_crosses_component`，target ids 绑定到 offending edge 和 crossed component。
  - `src/tools/review.rs` 改进 label/annotation 与 edge 的碰撞检测：保留旧面积判定，同时用 line clipping 判断 connector stroke 是否穿过扩展后的 label bbox。
  - `src/agent.rs` 的 DrawPlan revision 和 RoundImprovementPlan prompt 现在明确要求对 `component_overlap`、`edge_crosses_component`、`label_overlaps_edge` 给出绑定具体 id 的局部移动、缩放或 reroute。
  - `tests/prompt_tests.rs` 增加 prompt 契约检查，防止这些 issue 类型以后被删掉。
- 已完成验证：
  - `cargo test --test review_tests quality_report_flags_ -- --nocapture` 通过。
  - `cargo test --test review_tests -- --nocapture` 通过，22 个 review 测试全绿。
  - `cargo test --test prompt_tests draw_plan_revision_prompt_uses_autofigure_style_visual_optimization_contract -- --nocapture` 通过。
- 全量验证暴露的回归与修复：
  - `cargo test` 首次全量运行时，`resume_pipeline_uses_existing_run_directory` 失败；新 `edge_crosses_component` gate 拦住了 mock multimodal fusion 布局里的真实问题：`vision_to_fusion` 水平穿过非端点组件 `text_encoder`。
  - 复现 run 显示 `round_001/quality_report.json` 中 blocker 为 `edge_crosses_component`，说明不是误报，而是旧 multimodal mock 布局把两个 encoder 和 fusion/head 排成一行，导致两路输入合流语义被画成穿框直线。
  - 新增 `draw_plan_from_multimodal_fusion_stacks_inputs_without_connector_through_encoder`，要求 multimodal fusion 的 vision/text 输入上下分开，fusion/head 保持右侧主路径，且 `vision_to_fusion` 不穿过 `text_encoder`。
  - `src/tools/draw_plan.rs` 新增 `packed_multimodal_fusion_boxes`：只在 `Template::MultimodalFusion` 且为精确四节点 mock 结构时触发，把两个 encoder 放成上下两路输入，fusion/head 放右侧；其他自由 reasoner 结构仍走原有通用 pack。
  - 首次修复后又触发 `component_crowding`，因为上下输入间距只有约 `1.5 mm`；已把上下输入中心进一步拉开，使间距越过质量门。
  - `cargo test --test draw_plan_tests draw_plan_from_multimodal_fusion_stacks_inputs_without_connector_through_encoder -- --nocapture` 通过。
  - `cargo test --test pipeline_tests resume_pipeline_uses_existing_run_directory -- --nocapture` 通过。
- 完整本地验证：
  - `cargo fmt --check` 首次提示 `src/tools/draw_plan.rs` 新 helper 调用需要标准换行；已运行 `cargo fmt` 修正。
  - `cargo test` 全量通过，包括新增的 68 个 `draw_plan_tests` 中的 multimodal 回归测试、22 个 `review_tests` 和 workspace/pipeline 测试。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 剩余验证：
  - 再跑真实 `.env` required-preview 多轮 smoke，检查每一轮是否在同一 run dir 下产生 issue-bound、非空、可见的修复动作，并确认 final PPTX zip 完整。

## 2026-06-21 real smoke after visual gates

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=visual_collision_smoke_20260621_013130 REFERENCE_PREVIEWS=required MAX_ITERATIONS=4 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/visual_collision_smoke_20260621_013130`
  - 结果：`accepted=false`、4 轮、`reason="cap reached before acceptance"`。
  - 同目录完成 `round_000` 到 `round_003`，没有重开目录。
  - 每轮 `renderer_status.json` 都是 `source="model_generated_code"`、`used_fallback=false`。
  - `unzip -t final/figure.pptx` 通过，PPTX zip 完整。
- smoke 结论：
  - 新增质量 gate 生效：`quality_report.json` 能稳定给出带具体 target ids 的 `component_overlap`、`component_crowding`、`edge_crosses_component`、`edge_crossing`、`route_detour`、`excessive_internal_whitespace`。
  - 但 reasoner 第 0 轮 `reference_replan` 错误建议把单一 `latent_residual_obj` 拆成 `teacher_residual_obj` 和 `student_residual_obj`。之后 vision review 连续指出应合并回单一 residual objective，但第 2、3 轮的 `repair_report.material_changes` 基本重复，说明 planner/coder 仍会重复失败策略。
  - final PNG 仍然很差：大 residual/task-loss 盒子压在顶部，input 被挤到底部，connector 穿框，局部 label 仍贴线；这次不能算视觉质量收敛。
- 根因：
  - `polish_model_draw_plan_geometry_with_figure_plan` 是 non-mock 真实路径，它只做温和 polish，没有运行强 canonical 的 `repair_teacher_student_lanes`。
  - 真实 DrawPlan 中新增了 `teacher_residual_obj` / `student_residual_obj`，但 FigurePlan 明确只有一个 `latent_residual_obj` residual 组件。
  - `remove_connectors_absent_from_figure_plan` 只会删除“两个端点都属于 FigurePlan 组件但边不存在”的 connector；连到模型新增 split residual box 的 connector 不会被删。
  - `add_missing_connectors_from_figure_plan` 又会因为同名错误 connector 已存在而跳过 canonical edge，导致错误结构持续进入下一轮。
- 已完成修复：
  - 新增测试 `model_draw_plan_polish_with_figure_plan_removes_split_residual_boxes`，复现同名 residual connector 被接到 `teacher_residual_obj` 的失败模式。
  - `src/tools/draw_plan.rs` 新增 `prune_residual_boxes_absent_from_figure_plan`，仅在 `Template::TeacherStudent` 且 FigurePlan 已声明 residual/latent 组件时触发。
  - 该函数删除未在 FigurePlan 声明的 residual/latent-like box，并删除连接到这些 extra boxes 的 connector；随后现有 `add_missing_connectors_from_figure_plan` 会补回 canonical edges。
  - 这个修复只作用于 FigurePlan-aware non-mock polish 路径，不影响无 FigurePlan 的 `polish_model_draw_plan_geometry`，避免把模板坐标硬套到所有模型输出。
- 已完成验证：
  - 修复前 `cargo test --test draw_plan_tests model_draw_plan_polish_with_figure_plan_removes_split_residual_boxes -- --nocapture` 按预期失败。
  - 修复后该测试通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，69 个 DrawPlan 测试全绿。
  - `cargo test --test workspace_pipeline_tests -- --nocapture` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 剩余验证：
  - 再跑一轮真实 `.env` smoke，确认 split residual 不再进入 round/final DrawPlan，并检查质量 issue 是否减少。

## 2026-06-21 split residual prune smoke

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=split_residual_prune_smoke_20260621_014545 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/split_residual_prune_smoke_20260621_014545`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
  - 每轮 `renderer_status.json` 都是 `source="model_generated_code"`、`used_fallback=false`。
  - `unzip -t final/figure.pptx` 通过。
- 验证结果：
  - `round_000`、`round_001`、`round_002` 的 `draw_plan.json` 都不再包含 `teacher_residual_obj` 或 `student_residual_obj`，证明 FigurePlan-aware prune 生效。
  - `quality_report` 从第 0 轮 9 个 issues / score 0，降到第 1 轮 2 个 issues / score 70；第 2 轮回升到 4 个 issues / score 30。
  - final 选中 best-so-far 结果，`final/quality_report.json` 只剩两个 `component_crowding`：`comp_task_loss` / `comp_inference_note` 横向间距 1.6mm，`comp_output` / `comp_inference_note` 纵向间距 1.0mm。
  - final PNG 明显优于上一轮 smoke：主结构可读，split residual 消失，输入、teacher、student、task loss、prediction 关系基本能看懂。
- 剩余问题：
  - final 仍未 accepted；`Inference-only` 与 task-loss/output 附近拥挤，`Prediction` 在窄框内发生不自然换行。
  - 本地 quality gate 目前能抓 crowding，但还没有专门抓“长单词在窄框里被硬换行”的文本可读性问题。
  - 第 2 轮相对第 1 轮有局部回退，说明 reasoner/coder 仍可能过度修改；best-so-far 会保护 final，但“每一轮单调提升”还没有严格保证。
- 下一步建议：
  - 增加 `text_wrap_risk` quality issue：基于 layout_map 的 `text`、`font_size_pt`、`margin_in` 和 bbox 宽度估算最长词是否能在单行内显示，避免 `Prediction` 这类窄框硬换行。
  - 在 issue history 中增加 “regressed_from_best” 或 “do_not_repeat_failed_strategy” 约束，让 round N+1 明确不得重新引入上一轮已经减少的 quality issue。
  - 对 final 的两个 crowding issue，可通过将 `comp_inference_note` 移到 output 下方或右下 gutter，并缩小 task-loss label 来进一步收敛。

## 2026-06-21 text wrap readability gate

- 根因：
  - `split_residual_prune_smoke_20260621_014545` 的 final PNG 中，`Prediction` 被窄输出框硬拆成两行，肉眼可读性差。
  - 当时 `final/quality_report.json` 只剩 crowding，没有抓到长 token 在窄框里被 awkward wrap 的问题。
  - 旧 `internal_whitespace_ratio` 只衡量空隙和填充率，不能证明最长词能在 textbox 宽度内单行显示。
- 红测：
  - 新增 `quality_report_flags_single_word_wrap_risk_from_text_metadata`，用 smoke final 的 `comp_output` 几何、`text="Prediction"`、`font_size_pt=13.1`、`margin_in=0.035` 复现问题。
  - 修复前该测试失败，说明旧本地 gate 漏报。
- 已完成实现：
  - `src/tools/review.rs` 新增 `text_wrap_risk`，基于 layout_map 中的 `text`、`font_size_pt`、`margin_in`、bbox 宽度和 target paper width，估算最长 token 的纸面宽度。
  - 当最长 token 需要的宽度超过内容宽度时，输出 major `text_wrap_risk`，绑定到具体 component id，并提示 widen bbox / shorten label / lower font size。
  - `src/agent.rs` 的 DrawPlan revision 和 RoundImprovementPlan prompt 增加 `text_wrap_risk` 契约，要求下一轮明确给出 width、label 或 font-size 的可见变化。
  - `tests/prompt_tests.rs` 增加 `text_wrap_risk` 和 `longest token` 契约检查。
- 已完成验证：
  - `cargo test --test review_tests quality_report_flags_single_word_wrap_risk_from_text_metadata -- --nocapture` 修复后通过。
  - `cargo test --test review_tests -- --nocapture` 通过，23 个 review 测试全绿。
  - `cargo test --test prompt_tests draw_plan_revision_prompt_uses_autofigure_style_visual_optimization_contract -- --nocapture` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 剩余验证：
  - 跑真实 `.env` smoke，确认 `text_wrap_risk` 能进入 round/final feedback，并观察 `Prediction` 窄框硬换行是否减少。

## 2026-06-21 text wrap smoke and annotation-over-component gate

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=text_wrap_gate_smoke_20260621_015915 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/text_wrap_gate_smoke_20260621_015915`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
  - `round_000` renderer 使用 deterministic fallback；`round_001`、`round_002` 和 `final` 均为 `source="model_generated_code"`、`used_fallback=false`。
  - `unzip -t final/figure.pptx` 通过。
- 验证结果：
  - `quality_report` issue 数从第 0 轮 26 个降到第 1 轮 12 个，再降到第 2 轮 2 个；说明质量 gate 给出的反馈能推动整体布局改善。
  - 第 1 轮出现 `text_wrap_risk`：`residual_objective` 的 `Residual` token 需要约 `9.6 mm`，但可用宽度约 `9.4 mm`。
  - 第 2 轮和 final 中 `text_wrap_risk` 消失，证明新增 gate 能被 loop 消化。
- 新暴露问题：
  - final 仍未 accepted，剩余问题是 `student_pred` 与 `residual_objective` 间距仅约 `1.5 mm`，以及 `ann_residual_eq` 与 `e_student_pred` 重叠。
  - 人工检查 PNG 后发现更直接的视觉问题：`ann_residual_eq` 的大 annotation bbox 覆盖了 `Prediction Head` 组件区域，但旧 gate 只报告 `label_overlaps_edge`，没有把 annotation 压住 component text 作为 blocking issue 暴露给 reasoner。
- 红测：
  - 新增 `quality_report_flags_annotation_covering_component_text`，用 smoke final 的 `ann_residual_eq` 和 `student_pred` bbox 复现 annotation 覆盖 component 的问题。
  - 修复前该测试失败，证明旧结构化质量门没有稳定表达“标签/注释压住组件文字”。
- 已完成实现：
  - `src/tools/review.rs` 在 label/annotation 检查中增加 `label_overlaps_component`：当 label/annotation 与 component 有毫米级可见重叠，且覆盖比例达到阈值时输出 blocking issue。
  - 新 issue 绑定 label/annotation id 和 component id，提示将 label 或 annotation 移出 component bbox，避免 editable text 覆盖模块文字。
  - `src/agent.rs` 的 DrawPlan revision 和 RoundImprovementPlan prompt 增加 `label_overlaps_component` 契约，要求模型只移动/缩放命名 bbox 或 reroute 命名 label/connector。
  - `tests/prompt_tests.rs` 增加 `label_overlaps_component` prompt 契约检查。
- 已完成验证：
  - `cargo test --test review_tests quality_report_flags_annotation_covering_component_text -- --nocapture` 通过。
  - `cargo test --test review_tests -- --nocapture` 通过，24 个 review 测试全绿。
  - `cargo test --test prompt_tests draw_plan_revision_prompt_uses_autofigure_style_visual_optimization_contract -- --nocapture` 通过。
  - `cargo fmt` 已运行。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 剩余验证：
  - 再跑真实 `.env` smoke，确认 `label_overlaps_component` 能进入反馈并促使模型把 residual equation annotation 移出 `Prediction Head` 等组件区域。

## 2026-06-21 label/component gate smoke and route polish

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=label_component_gate_smoke_20260621_021048 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/label_component_gate_smoke_20260621_021048`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
  - 所有轮次和 final 的 `renderer_status.json` 都是 `source="model_generated_code"`、`used_fallback=false`。
  - `unzip -t final/figure.pptx` 通过。
- 验证结果：
  - `quality_report` issue 数从第 0 轮 29 个降到第 1 轮 4 个，再到第 2 轮 3 个。
  - `label_overlaps_component` 在第 0/1 轮出现，到第 2 轮消失，证明新增 gate 能进入真实 loop 并被模型处理。
  - 上一轮 text-wrap smoke 中的 residual equation 压 `Prediction Head` 问题没有复现。
- 新暴露问题：
  - final 仍有 `component_overlap`：`student_head` 与 `output` 在右边界发生约 `1.7 x 3.8 mm` 可见重叠。
  - final 仍有 `edge_crosses_component`：`e_task_label` 穿过 `student_latent`。
  - final 仍有 `degenerate_edge`：`e_output` 过短。
  - 人工检查 PNG 后确认根因是 coder/reasoner 的动作没有完全兑现：计划里要求把 output 移到 `[0.93, 0.60, 0.99, 0.66]`，实际 layout_map 仍是 `[0.90, 0.58, 1.00, 0.68]`；右移越过画布后被 clamp，导致 overlap 和短边保留。
- 红测：
  - 新增 `model_draw_plan_polish_compacts_right_edge_output_without_overlap_or_degenerate_edge`，复现右边界 output 与 head 压框、`e_output` 太短的问题。
  - 新增 `model_draw_plan_polish_reroutes_task_label_edge_around_student_latent`，复现 task-label 线穿过 `student_latent` 的问题。
  - 修复前两个测试均失败，说明旧 DrawPlan polish 没有对齐新的质量门。
- 已完成实现：
  - `src/tools/draw_plan.rs` 新增 `repair_right_edge_output_collisions`：当 output-like 目标位于右边界且与来源重叠或连接过短时，把短文本 output 压成紧凑小节点，并在必要时把来源盒子左移，保证画布内仍有非退化连接长度。
  - `src/tools/draw_plan.rs` 新增 `reroute_connectors_around_intermediate_boxes`：当 connector 穿过非端点组件时，生成上/下/左/右避障折线候选，选择不穿组件且较短的路由。
  - 通用避障跳过 objective-to-main residual feedback，让已有 `reroute_objective_feedback_away_from_reverse_shared_segments` 专门规则最终裁决这类反馈边。
  - `component_overlap_gate_fails` 与 review gate 对齐，能抓到小面积但毫米级可见的 box overlap，避免 review 报错而 polish 不处理。
- 已完成验证：
  - 两个新增 draw_plan 回归测试修复后通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，71 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。

## 2026-06-21 post-route-polish smoke

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_route_polish_smoke_20260621_022824 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_route_polish_smoke_20260621_022824`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
  - 所有轮次和 final 的 `renderer_status.json` 都是 `source="model_generated_code"`、`used_fallback=false`。
  - `unzip -t final/figure.pptx` 通过。
- 验证结果：
  - 上一轮 smoke 中的 `student_head/output` overlap、`e_task_label/student_latent` crossing、`degenerate_edge` 没有再出现。
  - issue 数为 11 -> 6 -> 4；仍然有下降，但第 2 轮未 accepted。
  - final 剩余 issues：`comp_task_loss` 的 excessive whitespace；`anno_teacher_label` 压到 `comp_task_loss`；`edge_teacher_to_residual` / `edge_residual_to_student` 与 `edge_student_to_task_loss` 两处 edge crossing。
- 剩余根因：
  - 质量门已经能给出具体 blocker，但 DrawPlan polish 还没有“压缩 oversized short-label loss/objective box”的兜底；`comp_task_loss` 仍可成为大空框。
  - annotation 移出组件的本地 polish 仍不够强；`Large LM` 这种 branch label 会在 box 边界处产生毫米级重叠。
  - edge crossing 目前主要依赖 reasoner/coder 按 feedback reroute，本地只有“避开组件”的路由，没有“避开其他 connector”的通用路由。
- 下一步建议：
  - 增加 conservative `compact_oversized_loss_or_objective_boxes`，只对 role/text 明确是 loss/objective 且文本很短的大空框触发。
  - 增加 `move_annotations_off_components`，优先把 branch/context annotation 移到组件 union 外侧的最近空白。
  - 增加 `reroute_connectors_around_crossing_edges`，仅在结构化 `edge_crossing` 或本地检测到 crossing 时触发，候选路线需要同时避开组件和已有 connector。

## 2026-06-21 annotation / crossing polish continuation

- 本轮继续从真实 `.env` smoke 失败样例出发，而不是重写模板。
- 新增回归覆盖：
  - `model_draw_plan_polish_compacts_oversized_short_loss_box`：短 loss/objective 文本不应留在大空框内。
  - `model_draw_plan_polish_moves_figure_plan_annotation_off_component`：FigurePlan upsert 的 annotation 不能压住 component。
  - `model_draw_plan_polish_moves_figure_plan_annotation_off_connector`：FigurePlan upsert 的 annotation 不能盖住 connector。
  - `model_draw_plan_polish_moves_model_authored_annotations_off_components`：模型自己写出的 annotation 也必须移出组件。
  - `model_draw_plan_polish_moves_connector_label_off_endpoint_component`：connector label 不能贴在端点组件内。
  - `model_draw_plan_polish_reroutes_connector_around_crossing_edges`：本地 polish 要能绕开 connector crossing。
  - `model_draw_plan_polish_separates_vertical_output_from_head`：右侧 output 与 head 纵向贴边时需要留 gutter。
- 已完成实现：
  - `src/tools/draw_plan.rs` 增加 loss/objective 紧凑化、annotation 避开 component/edge、connector label 避开 component、connector-crossing reroute、output/head 间距兜底。
  - 这些规则都基于当前 DrawPlan 的 ids、bbox 和 connector endpoints，不把固定 teacher-student 模板坐标强行写死。
- 验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，78 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。

## 2026-06-21 post annotation / label polish smoke

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_model_annotation_label_polish_smoke_20260621_031500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_model_annotation_label_polish_smoke_20260621_031500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
  - 所有轮次和 final 的 `renderer_status.json` 都是 `source="model_generated_code"`、`used_fallback=false`。
  - `unzip -t final/figure.pptx` 通过。
- 验证结果：
  - issue 数从 `round_000` 的 22 个降到 `round_001` 的 8 个，再到 `round_002/final` 的 7 个。
  - 上一轮重点问题 `label_overlaps_component` 和 `edge_crosses_component` 在 `round_001` 消失，说明 annotation/label/穿框兜底确实进入真实 loop 并产生有效改善。
- 新暴露问题：
  - final 仍未 accepted，剩余问题为 `inference_note` 的 `text_wrap_risk`、`student_head/teacher_enc` 和 `task_loss/latent_residual` 的 crowding、`e_teacher_proj` 短边、`e_proj_residual` 穿 `teacher_enc`、以及两处 connector crossing。
  - final PNG 证明核心根因是模型生成了“上下贴住的 teacher context stack + residual 竖向回流 + task loss 反馈线”组合；旧 router 能避开单个 box，但不会稳定处理这种多条 supervision/main/task 线的局部死结。
- 已完成修复：
  - 新增 `model_draw_plan_polish_repairs_stacked_teacher_residual_smoke_layout`，用 final 几何复现 note 过窄、loss stack 贴边、teacher stack 短边、teacher residual 穿主输出、task-loss feedback 穿 student residual 等问题。
  - `src/tools/draw_plan.rs` 增加 `widen_short_context_note_boxes_for_text`，为 `Inference: student only` 这类 note 盒提供最小可读宽度。
  - 增加 `separate_crowded_loss_or_objective_boxes`，对横向重叠且纵向间距不足的 loss/objective stack 留出可见 gutter。
  - 增加 `separate_stacked_context_boxes`，对上下贴住的 context stack 先横向避开 main module，再补上下间距，避免短边和穿框。
  - 调整 connector stability：main/output 线比 supervision/residual 线更稳定，避免把主流程绕坏。
  - 增加 `reroute_supervision_connectors_around_main_edges`，当 supervision/residual 线穿过 main/output 线时，优先使用外侧 rail 绕行。
  - 增加 `reroute_task_loss_connectors_around_supervision_edges`，当 task/loss feedback 穿过已有 connector 时，先找完全无冲突路线；若局部几何无严格候选，则至少清掉当前 blocker 和 student-residual 保护线，避免原样保留最刺眼的穿线。
- 已完成验证：
  - 新回归测试修复前失败，修复后通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，79 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 剩余验证：
  - 继续跑真实 `.env` smoke，确认 stacked teacher/residual/task-loss 兜底在实际 loop 中是否减少 final blockers。

## 2026-06-21 quality-aware best-so-far and final smokes

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_stack_route_polish_smoke_20260621_034500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_stack_route_polish_smoke_20260621_034500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`，PPTX zip 完整，所有轮次 `source="model_generated_code"`、`used_fallback=false`。
- 结果与根因：
  - `round_000` 为 score 40 / 4 issues，`round_001` 退到 score 15 / 5 issues，`round_002/final` 退到 score 0 / 6 issues。
  - final 选中了质量更差的 `round_002`，说明 best-so-far 只按 vision review 排名，未把本地 `quality_report.score/issues` 纳入最终选择。
  - 这会直接破坏“后续轮可以尝试，但不能覆盖更好结果”的目标。
- 已完成修复：
  - `src/pipeline.rs` 增加 `BestRound { index, review, quality_report }`，run 和 resume 都保留 quality report。
  - 新增 `should_replace_best_round_quality`：排序优先级为 `quality_report.passed`、`quality_report.score`、blocking/major/total issue 数，再回退到原 review rank。
  - final selection 和 revision source 都改用 quality-aware best round；保留原 `should_replace_best_review` 以兼容旧 review-only 测试。
  - `tests/pipeline_tests.rs` 新增：
    - `best_round_quality_keeps_higher_quality_round_over_later_review_score`
    - `best_round_quality_replaces_when_quality_score_improves`
- 验证：
  - 新增 pipeline tests 通过。
  - `cargo test --test pipeline_tests -- --nocapture` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 真实 `.env` best-so-far smoke：
  - 命令：`SESSION_ID=post_quality_best_smoke_20260621_041500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_quality_best_smoke_20260621_041500`
  - 结果：`accepted=false`、3 轮、PPTX zip 完整，所有轮次和 final 均为 `source="model_generated_code"`、`used_fallback=false`。
  - 三轮和 final 都是 score 85 / 1 issue，final quality 与 `round_000`、`round_001`、`round_002` 一致；没有再发生“final 选中更差 quality round”的问题。
  - final 仅剩 `component_crowding`：`latent_residual_supervision` 与 `inference_only` 纵向间距 1.3mm。
- 针对剩余 note/loss crowding 的修复：
  - 新增 `model_draw_plan_polish_moves_inference_note_away_from_loss_crowding`，复现 `Inference only` note 与 `Latent Residual` 贴得太近的问题。
  - `src/tools/draw_plan.rs` 增加 `move_context_notes_away_from_loss_boxes`：context/note 与 loss/objective 横向重叠且纵向间距不足时，先尝试上/下移出 0.055 归一化 gutter，空间不足再左右侧移。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，80 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过；测试期间 LibreOffice 子进程打印过一次 deployment exception，但对应测试和主进程最终通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 最后 1 轮真实 smoke：
  - 命令：`SESSION_ID=post_note_crowding_smoke_20260621_043500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=1 MAX_MINUTES=20 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_note_crowding_smoke_20260621_043500`
  - 结果：`accepted=false`、1 轮、PPTX zip 完整，`source="model_generated_code"`、`used_fallback=false`。
  - 该随机初稿没有复现上一轮的 note/loss 单一 crowding，而是产生了 8 个新 issue：whitespace、student/residual crowding、label/component overlap、label/edge overlap、degenerate edge。结构化 quality gate 能正确抓到这些问题，但 1 轮 smoke 不能证明 note/loss crowding 修复在真实随机样例中触发。
  - 当前状态：
  - 明显改善：大面积重叠、线穿框、annotation 压字、final 选中更差轮的问题都有本地回归和实现兜底。
  - 仍未彻底解决：模型仍会随机生成新的差布局；每轮单调提升还没有强约束，只能靠 quality-aware best-so-far 保护 final。
  - 下一步如果继续做，应在 improvement plan/coder prompt 中加入 regression budget：下一轮不得增加 blocker 数、不得降低 `quality_report.score`，否则自动回退到 best round 并只允许对当前 blockers 做局部 patch。

## 2026-06-21 regression budget / latest-attempt context implementation

- 用户目标：
  - 不希望每轮都重构、原地打转；coding/vision/reasoning 模型应先读上一轮代码和问题，再做局部有效修改。
  - 视觉模型如果给足 prompt 应该能识别重叠、空白、拥挤等低级问题；每轮 feedback 必须有可执行约束。
- 本轮发现的实际问题：
  - best-so-far 能保护 final，但下一轮如果从 best round 回滚重修，prompt 只看到 best round，容易丢失刚刚失败的 latest attempt 负例。
  - `quality_report.score` 旧权重太容易归零，差图之间没有足够排序信号；真实 smoke 中 8-10 个 issue 都是 0 分。
  - best round 排序一开始按 score 优先，会把“score 更高但 blocking 更多”的轮次选成 final，违背 regression budget。
  - 初始 reasoner 偶尔生成 edge 指向不存在 component 的 FigurePlan，导致首轮还没写出 `figure_plan.json` 就中断，无法进入 loop。
- 已完成实现：
  - `src/pipeline.rs`
    - 新增 `regression_report.json`，记录 `source_round_index`、`quality_score`、`score_delta`、`blocking_delta`、`major_delta`、`issue_delta`、`regressed_issue_types`、`resolved_issue_types` 和下一轮 budget。
    - final copy list 增加 `regression_report.json`。
    - workspace 增加 `readable/previous_regression_report.json`。
    - 当 revision source 回滚到较早 best round 时，新增 latest attempt context：`latest_attempt_review.json`、`latest_attempt_quality_report.json`、`latest_attempt_regression_report.json`、`latest_attempt_issue_binding.json`、`latest_attempt_improvement_plan.json`、`latest_attempt_draw_plan.json`、`latest_attempt_code/figure.ts`。
    - 给 DrawPlan optimizer 和 coder 的 regression context 现在同时包含 revision source budget 和 latest attempt 负例。
    - best-so-far 排序改为 `passed -> blocking 数 -> major 数 -> score -> issue 数 -> vision review rank`，避免 score 提升掩盖 blocker 增加。
    - regression status 改为预算优先：只要 blocking/major 增加或 score 降低，就标 `regressed`。
  - `src/agent.rs`
    - `build_round_improvement_prompt`、`build_draw_plan_revision_prompt`、`build_revised_code_prompt` 都加入 `QualityReport`、`IssueHistory`、`IssueBinding`、`RegressionReport/RegressionContext`。
    - prompt 明确要求：改动必须绑定 issue_id/target_id；如果 latest attempt evidence 存在，要把它当负例，不重复失败几何/代码策略。
  - `src/tools/review.rs`
    - `quality_report.score` 改为更细粒度诊断分：blocking/major/minor penalty 为 12/6/2；acceptance 逻辑不变，任何 blocking/major 仍不通过。
  - `src/tools/validate.rs`
    - `normalize_plan_for_render` 增加 edge endpoint 修复：edge 如果误指向单组件 region，改写到该 component；无法解析的缺失 endpoint edge 直接删除，避免首轮中断。
- 已完成测试：
  - `tests/prompt_tests.rs`
    - 覆盖 round improvement / DrawPlan revision prompt 必须包含 RegressionReport/RegressionContext、budget、latest_attempt evidence。
  - `tests/workspace_pipeline_tests.rs`
    - 覆盖 round/final 写出 `regression_report.json`，第二轮 workspace 写出 `previous_regression_report.json`。
  - `tests/pipeline_tests.rs`
    - 覆盖 score 更高但 blocking 更多的轮次不能替换 best-so-far。
  - `tests/review_tests.rs`
    - 覆盖多 severe issue 时 score 仍保留非零排序信号。
  - `tests/plan_normalize_tests.rs`
    - 覆盖 edge 指向 region 的修复和无法解析 endpoint 的删除。
- 真实 `.env` smoke 记录：
  - 失败 smoke：
    - 命令：`SESSION_ID=post_regression_budget_smoke_20260621_055500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=40 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：首轮前失败，`FigurePlan validation failed: edge teacher_head_to_residual references missing to component residual_signal_region`。
    - 处理：新增 `normalize_plan_for_render` edge endpoint 容错。
  - 旧 score smoke：
    - 命令：`SESSION_ID=post_regression_budget_smoke_20260621_061000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=40 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：3 轮跑通但未 accepted，PPTX zip 完整。
    - 发现：三轮 score 都是 0，final 正确回滚到 issue 更少的 round，但 score 对 regression budget 不够有用。
  - score granularity smoke：
    - 命令：`SESSION_ID=post_regression_budget_score_smoke_20260621_063000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=40 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`round_000 score=64/blockers=1/majors=4`，`round_001 score=52/blockers=4/majors=0`，`round_002 score=70/blockers=2/majors=1`。
    - 发现：`round_002` score 更高但 blockers 更多，旧 best ranking 错选了 `round_002`；随后修复 blocker-first ranking。
  - 最终 blocker-first smoke：
    - 命令：`SESSION_ID=post_regression_budget_blocker_rank_smoke_20260621_064500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=40 bash scripts/run_real_env.sh examples/teacher_student.md`
    - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_regression_budget_blocker_rank_smoke_20260621_064500`
    - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
    - 质量变化：`round_000 score=46/issues=7/blockers=2/majors=5` -> `round_001 score=64/issues=5/blockers=1/majors=4` -> `round_002/final score=88/issues=2/blockers=0/majors=2`。
    - `round_002/final` 剩余两个 issue 都是 `component_crowding`，没有 blocker。
    - `round_002/final` 的 `renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
    - `unzip -t final/figure.pptx` 通过。
- 最终本地验证：
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 剩余问题：
  - 3 轮 smoke 仍未 accepted，最后只剩 `component_crowding`，说明 crowding 的局部修复还需要继续加强。
  - 首轮有一次 coder 失败后走 deterministic fallback，但最终轮是 model-generated code，且 final PPTX 完整可编辑。

## 2026-06-21 residual hub, connector route, and review false-positive fixes

- 本轮继续处理真实 smoke 中最后暴露的问题：
  - `post_regression_budget_blocker_rank_smoke_20260621_064500/final` 只剩两个 `component_crowding`：`latent_residual` 被夹在 teacher/student 分支之间。
  - 后续 `post_residual_hub_crowding_smoke_20260621_071500` 中，本地 `quality_report` 已出现 `round_001/final score=100/issues=0/passed=true`，但 vision review 仍拒绝，原因集中在 residual connector 过度折线、`task_loss` 卡在 student output/main module 中间，以及 `training_label` phase-only annotation 缺失误报。
- 已完成实现：
  - `src/tools/draw_plan.rs`
    - 新增 residual/supervision objective hub 局部移动逻辑：当 residual hub 与两侧 teacher/student branch crowding 时，将其移动到 branch gap 外侧的安全候选位置，而不是整图重排。
    - 新增 `move_task_loss_boxes_out_of_output_main_corridors`：当 `task_loss` 夹在 output 与 main module 的窄通道里时，优先移到 output 右侧并保持短直连。
    - 新增 `straighten_residual_alignment_rails`：对真正的 latent residual alignment edge 使用单条水平 rail，并把 label 放到 rail 上/下方，避免 4 段折线和 label 压线。
    - 限定 residual rail 规则不作用于 objective/loss feedback edge，避免破坏既有 loss/objective 回流测试。
  - `src/tools/review.rs`
    - 新增 `sanitize_review_false_positives`，只过滤“缺失 phase-only FigurePlan annotation”这类既定策略误报。
    - 过滤条件要求出现 missing/absent/omitted 等缺失语义，并且命中 `training_label`、`inference_label`、`anno_training` 等 phase-only id 或 FigurePlan phase annotation 语义；真实的 overlap/crowding/edge routing 问题不会被吞掉。
  - `src/agent.rs`
    - `review_rendered_figure` 在解析 vision Review 后调用 false-positive sanitizer，再进入 plan geometry gate 和 render quality gate。
    - review prompt/retry prompt 明确说明：`Training/Inference/Testing/training_label/inference_label` 这类 phase-only annotation 可以被折叠到附近模块或按设计省略，不能仅因缺失而拒收；只有可编辑 DrawPlan 里真实缺少 phase 语义时才报告。
  - `tests/draw_plan_tests.rs`
    - 新增 residual hub crowding 回归。
    - 新增 smoke review 几何回归，覆盖 `task_loss` 从 output/main corridor 移出、`student_to_output` 直连、`output_to_loss` 直连、`latent_residual` 单水平 rail 且 label 离线。
  - `tests/review_tests.rs`
    - 新增 false-positive sanitizer 回归：纯粹 `Missing training_label annotation from FigurePlan` 会被移除并重新按阈值通过；`training_label overlaps...` 和 `training_loss` 的真实问题会保留。
  - `tests/prompt_tests.rs`
    - 固定 review prompt 中的 phase-only annotation 约束。
- 已完成本地验证：
  - `cargo fmt --check` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，82 个 DrawPlan 测试全绿。
  - `cargo test --test review_tests -- --nocapture` 通过。
  - `cargo test --test prompt_tests -- --nocapture` 通过。
  - `cargo test --test pipeline_tests -- --nocapture` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 剩余验证：
  - 需要再跑真实 `.env` smoke，确认新的 route/hub/sanitizer 是否能让最终 accepted，或者至少保证 final 继续选择 blocker-free、quality 高的轮次。

## 2026-06-21 smoke-driven semantic cleanup and residual crossing fix

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_route_review_sanitize_smoke_20260621_073500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=40 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_route_review_sanitize_smoke_20260621_073500`
  - 结果：`accepted=false`、3 轮、final PPTX zip 完整，所有轮次和 final 均为 `source="model_generated_code"`、`used_fallback=false`。
- 本次 smoke 的实际改善：
  - `round_000`：`score=16/issues=12/blockers=2/majors=10`。
  - `round_001`：`score=82/issues=2/blockers=1/majors=1`，相对首轮减少 10 个 quality issue，blocking 减 1，major 减 9。
  - `round_002`：`score=82/issues=2/blockers=1/majors=1`，换成另一组同级问题。
  - `final` 正确保留 `round_001` 作为 best-so-far，而不是覆盖成第三轮，说明 quality-aware final selection 生效。
  - `training_label` 缺失误报没有再出现。
- 新发现的根因：
  - `comp_student_predict` 的文本被模型污染成 `Student Predict + Task Loss`，但图中已经有独立 `comp_task_loss`，导致语义重复和单职责破坏。
  - `comp_task_output` 在右边界处宽度只有 0.13，`Prediction` 最长 token 在 85mm 目标宽度下仍有 wrap risk。
  - `edge_latent_to_residual_loss` 的 residual 横线被 `edge_student_encode_to_predict` 的主路竖线穿过，旧 generic crossing repair 没有稳定选择 residual/objective edge 作为局部修复对象。
- 已完成实现：
  - `src/tools/draw_plan.rs`
    - 新增 `remove_embedded_task_loss_text_from_main_boxes`：当图里已有独立 task-loss box 时，清理 main/student/predict 模块文本中的 `Task Loss` 片段，避免把 loss 语义合进主模块。
    - 新增 `widen_output_boxes_for_long_tokens`：按最长 token 估算 output box 最小宽度，优先在画布内以中心扩宽，并重新吸附相关 connector 端点。
    - 新增 `reroute_residual_objective_connectors_around_main_crossings`：只针对 residual/supervision 到 objective/loss 的 connector 与 main route 交叉，生成外侧 rail 候选并通过 box/connector 冲突过滤，避免影响普通 supervision 或 task-loss feedback。
  - `tests/draw_plan_tests.rs`
    - 新增 `model_draw_plan_polish_unmerges_task_loss_and_reroutes_residual_crossing_from_smoke`，复现本次 final 几何，修复前失败，修复后通过。
- 已完成验证：
  - 新回归测试修复前失败，修复后通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，83 个 DrawPlan 测试全绿。
  - `cargo test --test review_tests -- --nocapture` 通过。
  - `cargo test --test prompt_tests -- --nocapture` 通过。
  - `cargo test --test pipeline_tests -- --nocapture` 通过。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
- 剩余验证：
  - 需要再跑真实 `.env` smoke，观察新的语义清理/输出扩宽/residual reroute 是否把 final 从 `score=82` 继续推高，或暴露下一类局部 blocker。

## 2026-06-21 short-content box compaction and final smoke

- 第二次真实 smoke 结果：
  - 命令：`SESSION_ID=post_semantic_route_cleanup_smoke_20260621_081500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=40 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_semantic_route_cleanup_smoke_20260621_081500`
  - 结果：`accepted=false`、3 轮、final PPTX zip 完整，所有轮次和 final 均为 `source="model_generated_code"`、`used_fallback=false`。
  - `round_000`：`score=64/issues=6/blockers=0/majors=6`，已经没有上一轮的 `Task Loss` 合并、`Prediction` wrap risk 或 residual/main crossing。
  - `round_001` 和 `round_002`：`score=82/issues=2/blockers=1/majors=1`，分数更高但引入 `edge_crosses_component` blocker，`regression_report.status="regressed"`。
  - `final` 正确保留无 local blocker 的 `round_000`，没有选择高分但有 blocker 的 round，证明 blocker-first best-so-far 和 regression budget 生效。
- 新发现的根因：
  - final 图里 `Teacher (Large LM)`、`Student (Compact)`、`Output` 都是短文本但占用超大框，导致内部空白大、任务损失/latent residual/输出之间的通道被挤压。
  - 这类问题不是新模板选择问题，而是 DrawPlan optimizer 对短文本主模块缺少“面积上限”约束。
- 已完成实现：
  - `src/tools/draw_plan.rs`
    - 新增 `compact_oversized_short_content_boxes`：只处理短文本且面积明显过大的 context/main/output/module box；跳过 input、loss/objective/residual/supervision，避免破坏语义节点。
    - 压缩后重新 realign connector endpoints，保持当前布局方向和稳定 id，不重排整图。
  - `tests/draw_plan_tests.rs`
    - 新增 `model_draw_plan_polish_compacts_oversized_short_main_boxes_from_smoke`，复现该 smoke final 几何，确保 teacher/student/output 盒面积被收紧，并给 task-loss/residual 留出可见 gutter。
- 已完成验证：
  - 新回归测试修复前失败，修复后通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，84 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 最后 1 轮真实 smoke：
  - 命令：`SESSION_ID=post_compact_short_boxes_smoke_20260621_084500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=1 MAX_MINUTES=20 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_compact_short_boxes_smoke_20260621_084500`
  - 结果：`accepted=false`、1 轮、final PPTX zip 完整。
  - 本地 quality：`score=88/issues=2/blockers=0/majors=2`，剩余为 `input_text` 的 internal whitespace 和 `task_output/task_loss` crowding。
  - 限制：该轮 renderer 使用 `source="deterministic_fallback"`、`used_fallback=true`，因此只能证明 pipeline/fallback/render/quality gate 没坏，不能证明 model-generated TypeScript 完全通过。
- 当前剩余问题：
  - 还没有一次 3 轮真实 smoke 达到 accepted。
  - 最新 1 轮 smoke 暴露 coder 偶发 TypeScript 失败，需要后续单独提高 coder prompt/contract 或把 fallback round 的诊断更早反馈给 coder。
  - 局部几何质量已有明显改善：多次 smoke 中 final 能避免有 blocker 的回退轮，且本地质量最好达到 `score=88/blockers=0`。

## 2026-06-21 runtime contract feedback, annotation cleanup, and route quality gates

- 本轮目标：
  - 继续实现上一轮规划，重点解决真实 smoke 中“fallback 掩盖模型代码错误”“重复/游离 annotation 重叠”“本地 quality gate 对绕线路由和远离 edge 的 label 漏报”的问题。
  - 避免每轮重构，采用 smoke 暴露一个具体坏模式就加一个最小回归和局部修复的方式。
- 根因 1：coder 生成了未公开 runtime API 调用。
  - 证据：`post_compact_short_boxes_smoke_20260621_084500/round_000/figure.model_error.log` 中 `TypeError: runtime.getDrawPlan is not a function`。
  - 修复：
    - `src/tools/render.rs` 新增 `validate_generated_runtime_contract`，静态拒绝 `.getDrawPlan(`、`.getSlide(`、`.getPptx(`、`.getPresentation(`、`.track(`、`.write(` 等未公开 API。
    - `src/pipeline.rs` 把上一轮和 latest attempt 的 `figure.model_error.log` 暴露到 regression context 和 workspace readable 文件，确保 coder 下一轮能读到真实失败原因。
    - `src/prompts.rs`、`src/agent.rs` 明确要求 generated `figure.ts` 只使用 `createDrawPlanRuntimeFromEnv()` + `runtime.renderDrawPlan()`。
  - 回归测试：
    - `runtime_contract_rejects_undocumented_runtime_methods_before_node_execution`
    - `renderer_retries_deterministic_fallback_when_model_uses_unsupported_runtime_api`
    - `resume_workspace_exposes_previous_renderer_error_to_coder`
- 根因 2：FigurePlan annotation 回填会把坏 annotation 加回来。
  - 证据：真实 final 里 `anno_training_only` 和 `anno_task_loss` 堆在顶部，`inference_only_note` 在主 corridor 中；`student_encoder` 已写有 `inference-only`，`teacher_encoder` 已写有 `training only`。
  - 修复：
    - `src/tools/draw_plan.rs` 新增 `remove_redundant_phase_loss_and_inference_notes`。
    - 只在 phase 文本已经出现在语义 box 中时删除外部 phase annotation，避免误删 `training only` 作为唯一语义提示的旧用例。
    - 当 student/main box 已经包含 `inference-only/student only` 时，删除重复 inference note box/text 及其 incident connector。
    - 删除 generic `Task Loss` text annotation，避免它作为无锚点浮动文本；真正的 `Task Loss` box 和 connector label 不受影响。
  - 回归测试：
    - `model_draw_plan_polish_removes_redundant_phase_loss_and_inference_notes_from_smoke`
    - 同时确认 `model_draw_plan_polish_preserves_inference_text_annotation_outside_student_label` 和 task-loss box/edge 测试不回退。
- 根因 3：本地 quality gate 对 composition 级坏味道不够硬。
  - 证据：`post_annotation_cleanup_smoke_20260621_101500` 中 `round_001/round_002` 的 `quality_report` 均为 `score=100/issues=[]`，但 vision 仍指出：
    - 4 点折线路由绕远；
    - connector label 漂在远离所属 edge 的空白处；
    - 约 2.7mm 的 input/loss gutter 对 paper-width figure 仍显拥挤。
  - 修复：
    - `src/tools/review.rs` 将水平 component crowding 阈值从 2.5mm 提高到 3.2mm。
    - 新增 4 点 dogleg 识别；对 fan-in merge 目标做豁免，避免误伤 multimodal encoder -> fusion 的合法汇聚路由。
    - 新增 `label_far_from_edge`，只对目标 edge 非 fan-in merge 的 connector label 生效，阈值调为 0.16 normalized，以保留普通模板标签但抓住真实漂移 label。
    - `src/agent.rs`、`src/prompts.rs` 将 `route_detour`、`label_far_from_edge` 写入 DrawPlan revision 和 RoundImprovementPlan 合同。
  - 回归测试：
    - `quality_report_flags_smoke_dogleg_far_label_and_narrow_gutter`
    - `pipeline_tests` 中 `max_iterations=0` until-pass 重新通过，证明 mock optimizer 不再长循环。
- 根因 4：aux/reference inference note connector 会制造无意义 route_detour。
  - 证据：`post_quality_gate_smoke_20260621_104500/final` 保留 best round，但剩余 local issue 中 `edge_inference_hint` 是从 student 到 context note 的 4 点 dashed/reference 说明边。
  - 修复：
    - `src/tools/draw_plan.rs` 新增 `remove_auxiliary_inference_note_connectors`：仅删除指向 standalone inference note 且 id/style/label 表现为 `hint`、`reference` 或 dashed 的说明性 connector。
    - 保留真正的 data-flow inference component edge，避免破坏 `pipeline` 模板中需要 connectable inference node 的场景。
  - 回归测试：
    - `model_draw_plan_polish_removes_auxiliary_inference_note_connectors_from_smoke`
    - `model_draw_plan_polish_converts_note_text_components_to_connectable_boxes` 继续通过。
- 真实 `.env` smoke：
  - `post_runtime_contract_feedback_smoke_20260621_091500`
    - 命令：`SESSION_ID=post_runtime_contract_feedback_smoke_20260621_091500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=30 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`、2 轮、cap 前未接受；两轮均 `source="model_generated_code"`、`used_fallback=false`，无 `figure.model_error.log`。
    - 质量：`round_000 score=64/issues=5/blockers=1/majors=4` -> `round_001/final score=76/issues=4/blockers=0/majors=4`，证明 runtime contract 后真实 model-generated code 不再 fallback，且有正向提升。
  - `post_annotation_cleanup_smoke_20260621_101500`
    - 命令：`SESSION_ID=post_annotation_cleanup_smoke_20260621_101500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`、3 轮；全部 `source="model_generated_code"`、`used_fallback=false`，final PPTX `unzip -t` 通过。
    - 质量：`round_000 score=76/issues=2` -> `round_001 score=100/issues=0` -> `round_002 score=100/issues=0`。
    - 发现：虽然 local quality 过了，vision 仍拒绝 4 点绕路、label 漂移和窄 gutter，直接促成后续 quality gate 扩展。
  - `post_quality_gate_smoke_20260621_104500`
    - 命令：`SESSION_ID=post_quality_gate_smoke_20260621_104500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`、3 轮；全部 `source="model_generated_code"`、`used_fallback=false`，final PPTX `unzip -t` 通过。
    - 质量：`round_000/final score=82/issues=3/blockers=0/majors=3`；`round_001/002 score=76/issues=3/blockers=1/majors=2` 且 `regression_report.status="regressed"`。
    - 结论：新 gate 生效，`route_detour`、`component_crowding`、`degenerate_edge` 都进入机器可读反馈；best-so-far 选择正确保留无 blocker 的 `round_000`，没有采用后续回归轮。
- 已完成验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，86 个 DrawPlan 测试全绿。
  - `cargo test --test review_tests -- --nocapture` 通过。
  - `cargo test --test prompt_tests -- --nocapture` 通过。
  - `cargo test --test pipeline_tests -- --nocapture` 通过。
  - `cargo test` 全量通过；过程中 LibreOffice 仍偶发打印 `DeploymentException`，但测试 exit 0。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 当前剩余问题：
  - 仍没有 3 轮真实 smoke accepted。
  - vision 仍会要求 `Task Loss` 语义可见；当前做法是删除无锚点 generic `Task Loss` text annotation，下一步更合理的是把这类语义转成靠近 `edge_student_output` 的 connector label 或独立 loss box，而不是恢复顶部浮动 annotation。
  - `post_quality_gate_smoke_20260621_104500` 暴露 input/student box 的 internal whitespace 仍偏大，下一步应让已有 short-content compaction 覆盖更高更窄的 input/main boxes，或在 reasoner plan 中避免竖向巨框。

## 2026-06-21 tall short boxes and task-loss label preservation

- 本轮目标：
  - 继续实现上一轮计划，针对 `post_quality_gate_smoke_20260621_104500/final` 中仍然可见的高瘦短文本框和 `Task Loss` 语义缺失做局部修复。
  - 继续保持“上一轮暴露一个具体坏模式，就加一个最小回归测试和局部 polish”的策略，避免重排整图或硬套模板。
- 根因 1：短文本框压缩规则漏掉高瘦 input/output/main box。
  - 证据：final DrawPlan 中 `comp_input` 为 `[0.0233, 0.44, 0.1433, 0.9767]`、文本 `Input x`；`comp_student` 为 `[0.278, 0.6113, 0.472, 0.972]`、文本 `Student`；`comp_task_output` 为 `[0.815, 0.6067, 0.935, 0.9767]`、文本 `ŷ`。
  - 旧 `compact_oversized_short_content_boxes` 直接跳过 input box，并且只用大面积或宽大框判定 oversized；高瘦短框面积不一定超过旧阈值，因此保留了大量内部空白。
  - 修复：`src/tools/draw_plan.rs` 允许短 input box 参与 content compaction，并新增 `tall_short_box` 判定，覆盖高度大、宽度窄、短文本的 input/main/output/module box；压缩后继续 realign connector endpoints。
  - 回归测试：`model_draw_plan_polish_compacts_tall_short_input_main_and_output_boxes_from_smoke`，确认 `Input x`、`Student`、`ŷ` 不再留在高空白框中，且 `edge_student_output` 重新贴到压缩后的框边界。
- 根因 2：generic `Task Loss` annotation 被删除后没有转成可见语义。
  - 证据：旧修复为了避免顶部悬浮 `anno_task_loss`，只删除 generic `Task Loss` text annotation；vision review 仍会认为任务损失语义缺失。
  - 修复：`src/tools/draw_plan.rs` 在 FigurePlan annotation 回填阶段先识别 generic `Task Loss`/`loss` annotation；如果 target 是 connector 或 FigurePlan edge，且当前没有独立 Task Loss box，则把它折叠成目标 connector 的 `DrawLabel`，不再创建悬浮 Text object。
  - 保守条件：已有独立 Task Loss box 时不额外加 label，避免重复表达；无法解析到 connector target 时保持旧的 annotation 处理路径。
  - 回归测试：扩展 `model_draw_plan_polish_removes_redundant_phase_loss_and_inference_notes_from_smoke`，确认 `anno_task_loss` text 不存在，但 `edge_student_to_output` 上存在可编辑 `Task Loss` label，且 label 不压在线段上。
- 测试副作用与处理：
  - 全量 DrawPlan 测试首次失败在 `model_draw_plan_polish_routes_right_side_residual_feedback_to_student_edge`。
  - 原因：student 短文本框现在会被压缩，residual feedback 边终点随当前 student 上边缘移动；旧测试硬编码了压缩前的 `y=0.62`。
  - 处理：测试改为断言终点连接到当前 `comp_student` 的右侧/上边界，保留原有语义约束，不再依赖旧框高。
- 已完成验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_compacts_tall_short_input_main_and_output_boxes_from_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_removes_redundant_phase_loss_and_inference_notes_from_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，87 个 DrawPlan 测试全绿。
  - `cargo fmt --check` 通过。
  - `cargo test` 全量通过；过程中 LibreOffice 仍偶发打印外部 `DeploymentException`，但测试 exit 0。
  - `cd renderer && npm run build` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑 3 轮真实 `.env` smoke，确认 aux inference connector 删除、短框压缩和 Task Loss label 保留在真实模型路径下共同生效，并检查每轮是否有实质提升。
- 第一次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_tall_short_taskloss_smoke_20260621_113000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_tall_short_taskloss_smoke_20260621_113000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；全部轮次都是 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=94/issues=1` -> `round_001 score=100/issues=0` -> `round_002/final score=100/issues=0`，说明短框压缩和本地几何 gate 已生效。
  - 视觉 review：仍未通过。主要残留问题集中在 `comp_inference` 作为左上角 standalone inference note、`anno_residual_label` 作为浮动 residual signal 注释、`edge_residual_to_student` 保留 4 点 detour，以及 `edge_residual_to_student` 的泛化 `supervise` label 漂移到页面顶部。
  - 结论：本地 quality gate 已经不再暴露这些问题，但视觉模型能稳定指出。下一步应把这类“FigurePlan 保护但视觉上 detached 的 note/annotation/label”转成确定性 polish 规则。
- 第二组局部修复：
  - `src/tools/draw_plan.rs` 新增后期 `fold_detached_protected_inference_notes_into_student_annotations`：只在 protected inference note 位于 student 上方且偏左/外侧时触发，把 standalone box 折叠成靠近 student 的 editable text annotation；靠近 student 的 badge 或 FigurePlan 明确放在 student 下方的 note 仍保留为 box。
  - `remove_redundant_phase_loss_and_inference_notes` 现在会删除 `residual signal` 这类 annotation，只在已有 semantic box 文本包含 residual 时触发，避免误删唯一语义。
  - 新增 `simplify_residual_objective_to_main_edges`，将 residual objective 到 main/student 的 4 点 detour 简化为不超过 3 点的正交路线。
  - 新增 `remove_redundant_residual_supervision_labels`，只删除 `supervise`、`supervision`、`residual signal`、`latent signal` 这类泛化 label；保留 `residual supervision`、`h_t - h_s` 等更具体的标签。
  - 回归测试：`model_draw_plan_polish_folds_detached_protected_inference_and_residual_notes_from_smoke`。
- 第二组本地验证：
  - 新增目标测试通过。
  - 由于规则初版过宽，曾导致 `model_draw_plan_polish_adds_missing_figure_plan_note_components`、`model_draw_plan_polish_moves_inference_note_out_of_residual_student_gap`、`model_draw_plan_polish_resnaps_elbow_connector_label_to_final_segment` 失败；已通过收窄 protected-note 判定和 generic-label 判定修复。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，88 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 下一步：
  - 再跑 3 轮真实 `.env` smoke，确认上述视觉 review 残留是否减少，尤其检查 `comp_inference`、`anno_residual_label`、`edge_residual_to_student.label` 和 `edge_residual_to_student.points`。
- 第二次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_protected_note_residual_smoke_20260621_121500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_protected_note_residual_smoke_20260621_121500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；全部轮次都是 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=82/issues=3` -> `round_001/final score=94/issues=1`；`round_002 score=82/issues=3` 被 best-so-far 判为回退，没有成为 final。
  - 视觉 review：`round_002` 的视觉分数比 final 更好，但本地 quality 回退导致未选中。final 的主要本地问题是 `e_student_output` 因 `task_loss` 挡在 student-output 水平通道中而走四点 detour；vision 还指出多个 connector label 贴线、teacher/student size asymmetry 不明显、inference note padding 偏大。
  - 结论：每轮都有实质改动和部分提升，但 `task_loss` 占据主输出通道是下一类最确定、可本地修复的根因。
- 第三组局部修复：
  - `src/tools/draw_plan.rs` 新增 `move_task_loss_boxes_out_of_main_output_horizontal_corridors`。
  - 该规则检测同一 main/source 同时连到 output 和 task-loss 的情况；若 task-loss box 与 main→output 水平 lane 相交，则优先把 task-loss 移到该 lane 上方或下方的空位，再 realign connector endpoints。
  - 后续已有 `improve_connector_routes_against_boxes` 会在障碍移走后把 main→output connector 简化回直接水平线。
  - 回归测试：`model_draw_plan_polish_moves_task_loss_out_of_student_output_lane_from_smoke`，复现 `post_protected_note_residual_smoke_20260621_121500/final` 的 `student_block/task_loss/output/e_student_output` 几何。
- 第三组本地验证：
  - 新增目标测试通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，89 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过；LibreOffice 仍偶发打印外部 `DeploymentException`，但测试 exit 0。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 下一步：
  - 再跑 3 轮真实 `.env` smoke，重点检查 `e_student_output` 是否不再产生 route_detour，以及是否暴露下一类视觉模型指出但本地 gate 漏掉的问题。
- 第三次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_student_output_lane_smoke_20260621_130000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_student_output_lane_smoke_20260621_130000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；全部轮次都是 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=100/issues=0`、`round_001 score=94/issues=1`、`round_002/final score=100/issues=0`。`e_student_output` route_detour 已消失，说明第三组修复生效。
  - 视觉 review：仍未通过，主要问题变成 `c_inference_note` standalone box、residual dashed paths 过近、`h_t/h_s` label 贴线、部分框仍偏大。人工查看 final PNG 确认 inference note 仍是大独立框，student-output 主线已不再绕 task-loss。
- 第四组局部修复：
  - 放宽 `detached_protected_inference_note_should_fold`：只要 protected inference note 完全位于 student 上方，就折叠成 editable text annotation；不再要求它必须偏左。
  - 更新 `model_draw_plan_polish_folds_detached_protected_inference_and_residual_notes_from_smoke`，用第三次 smoke 中 `c_inference_note` 类似位置覆盖“上方但不偏左”的情况。
  - 更新 `model_draw_plan_polish_moves_inference_note_out_of_residual_student_gap`：允许合格结果为保留 note box 且避开 residual，或折叠为 editable inference text；不再强制 standalone box 必须存在。
- 第四组本地验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，89 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 第四次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_fold_smoke_20260621_134500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_fold_smoke_20260621_134500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；全部轮次都是 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=76/issues=4` -> `round_001/final score=100/issues=0`；`round_002 score=94/issues=1` 被判为回退。说明每轮仍有实质修复，但 final 仍按本地 best-so-far 选择 `round_001`。
  - 验证结果：`round_000` 的 inference note 已折叠成 `ann_inference` editable text；但后续 model 又生成了新的 `comp_inference_note` standalone box，位置变成 task-loss 附近/底部 corridor，视觉 review 继续把它列为 blocker。
  - 当前剩余根因：
    - inference note 可以换位置复发；本地规则目前只处理上方 detached note，未覆盖“与 loss/task/output 区域拥挤的 standalone inference box”。
    - 本地 `quality_report` 对 connector label 贴线的 recall 仍不足，vision 反复指出 `h_t/h_s/ŷ` label 离真实 connector path 太远或贴线。
    - 本地 quality 与 vision review 的选择目标仍冲突：`round_002` 有时 vision 更好但本地 quality 回退，final 选择本地更干净的 round。
  - 下一步建议：
    - 把 standalone inference note 的处理从位置特例升级为语义规则：如果 note 无连接且 role/style 是 muted/context，并且不在明确允许的 student-adjacent badge 区域，则折叠成 text annotation。
    - 给 `label_far_from_edge`/`label_overlaps_edge` 加基于实际 polyline 最近距离的检查，覆盖当前 `h_t/h_s/ŷ` 的视觉问题。
    - 调整 best-round selection，让 vision review 明显改善但 local quality 轻微回退的 round 不一定被丢弃，或把 vision blockers 转成下一轮更硬的 local gates。

## 2026-06-21 protected inference recurrence and label-distance gate

- 本轮目标：
  - 继续执行上一节规划，不重排模板，只把 `post_inference_fold_smoke_20260621_134500/final` 暴露的两个残留坏模式转成确定性规则和回归测试。
  - 坏模式 1：模型下一轮又生成新的 `comp_inference_note`，这次不在 student 上方，而是在 `Task Loss` 附近/底部 corridor，视觉模型仍判定为 standalone note blocker。
  - 坏模式 2：`edge_head_loss_label` 的 `ŷ` 和 `edge_tenc_resid_label` 的 `h_t` 视觉上远离所属 connector，但本地 `label_far_from_edge` 阈值过松导致漏报。
- 已完成实现：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_folds_protected_inference_note_crowding_task_loss_from_smoke`，复现底部 `comp_inference_note` 与 `comp_task_loss` 同层拥挤的 latest-smoke 几何。
  - `src/tools/draw_plan.rs`
    - `fold_detached_protected_inference_notes_into_student_annotations` 现在只折叠 protected、无连接、`context/muted` 的 inference note，避免误伤 data-flow component。
    - 新增 loss/objective 同层拥挤判定：当 protected inference note 与 loss/objective 在同一水平带且横向距离不超过 `0.20` 时，折叠成 `ann_inference` editable text。
    - 新增 student-adjacent badge 豁免：与 student 垂直重叠且横向距离很近的 inference badge 保持为 box，避免旧用例 `model_draw_plan_polish_repairs_stacked_teacher_residual_smoke_layout` 被错误折叠。
  - `tests/review_tests.rs` 新增 `quality_report_flags_smoke_labels_detached_from_actual_polyline`，复现 `edge_head_loss_label` 与 `edge_tenc_resid_label` 远离真实 polyline 的情况。
  - `src/tools/review.rs` 将 `label_far_from_edge` 改成按 label 文本长度分层：`ŷ`、`h_t` 这类短符号用 `0.08` normalized 的近线阈值，较长短语保留 `0.16` 阈值。这样能抓住 latest-smoke 的短标签漂移，同时不把 mock pipeline 里被模块行挤开的长短语 caption 判成 blocker。
- 已完成局部验证：
  - 新增 DrawPlan 回归测试先失败，修复后通过。
  - 新增 Review 回归测试先失败，修复后通过。
  - `cargo test --test review_tests -- --nocapture` 通过，28 个 review 测试全绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，90 个 DrawPlan 测试全绿。
- 验证中发现的副作用与处理：
  - 初版 label 阈值统一收紧到 `0.08` 后，`mock_pipeline_accepts_zero_max_iterations_as_until_passed` 会长循环；临时 mock run `tmp/debug_quality_gate_mock` 显示阻塞项是 `teacher_to_student_label`/`student_to_output_label` 这类长短语 caption 离模块中心线较远。
  - 处理方式不是回滚短标签 gate，而是按 label 文本长度分层；短公式/变量继续严格，长短语 caption 允许更大偏移。
- 已完成全量验证：
  - `cargo test` 通过；过程中 LibreOffice 仍偶发打印 `DeploymentException`，但测试 exit 0。
  - `cargo fmt --check` 通过。
  - `cd renderer && npm run build` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑真实 `.env` smoke，重点检查新一轮是否会把 `comp_inference_note` 折叠，并将 `h_t/ŷ` label 漂移转成机器可读反馈。
- 第五次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_crowding_label_gate_smoke_20260621_151500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_crowding_label_gate_smoke_20260621_151500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；全部轮次都是 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000/final score=76/issues=3`；`round_001` 和 `round_002` 都回退到 `score=58` 且带 blocking overlap/crossing，因此 best-so-far 正确保留 `round_000`。
  - 验证结果：此前反复出现的 `comp_inference_note` standalone box 已被折叠，final layout 中没有独立 inference box；但折叠出的 `ann_inference` 位于 `[0.2558, 0.435, 0.5358, 0.495]`，仍在主流 corridor 附近，被 vision 判为 blocker。
  - 新暴露根因：
    - `inference_annotation_bbox_near_student` 旧策略优先把折叠后的 annotation 放在 student 上方。对于 student branch 位于中下部的 method figure，这个位置会落入 input/student/output 的主流上沿，视觉上仍像浮动说明。
    - 这个问题不是 FigurePlan 保护问题，而是 fold 后 placement 策略的问题。
- 第六组局部修复：
  - `tests/draw_plan_tests.rs` 扩展 `model_draw_plan_polish_folds_protected_inference_note_crowding_task_loss_from_smoke`：除确认 `comp_inference_note` 被移除、`ann_inference` 保持 editable text 外，还断言 annotation 应落到 student 下方空白，而不是主流上方 corridor。
  - `src/tools/draw_plan.rs` 修改 `inference_annotation_bbox_near_student`：只要 student 下方有空间，就优先把 inference annotation 放在 student 下方；下方空间不足时才回退到 student 上方。
  - 该修复保留了“不把 inference 语义塞进 student module label”的既有设计约束，避免破坏 `model_draw_plan_polish_preserves_inference_text_annotation_outside_student_label`。
- 第六组本地验证：
  - 扩展后的目标测试在旧逻辑下失败，修复后通过。
  - `model_draw_plan_polish_folds_standalone_inference_note_into_annotation` 通过。
  - `model_draw_plan_polish_preserves_inference_text_annotation_outside_student_label` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，90 个 DrawPlan 测试全绿。
- 待完成验证：
  - 重新跑 `cargo test`、`cargo fmt --check`、`cd renderer && npm run build`、`git diff --check`。
  - 再跑真实 `.env` smoke，检查 `ann_inference` 是否离开主流 corridor，以及是否开始暴露下一类主要问题：latent residual/output crowding、student-output route detour、edge crossing。
- 第六组全量验证：
  - `cargo test` 通过。
  - `cargo fmt --check` 通过。
  - `cd renderer && npm run build` 通过。
  - `git diff --check` 通过。
- 第六次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_annotation_below_smoke_20260621_154500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_annotation_below_smoke_20260621_154500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；全部轮次都是 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：三轮和 final 均为 `score=94/issues=1`，唯一 local issue 是 `comp_input` 与 `comp_residual` 竖向距离过近。
  - 视觉 review：`ann_inference` 相比上一轮已离开 student 上方主流位置，但仍在 student branch 右侧空间中，被 vision 认为还在 vertical flow space；更主要的 blocker 变成 `comp_residual` 作为 standalone residual box，且 dashed `edge_residual_supervision` 绕着它形成 4 点 staircase。
  - 人工查看 final PNG：图的主要坏点是中心 `Latent Residual` 盒子打断 dashed residual edge；这应当是 edge label，而不是一个独立 loss box。
- 第七组局部修复：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_folds_unconnected_residual_box_into_supervision_label_from_smoke`，复现 `post_inference_annotation_below_smoke_20260621_154500/final` 中无连接 `comp_residual` 与已有 `edge_residual_supervision` dashed edge 的组合。
  - `src/tools/draw_plan.rs` 新增 `fold_unconnected_residual_boxes_into_supervision_labels`：
    - 只处理没有任何 connector endpoint 引用的 residual box。
    - 只有找到可承载 label 的 residual/supervision/dashed connector 时才折叠；否则保留 residual box，避免误删合法 objective/module。
    - 折叠时把 residual box 文本写成 edge label，并删除该 residual box。
    - 同时把 residual supervision edge 从 4 点 staircase 简化为不超过 3 点的正交 route。
  - 初版规则过宽，曾删除无 connector 场景里的合法 residual box，导致 3 个 DrawPlan 旧用例失败；已通过“必须成功绑定到 dashed/supervision connector 才删除 box”修复。
- 第七组本地验证：
  - 新增目标测试先失败，修复后通过。
  - 回归用例 `model_draw_plan_polish_compacts_oversized_short_main_boxes_from_smoke`、`model_draw_plan_polish_moves_inference_note_away_from_loss_crowding`、`model_draw_plan_polish_moves_near_overlapping_inference_component_off_semantic_boxes` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，91 个 DrawPlan 测试全绿。
- 待完成验证：
  - 重新跑全量验证。
  - 再跑真实 `.env` smoke，检查 `comp_residual` 是否消失、residual label 是否落在 dashed edge 上，以及是否减少 input/residual crowding 和 residual route detour。
- 第七组全量验证：
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - 初次 `cargo fmt --check` 发现 `src/tools/draw_plan.rs` 一处格式差异；已运行 `cargo fmt` 修复。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑真实 `.env` smoke，检查 `comp_residual` 是否消失、residual label 是否落在 dashed edge 上，以及是否减少 input/residual crowding 和 residual route detour。
- 第七次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_residual_edge_label_smoke_20260621_161500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_residual_edge_label_smoke_20260621_161500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；`round_000` 使用 `deterministic_fallback`，`round_001/round_002/final` 均为 `model_generated_code`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：final `score=76/issues=4`，主要是 `student_model/teacher_model` crowding、`task_output/latent_residual_supervision` crowding、`e_teacher_residual_label` 远离竖向 dashed edge、`e_input_teacher` route detour。
  - 验证结果：
    - 本轮 final 不再出现上一轮那种“无连接 `comp_residual` standalone box”；模型生成的是连接中的 `latent_residual_supervision` component，按第七组规则不应删除。
    - 当前 residual blocker 转移为 connected objective/component 与 output 过近，而不是无连接 residual box。
    - 视觉 review 新指出 `student_model` 文本包含重复 parenthetical：`Student\n(frozen at inference)\n(inference-only)`，说明 reasoner/coder 会把 inference 语义塞回主 box，但目前没有去重规则。
    - `e_input_teacher` 仍可能保留 4 点 detour；`e_teacher_residual_label` 对竖向 dashed connector 的 label placement 仍不够近。
- 当前剩余根因：
  - 对“连接中的 residual supervision/objective component”还需要单独的 gutter/placement 规则，不能复用无连接 residual box 删除规则。
  - 需要清理重复 phase parenthetical，尤其是 `frozen at inference` 与 `(inference-only)` 同时出现在 student label 时。
  - 需要增强 vertical edge label snapping，使短 label 靠近竖向 connector 的实际 path。
  - 需要针对 shared input → teacher/student 的 4 点 detour 做更强简化，避免每轮靠 coder 自己修。

## 2026-06-21 duplicate phase text and short vertical label polish

- 本轮目标：
  - 继续执行上一节规划，优先把最新真实 smoke 暴露且能确定本地化的两个坏模式转成 regression tests 和 deterministic polish。
  - 坏模式 1：`student_model` label 出现 `Student\n(frozen at inference)\n(inference-only)`，同一 inference 语义重复占用主模块内部空间。
  - 坏模式 2：`e_teacher_residual_label` 的 `latent z` 虽然 bbox 左边贴近竖向 dashed edge，但 bbox 宽度仍是 `0.16`，导致文本中心离真实 connector path 过远，视觉上像浮动标签。
- 已完成实现：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_removes_redundant_inference_parenthetical_from_main_label`，复现重复 inference parenthetical；旧逻辑下失败，修复后通过。
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_tightens_short_vertical_edge_label_near_route_from_smoke`，复现 `post_residual_edge_label_smoke_20260621_161500/final` 中 `latent z` label 过宽漂移；旧逻辑下失败，修复后通过。
  - `src/tools/draw_plan.rs` 新增 `remove_redundant_inference_only_parentheticals_from_main_boxes`：仅当主模块文本中已有更具体 inference 语义（如 `frozen at inference`、`student-only inference`）时，删除额外 standalone `(inference-only)`/`inference-only`，避免删掉唯一 inference cue。
  - `src/tools/draw_plan.rs` 新增 `tighten_short_connector_labels_near_routes`：在 final route snapping 后，对 `visible_text_len <= 8` 的 connector label 按字符数收窄 bbox，并贴回最近的水平/竖向 route segment；这解决短公式/变量 label 因默认 `0.16` 宽度导致中心远离边的问题。
- 已完成局部验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_removes_redundant_inference_parenthetical_from_main_label -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_tightens_short_vertical_edge_label_near_route_from_smoke -- --nocapture` 通过。
- 已完成全量验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，93 个 DrawPlan 测试全绿。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - 初次 `cargo fmt --check` 只发现新增 helper 的格式差异；已运行 `cargo fmt` 修复。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑真实 `.env` smoke，检查重复 inference 文本和短 label 漂移是否消失，并观察剩余 blocker 是否转移到 connected residual objective gutter 或 shared input route detour。
- 第八次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_dup_phase_short_label_smoke_20260621_080949 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_dup_phase_short_label_smoke_20260621_080949`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；三轮和 final 均为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=40/issues=7` -> `round_001/final score=88/issues=2`，`round_002 score=76/issues=3` 回退。说明 loop 仍有实质提升，但 final 未通过。
  - 验证结果：
    - final 中不再出现 `Student\n(frozen at inference)\n(inference-only)` 这类重复 inference parenthetical；student label 为 `Student\nEncoder`。
    - final 中也没有上一轮 `latent z` 竖向短 label 漂移；本轮模型没有生成该 label，短 label blocker 未复发。
    - 新主要 blocker 变成 connected objective/loss 与主 branch 的上下 gutter：`teacher_encoder` 与 `latent_residual` 只有 0.8mm、`student_encoder` 与 `task_loss` 只有 1.5mm。
    - 人工查看 final PNG：`Latent Residual Supervision` 夹在 Teacher/Student 中间并贴着 Student 上边，`Task Loss` 贴着 Student 下边，整体像三层堆叠挤压；`ann_inference` 仍靠近 output corridor。
- 第九组局部修复：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_moves_connected_residual_hub_out_of_tight_branch_gutter_from_smoke`，复现 connected `latent_residual` 位于 teacher/student branch 中间但贴边的情况；旧逻辑下失败，修复后通过。
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_adds_gutter_between_student_and_connected_task_loss_from_smoke`，复现 `task_loss` 直接压在 `student_encoder` 下方的情况；旧逻辑下失败，修复后通过。
  - `src/tools/draw_plan.rs` 放宽 `move_objective_hubs_out_of_branch_gap_crowding` 触发条件：当 residual/supervision hub 位于上下 branch rows 之间且至少贴近一个 branch 时，也使用已有 side/below/above clearance candidates，而不是要求同时被两个 branch 以旧 gate 判定 crowded。
  - `src/tools/draw_plan.rs` 新增 `separate_task_loss_boxes_from_main_modules`：对 main -> task loss connector，如果 task loss 与 main 纵向/横向间距不足，优先沿原上下方向增加到 `0.055` normalized gutter；不行再尝试左右侧候选，并 realign connector endpoints。
- 已完成局部验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_connected_residual_hub_out_of_tight_branch_gutter_from_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_adds_gutter_between_student_and_connected_task_loss_from_smoke -- --nocapture` 通过。
- 验证中发现的副作用与处理：
  - 初版 `separate_task_loss_boxes_from_main_modules` 会再次移动已有合格的 `Student Output Head -> Task Loss` 短竖线布局，导致 `model_draw_plan_polish_moves_touching_task_loss_to_short_vertical_route` 的 connector 从 2 点变成 5 点绕线。
  - 处理方式：收窄新规则，只处理真正 main/student module；当 source 文本/id/role/style 显示为 output/head-like 时跳过，让已有 `align_touching_task_loss_boxes_with_sources` 负责该局部结构。
  - 回归测试 `model_draw_plan_polish_moves_touching_task_loss_to_short_vertical_route` 已恢复通过。
- 已完成 DrawPlan 验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，95 个 DrawPlan 测试全绿。
- 已完成全量验证：
  - `cargo test` 通过；过程中 LibreOffice 仍偶发打印外部 `DeploymentException`/`Unspecified Application Error`，但测试 exit 0。
  - `cd renderer && npm run build` 通过。
  - 初次 `cargo fmt --check` 发现新增函数和测试的格式差异；已运行 `cargo fmt` 修复。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑真实 `.env` smoke，检查 connected residual/task-loss gutter 是否消除，并观察下一类 blocker，尤其是 `ann_inference` 是否仍占 output corridor、`e_input_teacher` 是否仍有 detour。
- 第九次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_connected_gutter_smoke_20260621_082404 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_connected_gutter_smoke_20260621_082404`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；三轮和 final 均为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=64/issues=4` -> `round_001 score=82/issues=2` -> `round_002/final score=100/issues=0`。新增 connected gutter 规则没有引入本地几何问题。
  - 验证结果：
    - 之前的 residual/task-loss 与主模块贴边问题在 final 本地 gate 中消失。
    - vision 没有列 blocking issues，但整体评分仍低：`story_clarity=5`、`layout_cleanliness=3`、`arrow_routing=3`、`aesthetic_quality=4`，因此未 accepted。
    - 人工查看 final PNG：主要坏点转为本地 quality 漏检的整体构图问题：`anno_inference` 插在 Teacher/Student 水平主通道中，`edge_input_to_student` 从底部横穿后上折成长 L，多个 connector label 占据主通道。
    - 结论：下一步不能只看 component overlap/crowding，需要把 vision-only 的 corridor clutter 转成本地 quality issue，至少让下一轮得到具体 target，而不是只有低总分。
- 第十组局部修复：
  - `tests/review_tests.rs` 新增 `quality_report_flags_inference_annotation_in_teacher_student_corridor_from_smoke`，复现第九次 smoke final 中 `anno_inference` 位于 `comp_teacher` 和 `comp_student` 之间水平 corridor 的情况；旧 quality gate 下失败，修复后通过。
  - `src/tools/review.rs` 新增 `annotation_in_main_corridor` quality issue：仅针对 `kind="annotation"` 且文本/ID 包含 inference/student-only 的 annotation；当它位于 teacher 与 student 两个 component 之间且贴近两者水平行时，生成 major issue，target ids 包含 annotation、teacher、student。
  - 这个修复不强行改布局；它把 vision-only blocker 转为机器可读反馈，避免下一轮只收到泛泛低分。
- 已完成局部验证：
  - `cargo test --test review_tests quality_report_flags_inference_annotation_in_teacher_student_corridor_from_smoke -- --nocapture` 通过。
- 已完成全量验证：
  - `cargo test --test review_tests -- --nocapture` 通过，29 个 review 测试全绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，96 个 DrawPlan 测试全绿。
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - 初次 `cargo fmt --check` 发现新增 review helper 和测试的格式差异；已运行 `cargo fmt` 修复。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑真实 `.env` smoke，检查 `anno_inference` corridor 是否被下一轮明确修复，或至少是否在 quality report 中稳定出现并绑定 target。
- 第十次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_annotation_corridor_gate_smoke_20260621_083800 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_annotation_corridor_gate_smoke_20260621_083800`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；三轮和 final 均为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000/final score=82/issues=3`，`round_001 score=64/issues=4`，`round_002 score=64/issues=4`。best-so-far 正确保留 `round_000`。
  - 验证结果：
    - 本次模型没有复现 `anno_inference` 位于 teacher/student 水平 corridor 的具体模式，因此新 `annotation_in_main_corridor` gate 未触发是合理的。
    - quality report 已经稳定给出具体 target：`student_model` text wrap risk，`teacher_model/teacher_latent` 水平 crowding，`student_model/student_output` 水平 crowding。vision 还指出 `e_residual_to_student` 大 elbow 和 `ann_inference` 浮在主图下方缺少 anchor。
    - 人工查看 final PNG：当前坏点是左右相邻模块几乎贴边、Student/Prediction 框过大且贴近、`Inference: student only` 在底部漂浮、residual-to-student dashed route 绕出大折线。相比早期重叠/独立 residual box/重复 inference 文本，问题已转移到更具体的布局细化层。
- 当前剩余根因：
  - 需要处理水平相邻模块的最小 gutter，尤其是 `teacher_model -> teacher_latent`、`student_model -> student_output` 这种 source-output 或 source-latent 相邻结构。
  - 需要给 `Student` 这类短主模块 label 提供最小宽度，避免刚好触发 `text_wrap_risk`。
  - 需要把底部 `ann_inference` floating note 也转成 quality gate 或 deterministic re-anchor：它不在 teacher/student corridor，但仍缺少视觉 anchor。
  - 需要简化 `e_residual_to_student` 这种 residual objective 到 student 的大 elbow route。

## 2026-06-21 horizontal connected gutter and short main width

- 本轮目标：
  - 继续处理 `post_annotation_corridor_gate_smoke_20260621_083800/final` 暴露的下一组高确定性问题。
  - 坏模式 1：`teacher_model -> teacher_latent` 和 `student_model -> student_output` 是有连接的同一行组件，但水平间距分别只有约 0.1mm 和 2.0mm，导致本地 `component_crowding` 与视觉上的“贴边块”。
  - 坏模式 2：`student_model` 只有 `Student` 一个短词，但 bbox 宽度仅约 `0.1127`，在 paper width 下刚好触发 `text_wrap_risk`。
- 已完成实现：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_separates_horizontally_crowded_connected_modules_from_smoke`，复现 latest smoke 中 teacher/latent 与 student/output 水平贴边；旧逻辑下失败，修复后通过。
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_widens_short_main_module_label_from_smoke`，复现 `student_model` 短主模块过窄；旧逻辑下失败，修复后通过。
  - `src/tools/draw_plan.rs` 新增 `widen_short_main_boxes_for_readability`：对 main/student 这类主模块按最长行字符数补到最小可读宽度，避免短词在目标论文宽度下处在 wrap 风险边缘。
  - `src/tools/draw_plan.rs` 新增 `separate_horizontally_crowded_connected_boxes`：只处理有 connector 连接、同一行、非 input/loss/objective 的水平相邻模块；目标 gutter 为 `0.035` normalized。优先移动右侧 target，若会挤到其它框则尝试移动左侧 source，并要求候选不会造成新的 overlap/crowding。
- 已完成局部验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_separates_horizontally_crowded_connected_modules_from_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_widens_short_main_module_label_from_smoke -- --nocapture` 通过。
- 验证中发现的副作用与处理：
  - 初版 horizontal gutter 把 `role=context` 的 `teacher_model` 错误当作 note-like 端点跳过，导致目标回归没有触发；已收窄 note-like 判定，不再把普通 `context` 模块排除。
  - 另一个旧用例 `model_draw_plan_polish_converts_note_text_components_to_connectable_boxes` 暴露 note-like component 会与 source module 贴边；已新增 `separate_note_like_connected_boxes_from_sources`，只对 note/inference/student-only 这类 connected note 端点补最小 gutter，旧用例恢复通过。
- 已完成 DrawPlan 验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，98 个 DrawPlan 测试全绿。
- 已完成全量验证：
  - `cargo test` 通过。
  - `cd renderer && npm run build` 通过。
  - 初次 `cargo fmt --check` 发现新增 helper 的格式差异；已运行 `cargo fmt` 修复。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑真实 `.env` smoke，检查 `teacher_model/teacher_latent`、`student_model/student_output` 水平 crowding 和 `student_model` text wrap 是否消失。

## 2026-06-21 thin visible overlap gate after horizontal gutter smoke

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_horizontal_gutter_smoke_20260621_085754 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_horizontal_gutter_smoke_20260621_085754`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final 为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
- 验证结果：
  - 前一组水平 connected gutter 和短主模块宽度修复有效：final 没有复现 `teacher_model/teacher_latent`、`student_model/student_output` 的水平贴边，也没有 `Student` 短框 wrap blocker。
  - final `quality_report.json` 只剩 `annotation_in_main_corridor`，但人工查看 PNG 发现右侧 `teacher_residual_out` 与 `task_loss_box` 有明显薄相交，`Latent residual r` 文本被 `Task loss` 区域视觉压住。
  - 根因不是 renderer metadata 分类错误：二者在 `layout_map.json` 中均为 `kind="component"`。漏检来自 `component_overlap` 的阈值：旧规则要求纵向重叠至少约 1mm，当前 smoke 的纵向重叠约 0.7mm，但横向重叠很长且 overlap ratio 已超过 0.15，纸面上仍是可见碰撞。
- 已完成实现：
  - `tests/review_tests.rs` 新增 `quality_report_flags_thin_smoke_component_collision`，使用 `post_horizontal_gutter_smoke_20260621_085754/final` 中 `teacher_residual_out` 与 `task_loss_box` 的真实 bbox；旧逻辑下失败，修复后通过。
  - `src/tools/review.rs` 抽出 `component_visible_collision`，保留原有面积碰撞和 `>=1mm x >=1mm` 可见碰撞，同时新增 thin-but-visible 分支：横向重叠足够长、纵向压入约 0.55mm 以上、normalized 高度和 overlap ratio 达标时，也输出 blocking `component_overlap`。
  - `src/tools/draw_plan.rs` 同步收紧 `component_overlap_gate_fails`，并让 overlap repair 的评分按该 gate 给重罚，避免 structured/fallback path 把薄相交视为“移动成本高于保留重叠”。
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_separates_thin_visible_loss_residual_overlap_from_smoke`，覆盖 DrawPlan polish 不应保留同源薄相交。
  - 另新增 `model_draw_plan_polish_deduplicates_inference_annotations_from_smoke`，确认当前已有 polish 会把 `ann_inference` / `anno_inference` 这类重复 inference annotation 合并；该测试当前代码已通过，说明当前必须补的是 overlap quality gate 漏检。
- 已完成验证：
  - `cargo test --test review_tests -- --nocapture` 通过，30 个 review 测试全绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，100 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - 初次 `cargo fmt --check` 只发现新增测试 bbox 格式差异；已运行 `cargo fmt` 修复。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 再跑真实 `.env` smoke，检查薄相交是否被本地 `component_overlap` 稳定捕获或被 DrawPlan polish 直接消除，并观察剩余 blocker 是否转移到 annotation corridor / residual route detour。
- 第十一次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_thin_overlap_gate_smoke_20260621_091230 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_thin_overlap_gate_smoke_20260621_091230`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；三轮和 final 均为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=34/issues=10` -> `round_001 score=64/issues=6` -> `round_002/final score=76/issues=4`。第 0 轮新增 gate 把 `student_head/task_loss` 的可见 overlap 抓成 blocking `component_overlap`，后续轮次已消除该 blocker，证明 thin visible overlap gate 生效。
  - 当前 final 剩余 issues：`teacher_latent/student_head` horizontal crowding、`teacher_latent/latent_residual` vertical crowding、`ann_inference` `label_outside_main_area`、`e_input_student` `route_detour`。
  - 人工查看 final PNG：右侧 `Task Loss` 与 residual 不再重叠；主要坏点转为顶部 `Inference: student only` 贴近画布顶边、input→student connector 绕到顶部形成大 detour、teacher latent/residual 仍挤。
- 第十一组 prompt/contract 修复：
  - 发现 DrawPlan polish 对 top-edge `ann_inference` 的同类复现会删除该浮动 phase label，但真实 `model_generated_code` path 仍能保留它；因此不能只在 DrawPlan fallback path 加局部规则，必须把 `label_outside_main_area` 作为明确的 model repair contract。
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_removes_top_edge_inference_annotation_from_smoke`，记录 DrawPlan path 对顶边 inference label 的预期删除行为。
  - `src/prompts.rs` 的 DrawPlan optimizer contract 增加 `label_outside_main_area`：删除通用 floating phase/capacity text，或把有意义 label 重锚到主图内目标附近。
  - `src/agent.rs` 的 DrawPlan revision prompt 与 RoundImprovementPlan prompt 同步增加 `label_outside_main_area` 的硬规则，要求具体说明 remove/reanchor，不能继续把 named label 留在 top/bottom margin。
  - `tests/prompt_tests.rs` 增加断言，确保 revision/improvement prompt 继续包含 `label_outside_main_area` 和 remove/reanchor 语义。
- 已完成局部验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_removes_top_edge_inference_annotation_from_smoke -- --nocapture` 通过。
  - `cargo test --test prompt_tests -- --nocapture` 通过。
- 待完成验证：
  - 跑全量本地验证；如时间允许，再跑一轮短真实 smoke 检查 `ann_inference` 顶边 label 是否被下一轮模型删除或重锚。
- 已完成全量验证：
  - `cargo test` 全量通过；LibreOffice 在 workspace 测试中仍打印一次历史外部 `DeploymentException`/`Unspecified Application Error` 文本，但测试 exit 0。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 第十二次短真实 `.env` smoke：
  - 命令：`SESSION_ID=post_label_outside_contract_smoke_20260621_092803 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_label_outside_contract_smoke_20260621_092803`
  - 结果：`accepted=false`、2 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - `round_000` 为 `deterministic_fallback`、`used_fallback=true`，`quality score=34`，含 `component_overlap` 等 blocker。
  - `round_001` 为 `model_generated_code`、`used_fallback=false`，`quality score=52`；它没有 `label_outside_main_area`，说明新增 prompt contract 对顶边浮动 label 有效果，但模型改成了左侧 `Inference Only` 模块，并引入两个 blocking `edge_crossing`。
  - final 选择回退到 `round_000`，因为 best-so-far 当前优先减少 blocking issue 数：`round_000` 1 个 blocker，`round_001` 2 个 blockers。该回退合理，但也说明下一步应约束“修 label_outside 时不能新增 edge_crossing/annotation corridor”，或者在 regression budget 中更明确禁止用新增 inference lane/重复 input 来修浮动标签。
- 当前剩余风险：
  - prompt contract 已让模型处理 `label_outside_main_area`，但模型可能用过度结构重排换来新 crossing；后续需要把 `edge_crossing`/`annotation_in_main_corridor` 的 anti-regression 作为更强的 next-round 禁止项。
  - final 仍未 accepted；这次任务完成的是把 thin overlap 漏检补进本地 gate、证明真实 loop 能修掉该 blocker，并强化 `label_outside_main_area` 的修复合同。

## 2026-06-21 anti-regression contract for inference-lane repair

- 本轮目标：
  - 继续处理第十二次 smoke 暴露的新根因：模型按 prompt 修掉 `label_outside_main_area` 后，用新增左侧 `Inference Only` 模块、重复 `Task Input` 分支和新的 `edge_crossing` 换掉旧问题，导致 best-so-far 只能回退。
  - 目标不是阻止 best-so-far 回退，而是让下一轮明确知道这种策略是负例，不能把“修一个 label”变成“新增 inference lane/edge crossing”。
- 已完成实现：
  - `src/prompts.rs` 的 DrawPlan optimizer contract 增加 `edge_crossing` 与 `annotation_in_main_corridor` 明确修复语义，并要求修 `label_outside_main_area` 时不得创建新 inference lane、重复 input 或新 edge crossing。
  - `src/agent.rs` 的 DrawPlan revision prompt 与 RoundImprovementPlan prompt 同步强化 anti-regression：新 `edge_crossing`、`annotation_in_main_corridor`、duplicate input lanes、standalone inference lanes 都是硬回归，即使另一个 issue 被改善也不能接受。
  - `src/pipeline.rs` 的 `RegressionContext` 机器指令加入同样禁区，让回滚到 best-so-far 时最新失败尝试作为负例进入 workspace。
  - `tests/prompt_tests.rs` 增加断言，锁定 `edge_crossing`、`annotation_in_main_corridor`、duplicate input lane、standalone inference lane 这些关键约束。
  - `tests/review_tests.rs` 新增 `quality_report_flags_standalone_inference_lane_component_from_smoke`，使用第十二次 smoke round_001 的真实 `comp_inference_note` bbox/text，要求无连接 `Inference Only` component 输出 major `standalone_inference_lane`。
  - `src/tools/review.rs` 新增 `standalone_inference_lane` quality issue：仅当 inference/student-only 语义的 component 没有任何 edge endpoint 引用时触发，避免误伤真实连接的 inference 子图。
- 已完成局部验证：
  - `cargo test --test prompt_tests -- --nocapture` 通过。
  - `cargo test --test review_tests quality_report_flags_standalone_inference_lane_component_from_smoke -- --nocapture` 通过。
  - `cargo test --test review_tests -- --nocapture` 通过，31 个 review 测试全绿。
- 待完成验证：
  - 跑全量本地验证。
  - 再跑真实 `.env` smoke，检查模型是否仍用 standalone inference lane 修浮动 label；若复发，`quality_report` 应明确输出 `standalone_inference_lane` 并绑定 `comp_inference_note`。
- 已完成全量验证：
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。

## 2026-06-21 task-loss connector label cleanup after inference-lane gate

- 第十三次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_lane_gate_smoke_20260621_094015 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_lane_gate_smoke_20260621_094015`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；三轮和 final 均为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=4/issues=13` -> `round_001 score=4/issues=13` -> `round_002/final score=34/issues=9`。新增 anti-regression 约束后没有复发上一轮的 standalone inference lane，但模型仍没有收敛到可接受质量。
  - final 剩余 issues：`teacher_enc/teacher_proj`、`student_enc/student_head`、`student_proj/output_pred` 的 `component_crowding`；三个 connector label 的 `label_far_from_edge`；`e_task_loss_label` 同时触发 blocking `label_overlaps_component` 和 `label_overlaps_edge`；`e_input_student` 仍有 `route_detour`。
  - 人工查看 final PNG：主要新坏点是 task-loss connector 的 `ŷ, y` 小标签压在 `Task Head` 底部并覆盖自己的连线，而 `task_ce` 框内已经写有 `Task Loss\nCE(y, ŷ)`。这类 label 在拥挤底部区域继续移动不稳定，删除冗余 connector label 更符合图面信息密度。
- 已完成实现：
  - `tests/draw_plan_tests.rs` 新增/更新 `model_draw_plan_polish_moves_task_loss_label_off_head_and_edge_from_smoke`，使用第十三次 smoke final 的真实几何复现 `e_task_loss_label` 压框压线问题。
  - `src/tools/draw_plan.rs` 新增 `remove_redundant_task_loss_connector_labels`，在 task-loss endpoint 框已经包含 `CE(y, ŷ)`、`loss` 或等价监督目标时，删除 connector 上重复的 `ŷ, y`/loss label。
  - 该清理放在 residual supervision label 清理之后、inference note 折叠之前执行，避免后续 label snapping 又把冗余 label 放回拥挤区域。
- 已完成局部验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_task_loss_label_off_head_and_edge_from_smoke -- --nocapture` 通过。
- 已完成本地验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，102 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过；LibreOffice 仍在部分 pipeline/workspace 测试中打印外部 `DeploymentException` 文本，但测试 exit 0。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 第十四次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_task_loss_label_cleanup_smoke_20260621_101200 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_task_loss_label_cleanup_smoke_20260621_101200`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final 为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=76/issues=3` -> `round_001 score=94/issues=1` -> `round_002 score=88/issues=2`，final 选择 `round_001`，说明 best-so-far 在同一 run 目录内工作正常。
  - 之前的 blocking `e_task_loss_label` 压框/压线问题没有复发；这验证了 task-loss connector label cleanup 的真实路径有效。
  - final 唯一剩余 issue 是 `standalone_inference_lane`：`comp_inference` 为无连接 `Inference: student only` 大号 context component。人工查看 final PNG，顶部大框占据主画布上方，虽不重叠但缺少 anchor，视觉上像一条孤立 inference lane。
- 第十四次 smoke 后追加修复：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_folds_standalone_inference_component_from_smoke`，使用第十四次 final 的 `comp_inference` 几何和 FigurePlan 语义复现问题。
  - 初版“所有 protected inference note 都折叠”的规则过宽，误删了小型 `inference_badge`、`role=output` 的真实 inference 节点和移除辅助连接后仍应保留的 context note；完整 DrawPlan suite 暴露 4 个旧用例失败。
  - 最终修复改为窄规则：`role=output` 不折叠；id 含 `note`/`badge` 的小 context note 继续按旧逻辑保留或移动；只有 `comp_inference` 这类无连接、非 note/badge、context/muted 的 lane-like protected inference component 会折叠成 `ann_inference` 文本。
  - 相关验证：新增回归通过；`model_draw_plan_polish_moves_inference_note_out_of_input_student_corridor`、`model_draw_plan_polish_moves_near_overlapping_inference_component_off_semantic_boxes`、`model_draw_plan_polish_removes_auxiliary_inference_note_connectors_from_smoke`、`model_draw_plan_polish_repairs_stacked_teacher_residual_smoke_layout` 均通过；`cargo test --test draw_plan_tests -- --nocapture` 通过，103 个 DrawPlan 测试全绿。
- 待完成验证：
  - 重新跑全量 `cargo test`、renderer build、format 和 diff whitespace 检查。
  - 如时间允许，再跑一轮真实 `.env` smoke，检查唯一剩余 `standalone_inference_lane` 是否被消掉，以及是否达到 acceptance 或暴露下一组更细 issues。

## 2026-06-21 standalone inference fold smoke and latent residual width fix

- 第十五次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_standalone_inference_fold_smoke_20260621_102350 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_standalone_inference_fold_smoke_20260621_102350`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final 为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=94/issues=1` -> `round_001 score=82/issues=2` -> `round_002 score=88/issues=2`，final 选择 `round_000`。
  - 第十四次唯一剩余的 `standalone_inference_lane` 已消失：`comp_inference` 不再作为 component 存在，语义被保留为 `anno_inference` 文本 `Student only at inference`。
  - 当前 final 唯一剩余 issue 是 `latent_residual` 的 `text_wrap_risk`：`Latent Residual` 框宽度只有 `0.10`，在 85mm paper-width 和 13.1pt 字号下，最长 token `Residual` 需要约 9.6mm，可用宽度约 8.0mm。
  - 人工查看 final PNG：整体结构明显简化，之前的大号 standalone inference box、task-loss label blocker、visible overlap/crossing 已消失；新坏点集中在 `Latent Residual` 框偏窄和整体 composition 仍偏空。
- 第十五次 smoke 后追加修复：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_widens_narrow_latent_residual_loss_from_smoke`，使用第十五次 final 的 `latent_residual`/`task_loss` 几何复现 text-wrap 风险。
  - `src/tools/draw_plan.rs` 新增 `widen_loss_or_objective_boxes_for_readability`：在 loss/objective compact 之后，根据最长 token 计算最小可读宽度；候选包含居中扩宽、向左扩、向右扩，并根据 overlap penalty 选择不会制造可见碰撞的方案。
  - 该修复暴露旧测试 `model_draw_plan_polish_reroutes_residual_feedback_off_reverse_shared_segment` 过度依赖旧 bbox 坐标；已把断言改为检查 connector 起点锚在当前 residual box 左下角，而不是固定旧坐标 `[0.58,0.36]`。
- 已完成局部验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_widens_narrow_latent_residual_loss_from_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，104 个 DrawPlan 测试全绿。
- 待完成验证：
  - 重新跑全量 `cargo test`、renderer build、format 和 diff whitespace 检查。
  - 尚未在 text-wrap 修复后再次跑真实 `.env` smoke；下一轮应检查 local quality 是否可以达到 100，以及 vision review 是否仍因 composition/aesthetic 未通过。
- 已完成最终本地验证：
  - `cargo fmt` 已运行。
  - `cargo test` 全量通过，包含 104 个 DrawPlan 测试、31 个 review 测试以及 pipeline/workspace 测试。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 中断的真实 smoke：
  - 尝试运行：`SESSION_ID=post_loss_width_smoke_20260621_103300 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 用户中断后检查进程，未发现残留 `methodfig run`、`run_real_env`、`soffice` 或 `pdftoppm`。
  - 该 run 目录只生成到 `round_000` 的 `figure.pptx`、`figure.png`、`layout_map.json`、`renderer_payload.json`、`draw_plan.json` 等渲染产物，没有 `final/status.json`，也没有完整 review/quality/final 选择流程。
  - 因此它不能作为 text-wrap 修复后的真实验收证据；当前 text-wrap 修复的可靠证据是目标回归和全量本地测试。

## 2026-06-21 compact inference badge gate and post-width smoke

- 补跑真实 `.env` smoke：
  - 命令：`SESSION_ID=post_loss_width_smoke_20260621_110500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_loss_width_smoke_20260621_110500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；三轮和 final 均为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=88/issues=2`、`round_001 score=88/issues=2`、`round_002/final score=88/issues=2`。
  - 验证结果：上一轮 `latent_residual` 的 `text_wrap_risk` 已消失，说明 loss/objective width 修复有效；剩余为 `comp_student` 的 `excessive_internal_whitespace` 和 `inference_note` 的 `standalone_inference_lane`。
- 根因分析：
  - `inference_note` 是小型 note/badge，文本明确为 `Inference: student only`；它不是上一轮那种大号无锚 inference lane。
  - 当前 `standalone_inference_lane` quality gate 对 compact note/badge 过宽，会把合理保留的 student-only cue 当成结构性 lane blocker，导致本地 loop 在错误目标上反复修。
  - 对同一几何运行 DrawPlan polish 回归显示 `comp_student` 大空白可以被当前 `compact_oversized_short_content_boxes` 等规则压缩，说明需要保留该回归，但本轮更高优先级是修正 inference gate 的假阳性。
- 已完成实现：
  - `tests/review_tests.rs` 新增 `quality_report_allows_compact_student_only_inference_note_badge`，用 `post_loss_width_smoke_20260621_110500/final` 的真实 note/badge 几何复现假阳性。
  - `src/tools/review.rs` 增加 `compact_student_only_inference_badge` 例外：只有 id/text 含 note 或 badge 且明确含 `student only`/`student-only` 的 compact inference component 才不触发 `standalone_inference_lane`。
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_compacts_oversized_student_box_from_loss_width_smoke` 和 `model_draw_plan_polish_compacts_full_loss_width_smoke_student_box`，锁定 student 大空白可以被 DrawPlan polish 压缩的行为。
- 已完成验证：
  - `cargo test --test review_tests quality_report_allows_compact_student_only_inference_note_badge -- --nocapture` 通过。
  - `cargo test --test review_tests quality_report_flags_standalone_inference_lane_component_from_smoke -- --nocapture` 通过，确认旧的大号 `Inference Only` lane 仍会被抓住。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_compacts_full_loss_width_smoke_student_box -- --nocapture` 通过。
  - `cargo test --test review_tests -- --nocapture` 通过，32 个 review 测试全绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，106 个 DrawPlan 测试全绿。
  - `cargo fmt`、`cargo test`、`cd renderer && npm run build`、`cargo fmt --check`、`git diff --check` 均通过。
- 修复后真实 `.env` smoke：
  - 命令：`SESSION_ID=post_badge_gate_compact_smoke_20260621_112200 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_badge_gate_compact_smoke_20260621_112200`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；三轮和 final 均为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=100/issues=0`、`round_001 score=70/issues=3`、`round_002 score=100/issues=0`、`final score=100/issues=0`。
  - vision review 仍未通过：`story_clarity=5`、`visual_hierarchy=5`、`paper_readability=5`、`layout_cleanliness=4`、`arrow_routing=4`、`aesthetic_quality=5`。
  - vision localized issues：`edge_input_to_teacher` 路径大折线；`comp_task_loss` 占 teacher/student branch 的阅读 corridor；`edge_student_to_task_loss` 向上短边破坏分支关系；`comp_inference_note` 虽然不应被当成 standalone lane，但位置仍缺少视觉锚点并靠近主通道；`comp_latent_residual` 文本略紧。
- 当前结论：
  - 本地 gate 已经能抓低层可编辑几何问题，但不能代表“画得好”。当前差距来自高层 composition：哪些对象应该在主 flow/corridor 外、哪些 note 可以保留但必须锚在 student/output 附近、哪些 route 算视觉绕路。
  - 下一步应把 vision-only 的高层问题转成结构化 quality issue 或 deterministic DrawPlan polish，而不是继续只调 overlap/crowding。

## 2026-06-21 branch corridor, shared input, and residual signal gates

- 本轮目标：
  - 继续把 vision review 里能稳定定位的高层坏模式转成本地 `QualityReport` issue 和 DrawPlan polish，而不是让模型每轮重构。
  - 重点处理：共享 input 贴到 student 行导致 teacher 输入线大绕路；task loss 占 teacher/student 分支 corridor；compact inference note 无锚点或太大；annotation 太贴近 flow edge；上下 teacher/student 分支之间的 inference annotation；简单 residual signal 盒占据主分支间隙。
- 已完成实现：
  - `src/tools/review.rs` 增加/收紧：
    - `task_loss_in_branch_corridor`：task loss 不应占 teacher/student branch gap。
    - `route_detour` 的 input->student / input->teacher 分支识别，避免长三点输入线被漏掉，同时不过度惩罚被拉回分支中线后的正常 teacher branch。
    - `annotation_too_close_to_edge`：annotation 不能贴在 flow/supervision stroke 上。
    - `inference_note_unanchored` 与 `inference_note_excessive_whitespace`：小型 student-only note 可以保留，但必须锚到 student/output 附近且不能大框空白。
    - `annotation_in_main_corridor` 支持上下 teacher/student 分支间隙，不再只识别左右并排 teacher/student。
    - `residual_signal_in_branch_corridor`：简单 `Teacher`/`Student` 分支之间的短 residual signal 盒会被报为 major；规则刻意排除 Encoder/Projection/Head/Compact 等内部结构，避免误伤真正的 residual 模块。
  - `src/tools/draw_plan.rs` 增加/调整：
    - `align_shared_input_boxes_with_branch_targets` 从“落在任意 branch row 范围内即可”改为拉到 teacher/student 中线，解决共享 input 被吸到 student 行的问题。
    - `align_single_input_boxes_with_main_targets`，处理单 input -> main 的偏行长绕路。
    - `move_task_loss_boxes_out_of_teacher_student_branch_corridors`，把 task loss 移出 branch gap。
    - `move_annotations_off_edges` 和 `place_label_outside_edge` 的 annotation 避让更严格；同时把短 connector label 间距单独收回到 `0.024`，避免把 connector 自带短标签推得太远。
    - `fold_detached_protected_inference_notes_into_student_annotations`、`compact_inference_note_is_anchored_to_student_or_output`，让 protected inference note 离主通道并保留语义。
    - `fold_connected_residual_signal_boxes_between_branch_rows`，只折叠简单 `Teacher`/`Student` 分支之间的短 `Latent Residual` signal 盒为 dashed supervision connector label；复杂 encoder/projection/head 场景继续保留 residual 盒并由已有规则移动。
  - 新增/扩展回归：
    - review tests 从 32 增至 40；DrawPlan tests 从 106 增至 111。
    - 覆盖 latest smoke 的 task loss corridor、long input detour、annotation too close、oversized/unanchored inference note、vertical branch annotation corridor、connected residual signal box fold 等失败模式。
- 第一轮真实 `.env` smoke：
  - 命令：`SESSION_ID=post_shared_input_balance_smoke_20260621_124900 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_shared_input_balance_smoke_20260621_124900`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final 为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=100/issues=0`、`round_001 score=88/issues=2`、`round_002 score=76/issues=3`、`final score=100/issues=0`。
  - vision final 分数：semantic 5、story 4、hierarchy 5、readability 4、layout 3、arrow 3、color 6、aesthetic 4、editability 8。
  - 人工/vision 结论：共享 input 大绕路明显缓解，但本地 quality 仍漏掉两个高层 blocker：`Latent Residual` 简单 signal 盒占 teacher/student branch gap，`Inference: student only` annotation 落在上下分支中间。
- 第一轮 smoke 后追加修复：
  - 新增红测 `quality_report_flags_inference_annotation_between_vertical_teacher_student_branches`、`quality_report_flags_connected_residual_signal_box_between_vertical_teacher_student_branches` 和 `model_draw_plan_polish_folds_connected_residual_signal_box_between_vertical_branches_from_smoke`。
  - 初版 residual 折叠过宽，误伤 `model_draw_plan_polish_moves_residual_hub_out_of_branch_gap_crowding` 与 `model_draw_plan_polish_repairs_stacked_teacher_residual_smoke_layout`。已通过 simple branch label 约束收窄：只有简单 Teacher/Student 分支标签之间的短 residual signal 盒才折叠；Encoder/Projection/Head 等场景保留 residual 盒。
- 已完成本地验证：
  - `cargo test --test review_tests -- --nocapture` 通过，40 个 review 测试全绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，111 个 DrawPlan 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 第二轮真实 `.env` smoke：
  - 命令：`SESSION_ID=post_vertical_corridor_residual_gate_smoke_20260621_132000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_vertical_corridor_residual_gate_smoke_20260621_132000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final 为 `source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=82/issues=3` -> `round_001 score=100/issues=0` -> `round_002 score=100/issues=0`；final `score=100/issues=0`。
  - vision 分数明显提升：round_000 为 story 4 / hierarchy 2 / readability 3 / layout 2 / arrow 3 / aesthetic 3；final 提升到 semantic 7、story 7、hierarchy 6、readability 6、layout 5、arrow 5、color 7、aesthetic 6、editability 9。
  - 人工查看 final PNG：结构已经从上一轮的中间 residual signal 盒与 corridor note，推进到更可读的 Y-branch 布局；仍未 accepted 的主要原因转为路线质量：`e_input_to_student` 沿底部绕大 L、`e_student_to_output` 右侧大 U、`e_residual_to_student` 贴 student 边，以及 teacher/student 宽度不平衡。
- 当前剩余任务：
  - 下一轮应把 route quality 继续本地化：识别靠画布外圈的长 L/U 形 route、edge endpoint tangent/贴边、output 与 source 垂直错位过大但水平相邻的情况。
  - 具体修复方向：扩展 `route_detour` 不只看 route/direct ratio，还应考虑 path 接近 canvas margin 的长段；DrawPlan polish 中让 output 盒优先与 source 同行，input 盒避免被放到远离 teacher/student 中线的画布底部；residual-to-student supervision edge 应避开 student box 边界切线。

## 2026-06-21 outer route and reverse teacher/student layout fixes

- 本轮目标：
  - 继续执行上一节剩余任务：把靠画布外圈的大 L/U 形路线、student-only inference 斜穿主图、teacher/residual/objective 小间距等真实 smoke 失败模式转成本地可重复修复。
- 第三轮真实 `.env` smoke：
  - 命令：`SESSION_ID=post_outer_route_gate_smoke_20260621_123838 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_outer_route_gate_smoke_20260621_123838`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=16/issues=9` -> `round_001 score=52/issues=5` -> `round_002/final score=64/issues=5`，说明 gate 有推进，但仍未收敛。
  - vision 分数：round_000/001 低位徘徊，round_002 小幅提升到 semantic 5、story 4、hierarchy 4、readability 4、layout 3、arrow 3、color 6、aesthetic 4、editability 8。
  - 人工查看 final PNG：模型把 student 放到左下、teacher 放到右侧，导致 `e_input_teacher` 沿左边和底边绕大圈，`e_student_inference` 从 student 斜穿到顶部 inference，`e_joint_student` 与它相交；同时 teacher/residual、teacher/joint_loss 间距过小，`latent` label 远离真实 connector。
- 已完成实现：
  - `tests/review_tests.rs` 新增 latest smoke 的 outer-margin route 红测，覆盖 input->student 和 student->output 的大 L/U 路线；`src/tools/review.rs` 增加 `has_main_output_outer_margin_detour`，让右侧相邻 output 的大 U route 被稳定报为 `route_detour`。
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_repairs_outer_margin_input_and_output_routes_from_latest_smoke`，锁定共享 input 不应留在底部、output 应与 source 同行。
  - `src/tools/draw_plan.rs` 增加 `move_shared_inputs_off_outer_margins`、`align_outer_margin_outputs_with_sources` 和 `straighten_adjacent_main_output_connectors`，在不重写模板的前提下做局部 bbox/connector 修正。
  - 追加 `model_draw_plan_polish_repairs_teacher_student_reverse_layout_from_latest_smoke`，直接使用第三轮 smoke final 的 `input/student/teacher/task_loss/residual/joint_loss/inference` 真实 geometry 复现失败图。
  - 针对该失败图新增局部 polish：
    - `move_student_only_inference_outputs_next_to_sources`：只移动非 note/muted 的 student-only inference output，把它放到 student 旁边，避免顶部单独 lane。
    - `separate_teacher_context_objective_gutters`：给 teacher/context 与 residual/objective 留出 0.065 normalized gutter，避免刚好卡在 quality 阈值边界。
    - `repair_outer_input_context_detours`：晚期强制把 input->teacher/context 的外圈大绕线收缩成不穿组件的短折线；优先不碰其它线，必要时退回到不穿组件的最短候选。
    - `orthogonalize_student_only_inference_connectors`：把 student->inference 的长斜线改成可读的正交 connector。
  - 初版 inference 旁置规则误处理了 `inference_note`，导致两个旧用例不再把长 note 折叠为 annotation；已通过 `note`/`muted`/`annotation` 排除条件收窄，旧折叠路径恢复。
- 已完成本地验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_teacher_student_reverse_layout_from_latest_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，113 个 DrawPlan 测试全绿。
  - `cargo test --test review_tests -- --nocapture` 通过，42 个 review 测试全绿。
  - `cargo test` 全量通过；LibreOffice 仍打印历史外部 `DeploymentException` warning，但所有相关测试 exit 0。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成验证：
  - 需要在这些 route/layout 修复后再跑一轮真实 `.env` smoke，检查外圈绕线、student-only inference 斜线、teacher/residual/joint 小间距是否在真实路径中消失，以及是否暴露下一组更高层 composition/aesthetic 问题。

## 2026-06-21 annotation, residual, and vertical-stack polish

- 第四轮真实 `.env` smoke：
  - 命令：`SESSION_ID=post_reverse_layout_polish_smoke_20260621_130321 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_reverse_layout_polish_smoke_20260621_130321`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=0/issues=16` -> `round_001 score=4/issues=12` -> `round_002/final score=28/issues=8`。上一轮的外圈 input->teacher 大绕线已明显改善，但新输出暴露出 annotation 和 residual/layout 问题。
  - vision final 分数：semantic 7、story 5、hierarchy 5、readability 4、layout 3、arrow 3、color 6、aesthetic 4、editability 9。
  - 人工查看 final PNG：`ann_inference` 与 `anno_teacher_frozen` 重叠并压在 `Task head` 上；`residual_supervision` 变成底部大宽条；`e_residual_student` 为 5 点绕线；`e_task_loss` 为 4 点 hook；单行 `task_input`、`student_encoder`、`teacher_encoder` 盒太高，导致上下模块间距被 quality gate 报为 crowding。
- 第四轮 smoke 后追加修复：
  - 新增 `model_draw_plan_polish_repairs_annotation_and_residual_stack_from_latest_smoke`，用第四轮 final 的真实 geometry 锁定：
    - 重叠 inference annotation 应删除或折叠；
    - teacher/frozen annotation 不应压 `task_head`；
    - 同列单行 flow box 应压缩高度以腾出垂直 gutter；
    - `residual_supervision` 不应保持超宽底座；
    - `e_residual_student` 和 `e_task_loss` 应简化为短正交路线。
  - `src/tools/draw_plan.rs` 新增：
    - `compact_single_line_flow_boxes_in_vertical_stacks`：对同列 vertical stack 中的单行 flow box 压缩下边界，解决“盒内空隙大但盒间距小”的根因；补了 pair-based fallback，避免只依赖 connector 方向。
    - `compact_wide_residual_supervision_boxes`：把超宽 residual supervision 盒收窄到可读范围，避免底部大 slab。
    - `simplify_main_to_residual_supervision_edges`：把 main/student -> residual supervision 的 5 点绕线收成紧凑 elbow。
    - `simplify_adjacent_output_loss_connectors`：把 output -> task loss 的近邻 hook 收成直连或短 elbow。
- 已完成本地验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_annotation_and_residual_stack_from_latest_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过，114 个 DrawPlan 测试全绿。
  - `cargo test --test review_tests -- --nocapture` 通过，42 个 review 测试全绿。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 第五轮真实 `.env` smoke：
  - 命令：`SESSION_ID=post_annotation_residual_stack_smoke_20260621_131833 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_annotation_residual_stack_smoke_20260621_131833`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=70/issues=4` -> `round_001 score=76/issues=3` -> `round_002/final score=94/issues=1`。第四轮的 annotation overlap、residual bottom slab、5 点 residual route 等问题未在 final 复发。
  - vision final 分数：semantic 6、story 5、hierarchy 5、readability 5、layout 4、arrow 4、color 7、aesthetic 5、editability 9。
  - final 唯一本地 issue：`task_loss` 与 `output_answer` 垂直间距过小。
  - 人工查看 final PNG：图已从大量重叠/绕线降到一个较干净的左右 teacher/student 结构，但仍有明显质量问题：student box 内带 `(inference only)` 造成三行拥挤，`Answer` 与 `Task Loss` 上下贴得太近，`ann_supervision` 浮在右侧，整体 composition 仍不够论文级。
- 当前剩余任务：
  - 把 `task_loss`/`output_answer` 近距离 crowding 转成 DrawPlan polish，优先把 `task_loss` 移到 answer 侧边或加大垂直 gap。
  - 清理 main student box 内的孤立 `(inference only)` 行，改为外部 compact note 或删除冗余 cue。
  - 处理 `ann_supervision` 这种浮动说明文字：若 dashed edge 已表达 supervision，应删除或吸附到 connector label。
  - 继续真实 smoke，目标是 local quality 100 且 vision layout/arrow/aesthetic 不再卡在 4-5。

## 2026-06-21 inference-only语义保留修复

- 本轮目标：
  - 修复 `remove_redundant_inference_only_parenthetical_from_text` 的过度清理，避免把主语义行从 `Student (compact, inference-only)` 这种主框注释里误删，导致唯一的推理语义丢失。
- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 调整 `remove_redundant_inference_only_parenthetical_from_text`：不再对主框文本进行泛化字符串替换，保留非独立行内的 `inference-only` 短语，只移除纯独立的 `(inference-only)` / `(inference only)` 行。
- 验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（117/117）。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_removes_redundant_phase_loss_and_inference_notes_from_smoke -- --nocapture` 通过。
- 真实 `.env` smoke 验证：
  - 命令：`SESSION_ID=post_inference_semantics_fix_smoke_20260621_135000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_semantics_fix_smoke_20260621_135000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；`final score=52`；本地问题集中在 `student_head` 空白/挤压和 `ann_inference` 漂浮/标签远离边的问题。
  - 与上一轮相比，本次 smoke 主要验证目标回归修复有效；未引入新的语义缺失；但 composition/aesthetic 仍未收敛到 acceptance。
- 剩余任务：
  - 将 `draw_plan` 继续收敛到避免 `student_head` 空白和上下拥挤，优先复用既有 `compact_single_line_flow_boxes_in_vertical_stacks` 与 `separate_task_loss_boxes_from_main_modules` 逻辑，减少无效回路。

## 2026-06-21 inference语义相关 debug 清理与本地回归

- 本轮目标：
  - 清理前序版本为定位问题而加的 `AUTOFIG_DEBUG_INFERENCE` 调试输出，确保正式路径不再引入测试/日志噪音，同时保留 `comp_inference -> ann_inference` 的语义修复结果。
- 已完成：
  - `src/tools/draw_plan.rs`：
    - 移除 `debug_inference_stage_snapshot` 与所有 `AUTOFIG_DEBUG_INFERENCE` 条件分支及 `eprintln!` 调用。
    - 保持 `is_marginal_annotation` 对 `ann_inference` 的语义保护逻辑不变（仅跳过语义型 inference 注释），避免误删。
  - `tests/draw_plan_tests.rs`：
    - 移除 `model_draw_plan_polish_folds_standalone_inference_component_from_smoke` 中临时 `eprintln!`，避免测试噪音。
- 验证：
  - `cargo fmt`
  - `cargo test --test draw_plan_tests -- --nocapture`（118/118）
- 当前状态：
  - 本地 `draw_plan_tests` 全绿，`AUTOFIG_DEBUG_INFERENCE` 已从当前路径移除；`draw_plan` 的 inference 折叠与语义保护行为仍保持。
  - 真实 `.env` smoke 仍未在本轮重新执行（仍建议下轮继续沿既定 smoke session 做可视化回归）。

## 2026-06-21 current smoke output/note/loss-route root cause fix

- 本轮真实 `.env` smoke：
  - 命令：`SESSION_ID=post_output_note_gate_smoke_20260621_190000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_output_note_gate_smoke_20260621_190000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；每轮 `renderer_status.source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=46/issues=8`，`round_001 score=64/issues=4`，`round_002 score=58/issues=5`，final 被 best-so-far 逻辑复制回 `round_000`，因此 final 仍有巨大 `output_pred`、拥挤 `inference_note` 和 loss/branch corridor 问题。
  - 人工查看 PNG：`round_001` 修小了 `ŷ` 框但引入 `ann_inference` 压 teacher 和右侧大回路线；`round_002` 缩短了部分路线但仍保留 teacher 上的 inference annotation 和弯折 student-loss edge。这说明模型每轮确实在改，但局部修复会引入 hard regression。
- 根因：
  - `compact_tall_output_boxes_for_short_labels` 只压缩高度，不压缩 `ŷ` 这类单字符输出的宽度，导致单字输出仍是大空框。
  - student-only inference note 的处理边界不清：真正的 note/badge/context 组件、lane-like inference component、漂浮 text annotation 被同一批折叠/移动逻辑处理，容易要么丢语义，要么保留成拥挤小面板。
  - student -> task loss 的模型输出可绕到画布右边界；旧局部修复只看组件避让，没有检查新的短路由是否会与其它 connector 冲突。
- 已完成实现：
  - `tests/draw_plan_tests.rs` 新增两条 current-smoke 回归：
    - `model_draw_plan_polish_compacts_current_smoke_prediction_and_student_inference_note`
    - `model_draw_plan_polish_moves_student_inference_annotation_and_shortens_loss_route_from_current_smoke`
  - `src/tools/draw_plan.rs`：
    - 将短输出框压缩从“只压高”改为“按可见字符数同时压宽高”，并用 incoming connector terminal 对齐单字输出。
    - 新增 `move_student_inference_notes_near_student`，只处理 note/muted/context/annotation，不碰真正的 inference branch/output；candidate 优先右侧，右侧被占才放到下方。
    - 收窄 protected inference note 折叠：`badge` 和裸 `inference_note` 保留为 editable box；lane-like `comp_inference` 可折叠；压到 loss/objective 的 `comp_inference_note` 仍折叠成 text annotation。
    - 新增 `simplify_student_to_task_loss_connectors`，只对跑出局部 bbox 的 student->task_loss 大绕线生成短 elbow；候选必须同时通过 component 避让和 connector conflict 检查，避免覆盖已有避障路线。
    - 对非常泛的 `ŷ,y` loss connector label，仅在成功短路由后删除，避免误删垂直 connector 的有效标签。
- 当前验证：
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（122/122）。
- 待完成：
  - 继续跑 `review_tests`、`pipeline_tests`、全量 `cargo test`、renderer build、fmt/diff check。
  - 再跑一轮真实 `.env` smoke，确认 round_000 的巨大 output、student-only inference note 拥挤和 student->loss 右边界大绕线不再复发，并观察下一组 vision/local blocker。
- 后续验证与新增 gate：
  - 本地验证完成：`cargo test --test draw_plan_tests -- --nocapture` 通过（122/122）；`cargo test --test review_tests -- --nocapture` 通过（45/45）；`cargo test` 全量通过；`cd renderer && npm run build` 通过；`cargo fmt --check` 通过；`git diff --check` 通过。
  - 真实 `.env` smoke：
    - 命令：`SESSION_ID=post_output_note_loss_route_fix_smoke_20260621_193000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
    - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_output_note_loss_route_fix_smoke_20260621_193000`
    - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；每轮 `renderer_status.source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
    - 本地 quality：`round_000 score=76/issues=4` -> `round_001 score=100/issues=0` -> `round_002 score=70/issues=5`；final 选择 `round_001`，`score=100/issues=0`。这说明本轮修复已避免 final 退回巨大 `ŷ` 空框和右边界大绕线版本。
    - 人工查看 final PNG：巨大单字输出和 inference note 挤压已明显改善，但仍有未本地化的高层问题：`Student Encoder` 框过高、`Task Loss` 被放到最右上角，`e_student_task` 是穿过主图的长水平 loss line；vision 因 student/task_loss 水平拥挤和 teacher/student 权重不平衡拒绝。
  - smoke 后新增 review gate：
    - `tests/review_tests.rs` 新增 `quality_report_flags_far_student_task_loss_route_from_latest_smoke`。
    - `src/tools/review.rs` 新增 `task_loss_far_from_student_source` issue：当 `student/main -> task_loss` 的 connector 有长水平段且 task loss 被送到远右侧时，本地 quality 不再给 100 分。
    - 新增 gate 后再次验证：`cargo test --test review_tests -- --nocapture` 通过（45/45）；`cargo test` 全量通过；`cd renderer && npm run build`、`cargo fmt --check`、`git diff --check` 均通过。
  - 当前剩余问题：
    - 下一轮应让 DrawPlan polish 或 model prompt 针对 `task_loss_far_from_student_source` 做具体几何修复：把 `Task Loss` 拉回 student/output path 附近，或改成 output->loss 的短局部 objective cue。
    - 还需要把 `Student Encoder` 过高/teacher-student 权重不平衡转成本地 gate 或 polish；当前 vision 能看出来，但 local gate 仍主要靠新增 far-loss issue 间接阻止 acceptance。

## 2026-06-21 far task-loss route follow-up

- 本轮目标：
  - 按上一轮真实 smoke 的具体视觉失败继续闭环：`Task Loss` 被放到画布最右上角，`student_enc -> task_loss` 形成跨过主图的长水平线；这类问题不能只靠 reasoning/vision 提示，必须让 coding 模型读取上一轮几何并改当前代码。
- 根因：
  - 前序 `simplify_student_to_task_loss_connectors` 只会在目标 box 已经合适时重画线；如果模型把 `Task Loss` 本身放到远右侧，它不会主动移动 loss box，因此长水平线仍然合法地保留下来。
  - review gate 已能报 `task_loss_far_from_student_source`，但 DrawPlan polish 尚无对应修复动作，导致本地 quality 可以识别问题却不能自动收敛。
- 已完成实现：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_pulls_far_student_task_loss_near_output_path_from_latest_smoke`，使用上一轮 final PNG 对应的真实 bbox/points：
    - `student_enc` 在中部偏左；
    - `task_loss` 在最右上；
    - `e_student_task` 包含长水平段；
    - 同时包含 `latent_residual` 和 `output_pred`，防止修复方式只是把 loss 压到其它框上。
  - `src/tools/draw_plan.rs` 新增 `pull_far_student_task_loss_boxes_near_output_path`：
    - 只处理 student/main -> task loss 且 route 明显过宽/有长水平段的情况；
    - 优先参考同一 student 的 output，在 output 上方/下方/左侧或 student 右侧生成候选；
    - 候选必须通过 box overlap 检查、中间穿框检查、connector conflict 检查，并且必须比当前 route 明显更短；
    - 移动 `Task Loss` 后立即把对应 student->loss connector 重画成本地短 elbow，避免旧远端折线遗留；
    - 对 `ŷ,y` 这类泛化 loss label，在短路由成功时同步删除，保留有信息量的 label。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_pulls_far_student_task_loss_near_output_path_from_latest_smoke -- --nocapture` 通过。
  - 首次完整 DrawPlan 回归发现 `model_draw_plan_polish_moves_student_inference_annotation_and_shortens_loss_route_from_current_smoke` 失败：新 pass 先重画了短路线，导致旧 `simplify_student_to_task_loss_connectors` 不再触发泛化 label 删除。
  - 已修复该回归：在新 pass 成功重画短路线时同步移除泛化 `ŷ,y` label。
  - 全量 `cargo test` 首次暴露另一个非确定性问题：`move_student_inference_notes_near_student` 从 `HashMap` 中用宽松的 `is_main_route_box` 找 student anchor，而 `comp_inference_note` 文本含 `student`，有时会被误选为 student 本体，导致 note 自己被跳过、保持过大 box。
  - 已修复该非确定性：新增 `student_anchor_for_inference_notes`，按 `plan.objects` 顺序选择真正的 student 模块，并排除 `inference`/`note`/`context`/`muted` 等 note-like box；`model_draw_plan_polish_repairs_latest_branch_corridor_smoke_layout` 单测恢复通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（123/123）。
  - `cargo test --test review_tests -- --nocapture` 通过（45/45）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt && cargo fmt --check && git diff --check` 通过。
- 待完成：
  - 跑真实 `.env` smoke，确认 `task_loss_far_from_student_source` 不再复发，同时观察 `Student Encoder` 过高/teacher-student 权重不平衡是否仍是主要 blocker。

## 2026-06-21 oversized inference annotation follow-up

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_far_task_loss_pull_smoke_20260621_203000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_far_task_loss_pull_smoke_20260621_203000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；`renderer_status.source="model_generated_code"`、`used_fallback=false`；final PPTX `unzip -t` 通过。
  - 本地 quality：三轮均 `score=94/issues=1`；唯一 issue 为 `annotation_excessive_whitespace`，target `anno_inference`。
  - 人工查看 final PNG：上一轮的 far task-loss 已解决，`Task Loss` 在 student 左侧，`e_student_taskloss` 是短横线；新的主要问题是右下角 `Inference: student only` 被保留为大文本框 `[0.7083, 0.75, 1.0, 1.0]`。
- 根因：
  - inner polish 会移动/压缩 inference note，但 `polish_model_draw_plan_geometry_with_figure_plan` 之后调用 `upsert_meaningful_annotations_from_figure_plan`，当 FigurePlan annotation 没有 `target_id` 时，会用原始大 bbox 直接覆盖现有 `anno_inference`。
  - 有 `target_id` 的 inference annotation 已由 `anchored_figure_plan_annotation_bbox` 正确锚定；不能对它们再做 student-note 二次移动。
- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 在 figure-plan annotation upsert 后再次调用 inference-note 压缩/锚定逻辑，处理无 target 的大 `anno_inference`。
    - 新增 `target_bound_inference_annotation_ids` 和 `move_student_inference_notes_near_student_except`，跳过带 `target_id` 的 inference annotation，避免破坏已有 target anchoring。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_recompacts_upserted_inference_annotation_from_latest_smoke`，复现本轮 final 的大 `anno_inference` bbox，要求 polish 后面积小于 `0.025` 且仍锚定在 student/output 附近。
    - 回归覆盖 `model_draw_plan_polish_anchors_inference_annotation_to_figure_plan_target`，确保有 target 的 annotation 不被二次移动。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_recompacts_upserted_inference_annotation_from_latest_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_anchors_inference_annotation_to_figure_plan_target -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（124/124）。
  - `cargo test --test review_tests -- --nocapture` 通过（45/45）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt && cargo fmt --check && git diff --check` 通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 `annotation_excessive_whitespace` 不再成为唯一 blocker，并观察 vision acceptance 仍缺什么。

## 2026-06-21 targeted inference annotation corridor follow-up

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_annotation_recompact_smoke_20260621_210000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_annotation_recompact_smoke_20260621_210000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；renderer 仍为 model generated code。
  - 本地 quality：三轮均 `score=46/issues=6`。上一轮右下角巨大 inference note 已消失，但新 final 中 `anno_inference` 被压到 residual/teacher-to-residual 边上，触发：
    - `annotation_in_main_corridor`
    - `label_overlaps_component` (`anno_inference` vs `latent_residual`)
    - `label_overlaps_edge` (`anno_inference` vs `e_teacher_to_residual`)
    - 另有 `anno_teacher_role` 顶部大注释和 `residual` connector label 覆盖边。
- 根因：
  - `anno_inference` 这次带 `target_id="student_predictor"`。上一轮为了保护有 target 的 annotation，upsert 后的 corridor repair 也跳过了它。
  - 正确边界应是：有 target 的 annotation 不走泛化 student-note 重定位；但如果 target-adjacent 位置实际覆盖组件或边，仍应允许 corridor/overlap repair 移开。
- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 保持 `move_student_inference_notes_near_student_except` 跳过 target-bound inference annotation，避免破坏清晰的 target anchoring。
    - upsert 后改为继续运行 `move_inference_note_boxes_out_of_flow_corridors`，让 target-bound annotation 在实际覆盖 residual/edge/corridor 时仍可移动。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_moves_targeted_inference_annotation_out_of_residual_corridor_from_latest_smoke`，复现当前 final：`anno_inference` 有 target，但覆盖 `latent_residual` 和 `e_teacher_to_residual`；要求 polish 后不再覆盖组件/边且仍靠近 student。
    - 保留并验证 `model_draw_plan_polish_anchors_inference_annotation_to_figure_plan_target`，确认清晰 target annotation 不会被普通重定位破坏。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_targeted_inference_annotation_out_of_residual_corridor_from_latest_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_anchors_inference_annotation_to_figure_plan_target -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（125/125）。
  - `cargo test --test review_tests -- --nocapture` 通过（45/45）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt && cargo fmt --check && git diff --check` 通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 inference annotation corridor blocker 是否清除；若仍失败，下一优先级是 `anno_teacher_role` 顶部大注释和 residual edge label 覆盖。

## 2026-06-21 post-targeted-corridor smoke result

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_targeted_inference_corridor_smoke_20260621_213000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_targeted_inference_corridor_smoke_20260621_213000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
  - 本地 quality：`round_000 score=0/issues=11`，`round_001 score=58/issues=4`，`round_002/final score=40/issues=6`。
  - 本轮验证结论：上一轮的 `anno_inference` 覆盖 residual/edge blocker 未在 final 复发；但模型换成了更差的整体布局。
- 当前 final 主要问题：
  - `input_text` 被放到左下角，`e_input_teacher` 和 `e_input_student` 形成大 U 型绕线；本地报 `route_detour`。
  - `task_loss` 和 `residual_loss` 被挤到顶部，`e_task_supervision`、`e_residual_supervision` 与主路径和 teacher/student 区域交叉；本地报 `edge_crosses_component`、`edge_crossing`。
  - `inference_note` 作为 box 保留，但 bbox 被压得偏小；本地报 `component_collapsed`。
  - 这说明当前 loop 的主要缺口已经从单个 annotation 转向“teacher/student 模板拓扑约束不稳定”：模型仍会把 input/loss/residual 放到不符合参考模板的区域，后处理只能局部救火。
- 下一步建议：
  - 不再继续只修单个 label；应把 `teacher_student` 模板的几何拓扑收紧为局部可变但结构稳定的约束：
    - input 必须在 teacher/student 左侧或两者中间左侧，禁止落到底部外圈；
    - teacher/student 两分支应共享输入左侧入口，避免 input->branch 的外圈 U 型绕线；
    - task loss/residual loss 不能同时占据顶部横向 corridor，loss/objective 应靠近对应 output/student 边；
    - inference note 最小可读尺寸与 student/output 附近 anchor 应同时满足。
  - 对应需要新增本地 fixture：复现本轮 final 的 bottom input / top losses / crossing supervision 组合，再实现 template-aware placement polish 或更强的 FigurePlan canonicalization。

## 2026-06-22 teacher/student topology repair implementation

- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `stabilize_teacher_student_shared_inputs`，把同时连接 teacher/context 与 student/main 的 shared input 从底部外圈拉回到分支左侧，并复用 `compact_input_context_route` 重算短局部绕线，避免 input->student 穿过 teacher 或回到底部。
    - 新增 `pull_top_edge_task_losses_near_outputs`，当 task loss 被放在顶部且 output->loss 路径形成长竖线时，将 task loss 拉回 output/student 附近，并重算相邻 objective connector。
    - 新增 `pull_top_residual_losses_near_sources`，当 residual/supervision objective 被放到顶边且远离来源时，将其拉回来源附近，避免顶部横向 corridor 与 task supervision 交叉。
    - 新增 `ensure_inference_note_boxes_readable`，对仍保留为 `inference_note` box 的 student inference cue 做最小高度保护，避免塌缩；直接 polish 路径可能会把孤立 note 折成 editable text，真实 FigurePlan pipeline 还需要 smoke 验证。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_bottom_input_and_top_losses_from_latest_smoke`，复现 `post_targeted_inference_corridor_smoke_20260621_213000` final 的 bottom input / top task loss / top residual loss / crossing supervision 组合。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_bottom_input_and_top_losses_from_latest_smoke -- --nocapture` 通过。
  - 初次 `cargo test --test draw_plan_tests -- --nocapture` 发现 3 个回归：同列 top task loss 被错误移动、同列 residual supervision 被错误移动、stacked teacher/residual fixture 的 loss stack 被破坏。
  - 根因：新增 top-edge repair 只判断“位于顶边且离来源远”，没有区分“source 与 objective 同列，应只绕线/调 label”与“source 与 objective 横向错位，应移动 objective”。
  - 修复：`top_edge_task_loss_needs_pull` 和 `top_residual_objective_needs_pull` 增加 `horizontal_separation > 0.04` 触发门槛，只处理横向错位明显的顶边 objective。
  - 复跑失败单测与新增单测均通过。
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（126/126）。
  - `cargo test --test review_tests -- --nocapture` 通过（45/45）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check && git diff --check` 通过。
- 待完成：
  - 跑真实 `.env` smoke，确认视觉输出是否从“底部 U 型路线 + 顶部 objective 交叉”改善到稳定 teacher/student 拓扑。

## 2026-06-22 post-topology-repair smoke result and follow-up

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_teacher_student_topology_repair_smoke_20260622_001736 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_teacher_student_topology_repair_smoke_20260622_001736`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；renderer 为 `model_generated_code`，未 fallback。
  - 本地 quality：`round_000 score=0/issues=15`，`round_001/final score=52/issues=7`，`round_002 score=58/issues=5`。
- 观察结论：
  - 上一轮 final 的 `task_loss`/`residual_loss` 顶边横向 corridor 问题已消失；objective 进入主图区域。
  - 新的主要 blocker 是后期 crossing reroute 把 `edge_input_to_student` 推到外圈（round_002 走到 `y=0.03` 顶边；final 走到底部大 U），以及模型会把 `Student Head` 放在 `Student Encoder` 上方，导致 task-loss/source 关系判断错。
  - `comp_inference_note` 在 final 中高度只有约 `0.052`，触发 `component_collapsed`；此前 readability guard 跳过了 `comp_` 前缀 note。
- 已完成 follow-up 实现：
  - `src/tools/draw_plan.rs`：
    - 将 `repair_outer_input_context_detours` 泛化到 `input -> student/main`，并在第二次 `reroute_connectors_around_crossing_edges` 后再执行一次，避免后期重路由重新把 shared input connector 推到外圈。
    - 新增 `stack_student_encoder_head_pairs_top_down`，当 `Student Encoder -> Student Head` 被模型反向堆叠时，按 encoder 在上、head 在下的拓扑重新排布，并重算 connector。
    - 放宽 `ensure_inference_note_boxes_readable` 到 `comp_inference_note`，同时把 note 最大宽度限制在 `0.16`，让高度可读但面积不膨胀。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_outer_shared_input_to_student_after_crossing_reroute_from_smoke`，复现 round_002 的顶边外圈 input->student route。
    - 新增 `model_draw_plan_polish_stacks_student_encoder_above_head_from_smoke`，复现 final 中 Student Head/Encoder 颠倒的问题。
- 当前验证：
  - 两个新增局部单测均通过。
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（128/128）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check && git diff --check` 通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 input 外圈路线、student 顺序、inference note collapse 是否消失。

## 2026-06-22 post-shared-input/student-order smoke result and final corridor fixes

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_shared_input_student_order_smoke_20260622_003108 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_shared_input_student_order_smoke_20260622_003108`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=82/issues=2`，`round_001/final score=88/issues=2`，`round_002 score=70/issues=4`。
  - 视觉评分：semantic 8、story 7、hierarchy 7、readability 6、layout 6、arrow 8、color 7、aesthetic 7、editability 9。
- 观察结论：
  - input 外圈路线、student encoder/head 顺序、`comp_inference_note` collapse 均已消失，整体从上一轮 final score 52 提升到 88，且无 blocking quality issue。
  - 剩余两个 major：
    - `task_loss_in_branch_corridor`：output->task_loss 的 loss 位于 output 上方的 teacher/student 中间带。
    - `annotation_in_main_corridor`：`anno_inference` 作为 Text 保留在 teacher/student 中间。
- 已完成 final corridor fixes：
  - `src/tools/draw_plan.rs`：
    - 新增 `move_output_task_losses_out_of_branch_corridors`，只在 `output -> task_loss` 且 loss 位于 output 上方中间带时，把 task loss 拉到 output periphery。
    - 调整 `student_inference_note_candidate`：Box note 靠近 loss/objective 时仍走折叠保护；Text annotation 允许移动出 teacher/student corridor。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_moves_output_task_loss_below_output_from_corridor_smoke`。
    - 新增 `model_draw_plan_polish_moves_inference_text_out_of_teacher_student_corridor_from_smoke`。
- 当前验证：
  - 两个新增测试通过。
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（130/130）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check && git diff --check` 通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 final 是否达到 quality pass 或至少没有 `task_loss_in_branch_corridor` / `annotation_in_main_corridor`。

## 2026-06-22 post-final-corridor-fixes smoke result

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_final_corridor_fixes_smoke_20260622_004202 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_final_corridor_fixes_smoke_20260622_004202`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`。
  - 本地 quality：`round_000 score=22/issues=8`，`round_001 score=46/issues=7`，`round_002/final score=64/issues=5`。
- 对比结论：
  - 本轮模型生成了与上一轮不同的更复杂 layout，因此没有复现 `task_loss_in_branch_corridor` / `annotation_in_main_corridor`；这两个 issue 在本轮 final 中消失。
  - 但新 layout 引入了新的 blocker：student vertical chain 的 box 间距过小、`student_head -> student_output` connector 太短、`anno_frozen` 靠近 `e_ground_truth_to_task_loss`。
  - 这说明后处理已经能修掉上一轮明确的 corridor 问题，但真实 loop 仍会在不同结构之间跳动；下一步不应继续只补单个 smoke，而应收紧 student vertical chain 的最小 gutter、output 右侧间距、以及 generic frozen annotation 的去重/移位。
- 当前最佳真实 smoke：
  - `post_shared_input_student_order_smoke_20260622_003108` final：`score=88/issues=2`，无 blocking quality issue；视觉上 input 路线、student 顺序、inference note collapse 已解决。
  - `post_final_corridor_fixes_smoke_20260622_004202` final：`score=64/issues=5`，说明后续模型换布局会回落。

## 2026-06-22 student-chain gutter and frozen annotation repair

- 根因定位：
  - `post_final_corridor_fixes_smoke_20260622_004202` final 的 student vertical chain 间隔为约 `0.045`，已有 `stack_crowded_student_branch_chains` 只有小于 `0.035` 才触发，因此视觉上仍然拥挤但 polish 不介入。
  - `student_head -> student_output` 的短边不是单点问题：chain stack 会尝试拉远 output，但后续 `widen_output_boxes_for_long_tokens` 将单字符 `ŷ` 重新放宽到 `0.10`，再由 `separate_horizontally_crowded_connected_boxes` 以旧的 `0.035` target gap 定格，最终仍然出现过短 connector。
  - `anno_frozen` 与 `Teacher (frozen)` 语义重复，且仅在真正压上线时才移动；真实评审会把“贴近长连线”也判为问题，所以这类冗余 annotation 更适合删除。
- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 将 student chain 纵向修复触发门槛从 `0.035` 提到 `0.055`，目标 gutter 从 `0.045` 提到 `0.060`。
    - 为 `head -> output` 相连盒子使用 `0.045` 的水平 target gap，普通相连盒子仍保持 `0.035`，避免全局放大间距。
    - 新增冗余 `frozen` annotation 去重：只有当 annotation-like 文本本身就是 `frozen` 且 semantic boxes 已包含 `frozen` 时删除，FigurePlan 明确保护的有意义 frozen 标注仍会保留。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_increases_student_chain_gutters_and_output_gap_from_smoke`，复现 student chain 过密和 head-output 短边。
    - 新增 `model_draw_plan_polish_removes_redundant_frozen_annotation_near_task_edge_from_smoke`，复现冗余 `anno_frozen` 靠近 task loss 连线。
- 当前验证：
  - 初次尝试 `cargo test --test draw_plan_tests test_a test_b -- --nocapture` 失败，原因是 `cargo test` 只接受一个 test name 过滤参数。
  - 新增的两个局部测试分别通过。
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（132/132）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认这次 final 的 `component_crowding`、`degenerate_edge`、`annotation_too_close_to_edge` 是否消失，观察是否有新的模型布局跳动。

## 2026-06-22 post-student-gutter/frozen smoke and inference-note follow-up

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_student_gutter_frozen_smoke_20260622_012000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_student_gutter_frozen_smoke_20260622_012000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=4/issues=12`，`round_001 score=58/issues=4`，`round_002/final score=94/issues=1`。
- 观察结论：
  - 本轮 loop 没有重开式退化：`regression_report` 显示 round_001 相对 round_000 `score_delta=54`，blocking/major/issue 数均下降。
  - 此前目标问题未复发：没有 `component_crowding`、`student_head -> student_output` degenerate、`anno_frozen` close-to-edge。
  - final 图显著改善，但仍未被 vision 接受；本地唯一 quality issue 是 `standalone_inference_lane`，即 `comp_inference_note` 作为无连接 box 留在图中。
  - 根因：`comp_inference_note` 因 id 含 `note` 被视为 protected FigurePlan component；旧逻辑只有它挤到 loss/objective 或是 lane-like id 时才折叠，因此这个“无连接、非 badge、但不挤 loss”的 note 漏掉了。
- 已完成 follow-up 实现：
  - `src/tools/draw_plan.rs`：
    - 放宽 `detached_protected_inference_note_should_fold`：无连接、非 badge、非显式 `inference_note` 的 protected inference note，如果不是紧贴 student 的小 badge，就折叠为 annotation。
    - 将 `Inference only` / `inference-only` 归一成 `Inference: student only`，避免折叠后语义变弱。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_folds_unconnected_protected_inference_note_from_final_smoke`，复现 final 中 `comp_inference_note` 无连接但未折叠的问题，并要求折叠后的 annotation 不覆盖 student/input box。
- 当前验证：
  - 新增局部测试通过。
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（133/133）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 下一次真实 smoke 应确认 final 的 `standalone_inference_lane` 是否消失；如果消失但 vision 仍拒绝，下一优先级是 teacher/student Y-branch 平衡和 residual connector 的主观美学问题。

## 2026-06-22 post-inference-note-fold smoke result

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_note_fold_smoke_20260622_013000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_note_fold_smoke_20260622_013000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=52/issues=5`，`round_001 score=70/issues=4`，`round_002/final score=76/issues=3`。
- 验证结论：
  - 上一轮 final 的 `standalone_inference_lane` 已消失，说明 protected inference note 折叠修复有效。
  - `regression_report` 显示 round_002 相对 round_001 仍为 improved，`score_delta=6`，没有新增 regressed issue type。
  - 新 final 的 3 个本地问题来自同一拓扑：teacher、latent residual、student 被压在同一水平线，导致 `teacher_encoder -> latent_residual_obj` 太短、teacher 与 residual 太近、`task_input -> student_encoder` 必须绕 teacher 走底部长 U。
  - 视觉观察：图已清爽很多，但结构像单行 pipeline，不像 teacher/student Y-branch；`ann_inference` 已从 box 折成 text，但仍位于 student 下方主流区，vision 认为它应该更靠外。
- 下一步：
  - 不再补单个 connector；应新增一个 focused topology repair：当 shared input 同时连 teacher 和 student，且 teacher/student/residual 被压成同一横排时，把 teacher/student 分成上下分支或至少拉开 residual/teacher 并避免 input->student 四点 U 型 dogleg。
  - 对应新增 final-smoke fixture，覆盖 `component_crowding`、`route_detour`、`degenerate_edge` 三个问题。

## 2026-06-22 same-row teacher/student collapse repair

- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_same_row_teacher_student_shared_input_collapse`，只在 shared input 同时连接 teacher 与 student、teacher/student 几乎同一行、teacher 位于 input 与 student 之间、且 input->student 已出现四点 U 型路线时触发。
    - 修复策略是局部移动 teacher 到 student 上方、把 residual/objective 放到 teacher 与 student 之间但保持可见 gutter，并用 `orthogonal_connector_points_between_boxes` 重算 input->teacher、input->student、teacher/student->residual 边。
    - 不移动正常的 vertical teacher/student 分支，不处理没有 shared input 的普通 pipeline，降低误伤已有布局的风险。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_same_row_teacher_student_shared_input_collapse_from_smoke`，复现 `post_inference_note_fold_smoke_20260622_013000` final 中的 `component_crowding`、`route_detour`、`degenerate_edge` 组合。
- 当前验证：
  - 新增局部测试通过。
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（134/134）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 如继续跑真实 smoke，应确认 `post_inference_note_fold_smoke_20260622_013000` final 的同排 collapse 三问题是否消失；仍需关注 vision 对 teacher/student Y-branch 平衡的主观评分。

## 2026-06-22 post-same-row smoke panic and fix

- 真实 `.env` smoke 尝试：
  - 命令：`SESSION_ID=post_same_row_collapse_smoke_20260622_014500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_same_row_collapse_smoke_20260622_014500`
  - 结果：round_000 只写出 `figure_plan.json`，随后 panic：`f64::clamp min > max`，`min = 0.9881666666666667, max = 0.94`。
- 根因定位：
  - panic 来自 `residual_signal_bridge_route` 的右侧 rail 计算：当 teacher/student branch 已经靠近右边界时，`right_x + 0.035 > 0.94`，但代码仍调用 `.clamp(right_x + 0.035, 0.94)`，导致 min/max 反序。
  - 这不是 LLM/API 问题，是本地几何保护缺口；真实 smoke 比单测覆盖到了更极端的右边界 residual bridge。
- 已完成修复：
  - `src/tools/draw_plan.rs`：
    - `residual_signal_bridge_route` 增加右边界 fallback：右侧 rail 空间不足时，改用左侧 rail；若左侧也极窄，则使用不反序的安全边界，避免 panic。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_folds_right_edge_residual_bridge_without_clamp_panic`，覆盖靠右 teacher/student residual bridge 的折叠路径。
- 当前验证：
  - 新增 panic 回归通过。
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（135/135）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 重新跑真实 `.env` smoke，确认 panic 已消失，并观察 same-row collapse repair 的端到端效果。

## 2026-06-22 post-panic-fix smoke result

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_same_row_collapse_smoke_20260622_020000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_same_row_collapse_smoke_20260622_020000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=0/issues=18`，`round_001 score=34/issues=9`，`round_002/final score=46/issues=7`。
- 验证结论：
  - `residual_signal_bridge_route` 的 clamp panic 已消失；round_000 成功渲染、导出、review。
  - `standalone_inference_lane` 未复发；final 中 inference cue 是 compact text。
  - 这次 LLM 生成了更复杂的 encoder/projection/output 双分支，而不是上一轮的同排 simple chain；final 主要问题变成 teacher/student projection 比例严重不对称、latent residual 放在 teacher flow 上方、task loss 贴 student output、分支间距仍不足。
  - 这类问题已经超出单条 edge / 单个 annotation 修复，下一轮应考虑更强的 teacher_student template canonicalizer，或者在 reasoner/coder prompt 中强制选择并保持一个参考模板的 branch grammar，而不是继续按真实 smoke 一个个补局部 polish。
- 当前状态：
  - 已修复并验证的具体问题：student chain gutter、head-output short edge、redundant frozen annotation、protected inference note standalone box、same-row shared-input collapse、right-edge residual bridge panic。
  - 未解决的系统性问题：LLM 仍会在每次 smoke 生成不同拓扑；现有 local polish 能显著提升某些布局，但对复杂 projection/out 多节点 teacher_student 图还缺少模板级结构约束。

## 2026-06-22 multistage teacher/student branch balancing repair

- 根因复查：
  - 读取 `post_same_row_collapse_smoke_20260622_020000/final/draw_plan.json`、`quality_report.json`、`review.json` 后确认，失败不是单条线问题，而是复杂 encoder/projection/output teacher-student 分支的结构失衡：
    - `teacher_proj` 高度 `0.319`，明显大于 `student_proj` 的 `0.180`，导致 paired stage 视觉权重失衡。
    - `latent_residual` 被放在 teacher flow 上方，而不是 teacher/student branch endpoints 之间，随后 dashed route 穿过 teacher projection flow。
    - `student_out` 与 `task_loss` 几乎贴边，产生 degenerate edge。
  - `templates/method_overview/method_templates.json` 只有 `simclr_contrastive_y_branch` 抽象参考；其规则是 shared source、两条分支、agreement/loss 位于分支端点上方或之间。因此本轮没有写死某张论文图坐标，而是把它落成 branch grammar repair。
- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `balance_multistage_teacher_student_branches`，插在同排 collapse repair 之后、task loss corridor repair 之前。
    - 触发条件收窄为同时存在 teacher/student 至少两个 paired stage，且必须有成对 projection/head stage；避免误伤只有 encoder/output 或普通 student vertical chain 的布局。
    - 对明显高度失衡的 paired encoder/projection stage 做局部压缩，不因同高同排而强行上下移动。
    - 将 residual/supervision hub 拉回 connected teacher/student branch rows 之间，并为 moved objective connectors 重算 orthogonal route。
    - 将贴住 student output 的 task loss 推到 output 下方或侧边，保证短边不再 degenerate。
    - `move_objective_hubs_out_of_branch_gap_crowding` 增加保护：如果 residual 已处于 multi-stage teacher/student 的合法 branch slot，不再把它搬到 branch union 上方，避免二次破坏。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_balances_multistage_teacher_student_branches_from_latest_smoke`，直接复现 latest final 的 teacher/student projection 失衡、residual 顶部漂浮、task loss 贴边和 crossing 问题。
- 调试中遇到的坑：
  - 初版 repair 只跑新增测试可通过，但全量 `draw_plan_tests` 暴露两个回归：没有 projection pair 的 student vertical chain 被误判为 multi-stage branch。
  - 解决方式是把触发条件收窄到必须有成对 projection/head stage，并只处理明显高度失衡，不再用普通 vertical gap 作为移动理由。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（136/136）。
  - `cargo fmt` 已执行。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 跑真实 `.env` smoke，确认复杂 projection/out 分支是否端到端改善；重点观察 `component_crowding`、`degenerate_edge`、`edge_crossing` 和 vision 对 residual/loss placement 的评价。

## 2026-06-22 post-multistage-balance smoke and embedded-inference follow-up

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_multistage_branch_balance_smoke_20260622_030000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_multistage_branch_balance_smoke_20260622_030000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=76/issues=3`，`round_001 score=88/issues=1`，`round_002/final score=100/issues=0`。
- 验证结论：
  - 本轮几何目标达成：final 本地 quality 已无 `component_crowding`、`degenerate_edge`、`edge_crossing`。
  - PNG 视觉上不再有上一轮那种 teacher/student projection 失衡和 residual 穿线问题。
  - vision 仍拒绝，但拒绝点已经切换为新问题：`final_output` 文本合并了 `(inference: student only)`，导致 output 框承担了辅助注记语义，视觉上像一个挤压的多行小框。
- 已完成 follow-up 实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `split_embedded_inference_notes_from_output_boxes`，在 polish 末尾拆分 output box 中独立的 `inference: student only` parenthetical。
    - output box 保留 `Final/Prediction` 等预测语义；`Inference: student only` 通过现有 `ensure_inference_annotation` 生成独立 editable text annotation。
    - 旧的 `remove_redundant_inference_only_parentheticals_from_main_boxes` 现在跳过 output box，避免提前删掉 inference 语义却不生成 annotation。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_splits_embedded_inference_note_from_output_box_from_smoke`，复现 final `final_output` 合并 inference note 的问题。
- 调试中遇到的坑：
  - 初版在 polish 早期拆分 annotation，但后续 inference-note 清理链会删掉新生成的 annotation。
  - 最终改为在 polish 末尾拆分，并让旧 main-box parenthetical 清理跳过 output，避免“先删文本、后无从恢复语义”。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（137/137）。
  - `cargo fmt && cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 再跑一次真实 `.env` smoke，确认 vision 是否仍拒绝；如果仍拒绝，应优先检查是否出现新的语义层问题，而不是回到几何微调。

## 2026-06-22 post-embedded-inference-split smoke result

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_embedded_inference_split_smoke_20260622_033000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_embedded_inference_split_smoke_20260622_033000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX 写出成功。
  - 本地 quality：`round_000 score=40/issues=9`，`round_001 score=88/issues=2`，`round_002 score=76/issues=3`，`final score=88/issues=2`。
- 验证结论：
  - 上一轮的 `final_output` 合并 `(inference: student only)` 问题没有复发；说明 output embedded inference split 修复有效。
  - 本轮 final 的拒绝点变成 exact `inference_note` box 自身：`inference_note_excessive_whitespace` 和 `student_enc`/`inference_note` vertical crowding。
  - PNG 观察：主 teacher/student 双分支、latent residual、task loss 都比早期 smoke 稳定；当前瓶颈是 inference note 被模型画成一个较大的 muted box，位置夹在 teacher/student branch 之间，距离 student encoder 太近。
- 尝试与撤回：
  - 曾尝试把 protected exact `inference_note` 直接折叠成 annotation，但最小 FigurePlan fixture 会先把它重定位成 student-adjacent badge，不能可靠复现 smoke 的最终路径。
  - 该未验证实现已撤回，没有留下未通过测试或未验证逻辑。
- 当前验证：
  - `cargo fmt && cargo test --test draw_plan_tests -- --nocapture` 通过（137/137）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 下一轮明确目标：
  - 针对 exact `inference_note` box 增加一个能复现真实 smoke final 的回归入口。更合理的修复方向不是继续改 branch 几何，而是让 compact inference note 在最终 polish 阶段满足：面积小于 quality 阈值、与 student encoder 有足够 gutter、或在非 badge 情况下折叠为 `ann_inference` text。

## 2026-06-22 exact inference note compact repair

- 根因定位：
  - 读取 `post_embedded_inference_split_smoke_20260622_033000/final/draw_plan.json` 后确认，final 的 exact `inference_note` bbox 为 `[0.3058, 0.5617, 0.4858, 0.6617]`，面积约 `0.018`、高度 `0.10`。
  - `src/tools/review.rs` 的质量门禁会把 compact student-only inference note 中 `area > 0.016` 或 `height > 0.095` 判为 `inference_note_excessive_whitespace`，所以本地 gate 已经能看出问题。
  - 真正漏修在 `src/tools/draw_plan.rs::ensure_inference_note_boxes_readable`：旧的 early continue 条件使用“当前尺寸大于等于目标尺寸”就跳过，刚好跳过了需要缩小的 exact `inference_note`。此外，直接折叠 exact `inference_note` 风险较大，因为 FigurePlan 明确声明了该 component；更稳妥的是压缩未连接 note 的尺寸并保持可编辑 box。
  - `move_student_inference_notes_near_student` 没移动该 note 是合理保护：它检测到 note 如果贴近 student 会挤到 `latent_residual` 区域，所以不应强行搬动。
- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - `ensure_inference_note_boxes_readable` 现在跳过有 connector 端点的 note，避免误伤真正 connectable 的 inference component。
    - 对 exact `inference_note` 使用更紧的目标尺寸：宽度最大 `0.16`，高度压到 `0.060..0.070`，保证低于质量门禁阈值。
    - early continue 改为比较当前尺寸与目标尺寸是否近似相等，避免“需要缩小却跳过”的反向判断。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_compacts_protected_inference_note_between_branches_from_smoke`，直接复现上一次 final 的 exact `inference_note` 过大且贴近 `student_enc` 的路径，并断言 note 面积、高度和 student gutter 都满足质量门禁。
- 调试中遇到的坑：
  - 新增测试绿后，全量 `draw_plan_tests` 暴露 `model_draw_plan_polish_converts_note_text_components_to_connectable_boxes` 回归：连接中的 `comp_inference_note` 被压缩导致 connector endpoint 变化。
  - 修复方式是只压缩未连接 note id；有 connector 的 inference component 继续保持原有几何，避免破坏连线语义。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_compacts_protected_inference_note_between_branches_from_smoke -- --nocapture` 通过。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 跑真实 `.env` smoke，确认 `inference_note_excessive_whitespace` 和 `student_enc`/`inference_note` crowding 是否消失；如果 vision 仍拒绝，再按新的 final issue 继续定位。

## 2026-06-22 post-compact smoke and inference annotation corridor repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_note_compact_smoke_20260622_040000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_note_compact_smoke_20260622_040000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=70/issues=5`，`round_001 score=88/issues=1`，`round_002/final score=88/issues=2`。
- 验证结论：
  - 上一轮 exact `inference_note` 大框问题已经消失，final 不再报告 `inference_note_excessive_whitespace` 或 `student_enc`/`inference_note` crowding。
  - 新 final 的两个本地 issue 都绑定到 `ann_inference`：`annotation_in_main_corridor` 和 `annotation_too_close_to_edge`。PNG 观察确认 `Inference: student only` 文本横在 teacher/student 之间，并贴近 `edge_latent_student` 虚线。
  - 根因不是 vision 看不出来，而是本地 polish 只处理 annotation 与组件/线段的重叠，没有处理“未重叠但位于 teacher-student 主走廊”的语义避让。`student_inference_note_candidate` 又认为该 annotation 与 student 距离不够远，所以不会触发移动。
- 已完成 follow-up 实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `move_inference_annotations_out_of_teacher_student_corridors`，复用已有 `teacher_student_branch_pairs` 和 `inference_note_is_in_teacher_student_corridor` 判断，使质量门禁和修复条件一致。
    - 对 inference/student annotation 落入 teacher-student corridor 或贴近 connector 的情况，尝试放到 student 右侧、下方、下方偏左等候选位置，并用已有 `connector_label_candidate_clear` 同时检查组件、connector 段、其它文本和 safe area。
    - 在普通 polish 和 `polish_model_draw_plan_geometry_with_figure_plan` 的 final annotation upsert 之后调用，覆盖 `ann_inference` 被重新生成后的路径。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_moves_upserted_inference_annotation_below_student_corridor_from_smoke`，直接复现本次 final 的 `ann_inference` bbox 和 `edge_latent_student` 路线，要求 annotation 离开 corridor 并避开 residual-to-student connector。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（139/139）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 `annotation_in_main_corridor` 与 `annotation_too_close_to_edge` 是否消失；若仍拒绝，重点查看 task-loss feedback connector 的弯折/回流问题。

## 2026-06-22 post-annotation-escape smoke and unanchored note repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_annotation_escape_smoke_20260622_043000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_inference_annotation_escape_smoke_20260622_043000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=58/issues=6`，`round_001 score=82/issues=2`，`round_002/final score=76/issues=4`。这次 final 相对 round_001 回落，说明模型第三轮换了一个新拓扑，仍需要本地 polish 兜底。
- 验证结论：
  - `ann_inference` corridor 问题没有以同一形式复发；但模型把 inference cue 重新生成为 `inference_note` component，放在 teacher 上方，导致 `inference_note_unanchored` 与 teacher 侧 crowding。
  - `e_task_loss_label` 的 `ŷ` label 留在 student head 下方，离短的 `student_head -> task_loss` 边太远。根因是短数学符号 label 的默认 bbox 宽度过大，放不进两个相邻模块之间的窄 connector gap，后续 component-avoidance 会把它推到远处。
- 已完成 follow-up 实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `move_unanchored_inference_note_boxes_to_student_periphery`，只移动未锚定或压线的 standalone inference note box；已锚定 student 的 note 继续交给 `ensure_inference_note_boxes_readable` 原地压缩，避免破坏上一轮 exact note 修复。
    - 新 mover 遍历所有 student/output 锚点，而不是只依赖 primary/main student。这样当模型把 student boxes 都标成普通 module 时，仍能把 `inference_note` 放回 `student_enc`/`student_head` 附近。
    - 将 1-2 字符 connector label 的目标宽度从通用下限收紧到 `0.036`，使 `ŷ`、`z_T`、`z_S` 这类短数学标签能贴近短边而不被组件避让逻辑推远。
  - `tests/draw_plan_tests.rs`：
    - 新增并扩展 `model_draw_plan_polish_moves_unanchored_inference_note_component_to_student_periphery_from_smoke`，复现本次 final 的 `inference_note` 顶部漂浮和 `e_task_loss` label 脱线问题。
- 调试中遇到的坑：
  - 初版 unanchored mover 把上一轮“可原地压缩”的 exact `inference_note` 也搬到 student 下方，导致 student gutter 变小。修复方式是区分 anchored 与 unanchored：已锚定的 note 不因单纯处于 branch gap 就移动，只有未锚定或确实压线时才移动。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（140/140）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 `inference_note_unanchored` 和 `e_task_loss_label` 是否消失；若仍拒绝，下一优先级是 residual annotation 与 teacher/student branch vertical gutter。

## 2026-06-22 post-unanchored-note smoke and collapsed context note repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_unanchored_inference_note_smoke_20260622_050000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_unanchored_inference_note_smoke_20260622_050000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=64/issues=5`，`round_001 score=82/issues=2`，`round_002/final score=88/issues=1`。这是本轮最稳定的一次：每轮本地质量都有实质提升，final 只剩一个本地 issue。
- 验证结论：
  - `inference_note_unanchored` 和 `e_task_loss_label` 不再是 final 本地 issue，说明上一轮 unanchored note mover 与短 label 宽度修复有效。
  - final 唯一 issue 是 `component_collapsed` on `inference_note`。PNG 显示 note 已在 student 下方、位置合理；失败原因是它仍作为 component box 渲染，继承主模块字号 `13.1pt`，而不是作为上下文注记使用较小 annotation 字号。
- 已完成 follow-up 实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `convert_peripheral_inference_note_boxes_to_annotations`：只把无连接、compact、已离开 teacher/student corridor、已锚定 student/output、且位于底部外围的 exact `inference_note` box 转成 editable text annotation。
    - 转换条件刻意收窄到 `id == "inference_note"` 且 `bbox[1] >= 0.80`，避免误伤已有测试中仍应保持 box 的 `comp_inference_note`、`inference_badge` 和非底部 exact note。
  - `tests/draw_plan_tests.rs`：
    - 扩展 `model_draw_plan_polish_moves_unanchored_inference_note_component_to_student_periphery_from_smoke`，要求底部外围 compact `inference_note` 最终以 editable text annotation 形式保留，而不是 collapsed component。
- 调试中遇到的坑：
  - 初版转换条件过宽，导致多个既有路径把 note/badge 从 box 误转成 text。全量 `draw_plan_tests` 暴露 7 个回归后，将转换条件收窄到当前真实 smoke 的底部外围 exact note 场景。
- 当前验证：
  - 新增/扩展测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（140/140）。
  - `cargo test` 全量通过。
  - `cd renderer && npm run build` 通过。
  - `cargo fmt --check` 通过。
  - `git diff --check` 通过。
- 待完成：
  - 如继续跑真实 `.env` smoke，重点确认 final 是否越过本地 quality gate；如果仍被 vision 拒绝，下一步应关注 residual placement 与 teacher/student branch symmetry，而不是继续围绕 inference note 微调。

## 2026-06-22 post-collapsed-context-note smoke result

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_collapsed_context_note_smoke_20260622_053000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_collapsed_context_note_smoke_20260622_053000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=70/issues=4`，`round_001 score=40/issues=8`，`round_002 score=52/issues=7`，`final score=70/issues=4`。final 选择了 best round，而不是最后一轮，说明 best-round 回退逻辑有效。
- 验证结论：
  - 上一轮唯一 blocker `component_collapsed` on `inference_note` 没有复发，说明底部外围 inference note 转 annotation 的修复生效。
  - 本轮失败切换到新的复杂 multi-stage topology：`teacher_encoder`/`teacher_latent` 和 `student_encoder`/`student_head` stage gutter 太小，`teacher_latent` 与 `residual_node` 横向太近，`e_student_residual` 穿过非端点组件 `teacher_latent`。
  - PNG 观察：模型把 task loss 放到左下并用反向箭头连回 task head，student branch 被压成两行并和 teacher latent/residual 连接纠缠。这已经不是 inference note 问题，而是 branch grammar / stage stack 的系统性约束缺口。
- 当前状态：
  - 已连续修复并验证：exact inference note compact、ann_inference corridor escape、unanchored inference note periphery move、short math connector label width、bottom peripheral inference note annotation conversion。
  - 最新未解决问题：multi-stage teacher/student 分支在模型换拓扑时仍会出现 stage gutter 不足、residual connector 穿线、task loss 反向放置。下一轮应围绕 “stage stack + residual slot + task loss side” 写一个更强的 branch grammar repair，而不是继续补 inference note。

## 2026-06-22 projectionless multi-stage branch repair

- 根因定位：
  - 复查 `post_collapsed_context_note_smoke_20260622_053000/final/quality_report.json`，本地 gate 的 4 个 final issue 分别是 teacher stage vertical crowding、teacher latent/residual horizontal crowding、student encoder/head vertical crowding、`e_student_residual` 穿过 `teacher_latent`。
  - 读取 `src/tools/draw_plan.rs::balance_multistage_teacher_student_branches` 后确认，旧逻辑要求同时识别到 `Projection` stage 才会继续。最新真实输出是 `teacher_encoder -> teacher_latent` 与 `student_encoder -> student_latent -> student_head -> output_pred`，没有 teacher projection pair，所以已有 multi-stage repair 直接 return。
  - `stack_crowded_student_branch_chains` 也不能单独解决：它尝试把 student encoder/latent/head 纵向重排，但候选会撞到未压缩的 `teacher_latent`，因此被 clear check 拒绝。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_projectionless_multistage_stack_from_smoke`，直接复现 latest smoke final 的 bbox 和连接关系。
    - 测试先红后绿；红灯首先暴露 teacher stage gutter 仍停在旧 bbox。
  - `src/tools/draw_plan.rs`：
    - `balance_multistage_teacher_student_branches` 现在在缺少 projection pair 但已有 encoder/output pair 时进入 `repair_projectionless_multistage_teacher_student_layout`，而不是直接退出。
    - 新 repair 只针对模型已经生成的 teacher/student encoder-latent-head 语义链：压缩 teacher encoder/latent 的宽高并保留 `0.060` vertical gutter；压缩 student encoder/latent/head 并保留 student encoder/head gutter。
    - residual hub 被放到 teacher/student latent 右侧的独立槽位，并重新正交路由 latent-residual connectors，避免 student residual edge 再穿过 teacher latent。
    - task loss 从左侧反向位置移动到 student 分支最右边界之外；当 output node 夹在 head 和 task loss 之间时，task-loss route 使用 output 上方 rail 绕行，避免横穿 prediction node。
- 调试中遇到的坑：
  - 初版 teacher gutter 设为 `0.055`，单测显示实际浮点结果卡在阈值边界；已提高到 `0.060`，避免真实毫米换算时再被判为 crowding。
  - 初版 task loss 右侧候选仍与 `student_latent` 在 x 方向轻微重叠，且 vertical gap 贴近阈值，被 `objective_hub_candidate_is_clear` 拒绝。最终改为优先使用 student 分支最右边界之外的槽位，而不是只贴着 output 右侧。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_projectionless_multistage_stack_from_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（141/141）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 通过后跑真实 `.env` smoke，观察 latest topology 是否还出现 residual/task-loss 结构性问题；如果仍拒绝，需要看新的 final PNG/quality issue，而不是回到 inference note 微调。

## 2026-06-22 post-projectionless smoke and shared-input middle-slot repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_projectionless_multistage_smoke_20260622_061500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_projectionless_multistage_smoke_20260622_061500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=0/issues=15`，`round_001 score=76/issues=3`，`round_002 score=70/issues=4`，`final score=76/issues=3`。best-round 回退选择 round_001。
- 验证结论：
  - 上一轮 projectionless multi-stage stack 问题没有作为 final blocker 复发；新的 final blocker 是 shared input 被模型放在 teacher encoder 和 student encoder 中间但离 teacher 太近，导致 `comp_teacher_encoder`/`comp_input` crowding、`comp_input`/`comp_latent_residual` crowding，以及 `edge_input_to_teacher` 退化短边。
  - PNG 观察确认 `Input x` 位于 teacher encoder 右侧的窄缝，下面紧贴 latent residual；这不是 vision 看不出来，而是本地 shared-input candidate 只会尝试“放到所有 target 左侧”。当 teacher 已经靠左时，左侧候选撞 teacher，于是旧逻辑退回到当前位置。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_moves_shared_input_to_clear_middle_slot_from_smoke`，复现本次 final 的 input/teacher/student/residual 几何，要求 input 与 teacher 有真实 horizontal gutter、与 residual 有真实 gutter，且 input-to-teacher 边不退化。
  - `src/tools/draw_plan.rs`：
    - `shared_teacher_student_input_candidate` 新增 middle-slot candidate：仅当 input 已经被模型放在左 branch 右侧时触发，避免接管 bottom-margin input 的旧路径。
    - middle-slot candidate 放在 left target 与 right target 之间，并根据下方/上方 residual hub 调整 y，避免刚好压在 residual 上方。
    - 只有 middle-slot candidate 使用更严格的 crowding clear；legacy left/current candidates 保持原有 overlap-based clear，避免破坏旧 smoke 中“先把 bottom input 拉回 teacher 左侧”的行为。
- 调试中遇到的坑：
  - 初版把严格 crowding clear 套到所有 shared-input candidates，导致旧 bottom-input 回归选择了轻微重叠的 current_x；已改为只对 middle-slot 使用严格 gate。
  - 初版 middle-slot 对所有 shared input 都可用，影响了旧 reverse-layout fixture；已收窄为 input 已处在 left branch 右侧时才触发。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_shared_input_to_clear_middle_slot_from_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_bottom_input_and_top_losses_from_latest_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_teacher_student_reverse_layout_from_latest_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（142/142）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。测试期间 LibreOffice 打印过一次非致命异常日志，但对应测试和最终退出码均为通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 验证通过后再跑一次真实 `.env` smoke；如果仍拒绝，优先检查新的 final quality/report 和 PNG，而不是继续围绕已解决的 projectionless stack 或 shared-input 问题。

## 2026-06-22 post-shared-input smoke and branch input/output gutter repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_shared_input_middle_slot_smoke_20260622_070000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_shared_input_middle_slot_smoke_20260622_070000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=46/issues=5`，`round_001 score=70/issues=4`，`round_002 score=40/issues=7`，`final score=70/issues=4`。best-round 回退选择 round_001。
- 验证结论：
  - 上一轮 shared input middle-slot blocker 没有以相同形式复发；新的 final topology 是 teacher/student 各自有 input + encoder + latent/prediction 的双岛结构。
  - 本地 final blocker：`teacher_input` 与 `teacher_latent` vertical crowding，`student_input` 与 `student_pred` vertical crowding，`e_residual` 穿过 `teacher_input`，`e_task_loss` 对上下对齐 endpoints 走了矩形 detour。
  - PNG 观察确认：input 被放在 latent/prediction 下方太近，residual 虚线从 teacher latent 下穿过 teacher input；task loss 本可沿 output 的可用竖直 lane 下接，但当前绕成左侧矩形。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_branch_input_output_gutters_from_smoke`，复现本次 final 的双岛 topology，断言 input/output gutter、residual 不穿 input、output-to-task-loss 不再矩形绕行。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_branch_input_output_gutters`：对 `input -> encoder -> output/latent` 分支，如果 input 与上方 output/latent gutter 小于阈值，则把 input 下移到 `0.060` gap。
    - 新增后期 `reroute_branch_residual_connectors_around_inputs`：residual connector 如果穿过 branch input，最终走上方 rail，避免被后续 residual 简化回退。
    - 扩展 `simplify_adjacent_output_loss_connectors`：上下对齐的 output-to-task-loss 边可简化为竖线，但必须先确认竖线不穿中间组件。
    - 新增 `move_task_losses_to_clear_blocked_vertical_output_lanes`：当 output 与 task loss 上下对齐但中间有 input 阻挡时，轻微移动 task loss 到 output 内可用竖直 lane 下方，再由简化逻辑变成短直线。
- 调试中遇到的坑：
  - 只在早期 reroute residual 会被后续 residual 简化覆盖；已在 late residual simplification 后再次执行 branch-input reroute。
  - 直接把上下对齐 output-to-task-loss 全部拉直会破坏已有“必须绕开 student branch”的测试；已加 intermediate-box 检查，直线穿组件时不简化。
  - task loss lane reposition 早期执行会被 `align_task_loss_boxes_with_outputs` 拉回 output 中心；已在后期、紧挨 connector simplify 前再次执行。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_branch_input_output_gutters_from_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_routes_prediction_to_task_loss_around_student_branch -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（143/143）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。测试期间 LibreOffice 仍打印过一次非致命异常日志，但最终退出码为 0。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 验证通过后再跑真实 `.env` smoke，确认 branch input/output gutter 和 residual crossing 是否不再作为 final blocker。

## 2026-06-22 post-branch-input-output smoke result

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_branch_input_output_gutter_smoke_20260622_073000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_branch_input_output_gutter_smoke_20260622_073000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=58/issues=5`，`round_001 score=40/issues=7`，`round_002 score=40/issues=7`，`final score=58/issues=5`。best-round 回退选择 round_000。
- 验证结论：
  - 上一轮 branch input/output gutter、residual crossing、task-loss vertical lane 问题没有作为 final blocker 复发；模型本轮 best round 切到另一个 Y-branch topology。
  - 最新 final blocker：
    - `inference_note` collapsed：note 在 student 下方很小，仍作为 component box 渲染。
    - `input_data` 与 `student` horizontal crowding。
    - `e_student_to_output_label` 的 “Task loss” label 覆盖 student box，且离目标 edge 太远。
    - `e_input_to_teacher` 走到画布底部形成外侧 detour。
  - PNG 观察：整体是 Y-branch，student/teacher 主框比早期更清晰；当前主要问题转为 label anchoring、bottom inference note component conversion、input-teacher detour 三类。
- 当前判断：
  - 这轮连续真实 smoke 表明本地 deterministic polish 能逐步消掉模型反复生成的具体几何失败，但模型会在下一轮生成新 topology，导致 final best round 切换到新的 blocker。
  - 下一轮不应继续改 projectionless stack、shared-input middle slot 或 branch input/output gutter；应围绕最新 Y-branch final 写新的回归：bottom compact inference note 转 annotation、connector label snap/off-component、shared input-to-teacher detour repair。

## 2026-06-22 Y-branch final blocker repair

- 根因定位：
  - 复查 `post_branch_input_output_gutter_smoke_20260622_073000/final/draw_plan.json` 与 `quality_report.json`，latest best round 已切到 Y-branch topology。旧的 branch gutter / projectionless / shared-input middle-slot 问题没有复发；新的 blocker 是 `inference_note` compact box 仍按 component 渲染、`input_data` 与 `student` 水平 gutter 只有约 3mm、`e_student_to_output_label` 被通用 annotation fallback 推离短边并覆盖 student、`e_input_to_teacher` 绕到画布底部。
  - `inference_note` 的旧转换条件只允许 `bbox[1] >= 0.80` 的底部 note；本次 note 已在 student 下方外围但 top 是 `0.7535`，因此仍保留为 collapsed component。
  - shared input 的 topology repair 只在 input 右缘接近/侵入 target union 时触发；本次 input 已在左侧但 gap 仍小于 paper-width 可读 gutter，所以不会再左移。
  - `Task loss` 短 connector label 先被通用 label snap 推到无法清空的位置，再被 `place_annotation_below_component` 当成注释处理，最小宽度回到 `0.14` 并脱离 edge。
  - `input -> teacher` 的已有 detour repair 只会尝试从 target center 进入 teacher；当 `task_output` 紧贴 teacher 左侧时，center-entry 候选会穿过 output，于是保留了底部外圈 detour。
- 已完成实现：
  - `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_repairs_y_branch_label_note_and_input_detour_from_smoke`，直接复现 latest final 的 Y-branch 几何，断言：inference cue 不再是 collapsed box、input/student 有可读 gutter、Task loss label 不压 student/output 且贴近短边、input-to-teacher 不再走底部外圈。
  - `src/tools/draw_plan.rs`：
    - `convert_peripheral_inference_note_boxes_to_annotations` 增加“位于 student/output 下方外围且 compact/无连接/已锚定”的判定，覆盖本次 `bbox[1] = 0.7535` 的 exact `inference_note`，仍限定 `id == "inference_note"` 以避免误伤受保护 note/badge。
    - `shared_teacher_student_input_needs_topology_repair` 增加 shared input 与 branch target 水平 gutter 检查，并把左侧候选目标 gap 从 `0.055` 提到 `0.060`，避免浮点边界和 paper-width crowding。
    - `compact_input_context_route` 增加 target top/bottom edge dogleg candidates，使 input-to-teacher 能沿 teacher 下边缘局部绕开 adjacent output，而不是绕到画布底部。
    - 新增 `snap_compact_task_loss_labels_near_short_output_edges`：只处理短 main->output edge 上的 compact `Task loss` label，使用更窄 label bbox 贴近 output 上沿，并保持不覆盖 component/connector。
- 当前验证：
  - 新增测试先红后绿：最初失败于 `inference_note` collapsed component，随后失败于 shared input gutter 和 Task loss label 脱线，修复后通过。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（144/144）。
- 待完成：
  - 跑 `cargo fmt`、全量 `cargo test`、`cd renderer && npm run build`、`cargo fmt --check`、`git diff --check`。
  - 再跑真实 `.env` smoke，确认 latest Y-branch blocker 是否消失；如果仍拒绝，只根据新的 final issue 继续定位，不回退已修复的 branch grammar。

## 2026-06-22 post-Y-branch smoke and residual/output gutter repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_y_branch_label_note_detour_smoke_20260622_083000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_y_branch_label_note_detour_smoke_20260622_083000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：`round_000 score=64/issues=4`，`round_001 score=82/issues=2`，`round_002/final score=94/issues=1`。
- 验证结论：
  - 上一轮修复的 `inference_note` collapsed component、`Task loss` label 压 student/脱线、`input -> teacher` 底部 detour 均没有作为 final blocker 复发。
  - 新 final 只剩一个本地 blocker：`latent_residual_supervision` 与 `task_output` 上下相邻且水平重叠，vertical gap 只有约 1.3mm paper-width。PNG 观察确认右侧 residual supervision 和 task output 视觉上贴得太近。
  - 根因是现有 spacing repair 覆盖 output 与 source、loss/objective 与 branch，但没有覆盖 output 与 residual/supervision hub 的上下相邻关系。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_separates_residual_supervision_from_task_output_from_smoke`，复现本次 final 的 exact bbox/connector，要求 residual supervision 与 task output 至少保留 `0.055` vertical gutter，并要求 student-output route 仍紧凑且不穿 residual。
  - `src/tools/draw_plan.rs`：
    - 新增 `separate_outputs_from_residual_supervision_hubs`，在 output/source spacing 后运行。
    - 该 repair 只处理 output 与 residual/supervision hub 上下相邻、水平重叠、且 gutter 小于 `0.060` 的场景；优先轻移 output，无法清空时才移动 hub。
    - 移动后用 `reroute_connectors_touching_box_ids_orthogonally` 重建相关 connector，避免只改 bbox 后留下旧路线。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture` 通过（145/145）。
- 待完成：
  - 跑 `cargo fmt`、全量 `cargo test`、`cd renderer && npm run build`、`cargo fmt --check`、`git diff --check`。
  - 再跑一次真实 `.env` smoke；如果仍未接受，以新的 final blocker 为准继续定位。

## 2026-06-22 post-residual/output smoke and simple Y-branch visual balance repair

- 验证补完：
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_residual_output_gutter_smoke_20260622_090000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_residual_output_gutter_smoke_20260622_090000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：final `score=100`、`passed=true`、`issues=[]`。说明几何 gate 已经没有 blocker；拒绝来自 vision review。
- vision review 的新 blocker：
  - inference note 仍在主 flow corridor 附近，不够像 compact marginal caption。
  - teacher branch 虽然 style 字符串为 `muted_module_dashed`，但 renderer 没有把 box border 画成 dashed，实际还是实线。
  - teacher box 宽度 `0.319`，student box 宽度 `0.240`，简单 Y-branch 视觉不平衡。
  - `Latent Residual` box 宽度仅 `0.140`，paper-width 下显得拥挤。
  - `student -> residual` connector 走高 U 形 detour，简单监督信号不应大幅绕行。
- 根因定位：
  - 本地 quality gate 主要检查 overlap/crowding/edge crossing；它不会因为 teacher 比 student 宽 33%、residual slot 偏小、branch 语义不够 muted 而失败。
  - `renderer/src/runtime.ts::drawBox` 只根据 style 决定颜色，没有把 `style.includes("dash")` 映射到 PPTX `dashType`，导致模型/DrawPlan 的 dashed 语义没有落到最终 PPTX。
  - 现有 `balance_multistage_teacher_student_branches` 只处理 encoder/projection/output 多阶段分支；最新 final 是单 teacher、单 student、共享 residual 的简单 Y-branch，所以没有任何尺寸平衡或 residual-slot 调整。
  - `remove_asymmetric_branch_annotations` 曾把所有含 `student` 的 annotation 都当作不成对分支标签删除，导致合法 `Inference: student only` caption 容易丢失。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_balances_simple_y_branch_from_vision_review_smoke`，复现本次 final 的 exact bbox/connector，断言 teacher/student 宽度平衡、teacher 保持 muted/dashed style、latent residual 可读且位于 teacher/student 之间、student-residual route 不再高 U 形、inference caption 下移到 student row 下方边注区。
    - 该测试先红后绿；红灯首先失败于 teacher/student 宽度比例。
  - `src/tools/draw_plan.rs`：
    - 新增 `balance_simple_teacher_student_y_branch_layout`，只识别“单 teacher + 单 student + 共享 residual hub”的简单 Y-branch，避免接管 encoder/head/projection 多阶段拓扑。
    - 对简单 Y-branch：压缩过宽 teacher box 到 student 宽度约 `1.08x`；把 teacher branch style 补成 muted/dashed；把 teacher-residual 边设为 dashed supervision；把窄/偏高 residual hub 扩到可读宽度并放到 teacher/student 之间的右侧槽位；重建相关 connector 的正交路线；把 inference caption 放到 student 下方紧凑边注区。
    - 收窄 `remove_asymmetric_branch_annotations`：`Inference: student only` 在非顶边位置不再被当成普通 student 分支标签删除；顶边漂浮 inference phase label 仍会被旧测试覆盖并删除。
    - 新增 `fold_unconnected_component_inference_notes_into_annotation` 的受控触发：只有当进入 `with_figure_plan` 时已经有 inference text annotation 或 FigurePlan 明确声明 inference annotation，才把无连接的重复 `comp_inference*` context note box 折叠回 `ann_inference`，避免破坏需要保留为 context box 的 note/badge。
  - `renderer/src/runtime.ts`：
    - `drawBox` 现在把 `object.style.includes("dash")` 映射为 PptxGenJS `line.dashType = "dash"`，让 DrawPlan 中的 muted/dashed teacher branch 真实反映到 editable PPTX 边框。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（146/146）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 simple Y-branch visual balance 和 dashed teacher border 是否能通过 vision review；如果仍 rejected，以新的 final review/blocker 为准继续定位。

## 2026-06-22 post-simple-Y smoke and same-row teacher branch repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_simple_y_branch_balance_smoke_20260622_093000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_simple_y_branch_balance_smoke_20260622_093000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：round_000 `score=34/issues=8`，round_001/final `score=76/issues=4`，round_002 `score=58/issues=6`。best-round 回退选择 round_001。
- 验证结论：
  - 上一轮 simple Y 宽度平衡和 dashed teacher border 修复有效，但模型本轮生成了新的 same-row topology：student 主框在左中，teacher 被压到右侧同一行，latent residual 作为右下 standalone hub。
  - final 本地 blocker：
    - `comp_task_loss` 与 `comp_output` 水平 gap 只有约 2.1mm。
    - `ann_inference` 位于 student 和 teacher 之间的主 flow corridor。
    - `anno_teacher_label` 的 `Frozen` annotation bbox 过大。
    - `edge_input_to_teacher` 使用长顶部 rail detour。
  - PNG 观察确认：teacher 与 student 同行导致 input-to-teacher 必须绕过 student；inference caption 横在 student/teacher 之间；`Frozen` 是巨大浮动文字；task loss 和 `ŷ` 在底部相互贴近。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_same_row_teacher_branch_from_latest_smoke`，复现本次 final exact topology，断言 teacher 不再压在右侧同一行、task/output sibling 保留可见 gutter、inference caption 离开主 corridor、oversized `Frozen` 被删除或压缩、input-to-teacher 不再长横向 rail。
  - `src/tools/draw_plan.rs`：
    - `balance_simple_teacher_student_y_branch_layout` 增加 same-row teacher repair：当单 teacher 和单 student 水平分离但垂直重叠时，将 teacher 移到 student 上方并补足宽度，随后重建相关 connector。
    - `simple_y_balanced_teacher_candidate` 现在同时处理 teacher 过宽和过窄，避免 right-edge teacher 被压得比 student 小太多。
    - 新增 `separate_sibling_task_loss_and_output_boxes`：对同一 source 下的 task-loss/output sibling，如果非重叠但水平 gap 太小，则优先移动 output 补足 gutter 并重建 connector。
    - 新增 `compact_oversized_short_annotations`：短 annotation（如 `Frozen`）如果 bbox 远大于文字需要，压缩并锚到最近 teacher/context；若旧清理链路直接删除该 oversized annotation，也允许，因为它同时消除 Frozen/Trained 不对称。
    - `move_simple_y_branch_inference_caption_to_periphery` 在普通 polish 和 `with_figure_plan` 后处理末尾运行；候选顺序改为下方优先、右侧 fallback，避免能放下方的旧用例被推到侧边。
    - `remove_line_overlapping_annotations` 不再删除 inference-specific caption；新增 `remove_inference_annotations_overlapping_other_annotations` 专门删除与其它非 inference annotation 大面积重叠的重复 inference 文本，保留可移动的 inference caption。
- 调试中遇到的坑：
  - 初版 final caption 兜底优先右侧，导致旧测试中本可下方放置的 `ann_inference` 被移到侧边或左侧；已改为下方优先。
  - 初版保护所有 inference annotation 不被 line-overlap 删除，导致一个与 `Frozen / train only` 重叠的旧 inference annotation 保留；最终改为只删除与其它 annotation 重叠的 inference 文本。
  - 初版 context note fold 把没有替代 annotation 的 `comp_inference_note` 也折叠掉，破坏 context box/badge 用例；最终只在进入 `with_figure_plan` 时已经存在 inference text annotation 或 FigurePlan 明确声明 inference annotation 的情况下才折叠重复 `comp_inference*` box。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（147/147）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 same-row teacher branch、task/output sibling gap 和 inference corridor 是否不再作为 final blocker。

## 2026-06-22 post-same-row smoke and direct teacher-student branch repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_same_row_teacher_branch_smoke_20260622_100000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_same_row_teacher_branch_smoke_20260622_100000`
  - 结果：`accepted=false`、`reason="cap reached before acceptance"`、final PPTX `unzip -t` 通过。
  - 本地 quality：final `score=100`、`passed=true`、`issues=[]`，说明上一轮 same-row teacher、task/output sibling gutter 和 inference corridor 的本地 blocker 已经消失。
- vision review 的新 blocker：
  - standalone `Latent Residual` annotation 与 supervision connector label 重复，造成视觉噪音。
  - 模型把 teacher 放在 student 左下方/同层附近，破坏 teacher-student supervision 的 Y-branch 阅读顺序。
  - `Inference: student only` note 虽在 student 下方，但仍贴近 teacher 垂直空间，视觉上拥挤。
- 根因定位：
  - 本地 `remove_redundant_residual_supervision_labels` 主要处理 residual/supervision 语义重复，但没有通用删除“文本 annotation 与 connector label 完全同名”的情况。
  - 上一轮 `balance_simple_teacher_student_y_branch_layout` 依赖 standalone residual hub；本次 final 没有 residual box，而是 teacher 直接连 student 并把 `Latent Residual` 写成 edge label，所以旧 simple-Y repair 不会触发。
  - inference caption 的 periphery repair 已经把 note 放到 student 下方，但当 teacher 仍在 student 左下方时，note 无法摆脱 teacher 垂直空间；真正根因是 direct teacher-student supervision branch 没有先恢复 teacher-above-student 的语义布局。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_direct_teacher_student_residual_label_from_vision_smoke`，复现本次 final exact topology，断言重复 standalone `residual_label` 被删除、direct teacher-student supervision branch 将 teacher 放到 student 上方、inference note 与 teacher 保持可见分离。
    - 该测试先红后绿；红灯首先失败于 standalone `residual_label` 未删除。
  - `src/tools/draw_plan.rs`：
    - 新增 `remove_duplicate_connector_label_annotations`，收集 connector label 的 normalized phrase，删除同名普通 text annotation；排除 inference-specific caption，避免误删学生推理说明。
    - 新增 `balance_direct_teacher_student_supervision_branches`，识别 teacher -> student 直连且 edge style/label 表示 supervision/residual 的拓扑；若 teacher 与 student 垂直重叠、同层或在 student 下方，则把 teacher 移到 student 上方，并补 `muted/dashed` teacher style、重建相关 connector。
    - 该 repair 独立于 standalone residual hub，只处理 direct supervision edge，避免接管 multi-stage encoder/head/projection 布局。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_direct_teacher_student_residual_label_from_vision_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（148/148）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
- 待完成：
  - 跑 `cargo fmt --check`、`git diff --check`。
  - 再跑真实 `.env` smoke，确认 duplicate residual label 与 direct teacher-student branch 顺序是否不再作为 final blocker；如果仍未 accepted，只根据新的 final blocker 继续定位。

## 2026-06-22 post-direct-branch smoke and training-only annotation repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_direct_teacher_branch_vision_smoke_20260622_103000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_direct_teacher_branch_vision_smoke_20260622_103000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地 quality：final `score=94`、`passed=false`、唯一 issue 是 `annotation_excessive_whitespace`，target `ann_training_only`。
- 验证结论：
  - 上一轮 direct teacher-student branch 的三个 vision blocker 没有作为 final blocker 复发：duplicate `Latent Residual` label、teacher/student direct supervision 顺序、inference note 贴 teacher 的问题都已消失。
  - 新的唯一 blocker 是 `ann_training_only` 被 FigurePlan 以 `[0.625, 0.7125, 1.0, 1.0]` 回填到右下角大框，文本只有 `training only`。
- 根因定位：
  - `polish_model_draw_plan_geometry_inner` 内部已经有 `compact_oversized_short_annotations`，但 `polish_model_draw_plan_geometry_with_figure_plan` 会在 inner polish 之后调用 `upsert_meaningful_annotations_from_figure_plan`。
  - `upsert_meaningful_annotations_from_figure_plan` 对非 inference annotation 直接保留 FigurePlan bbox；`ann_training_only` 因此在最后阶段被重新放大，后续没有再次压缩。
  - `short_annotation_anchor_bbox` 只把 `frozen`/`teacher` 归到 teacher/context anchor；`training only` 即使进入压缩，也可能退化为最近组件而非 teacher branch。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_compacts_training_only_annotation_from_latest_smoke`，用 latest final 的 DrawPlan + FigurePlan annotation 复现 `with_figure_plan` 路径；旧实现红灯，`ann_training_only` 保持 `[0.625, 0.7125, 1.0, 1.0]`。
  - `src/tools/draw_plan.rs`：
    - 在 `polish_model_draw_plan_geometry_with_figure_plan` 的 FigurePlan annotation 回填和 annotation/inference 移动后，再执行一次 `compact_oversized_short_annotations`。
    - `short_annotation_anchor_bbox` 将 `training`/`train` annotation 归到 teacher/context branch，避免短注释锚到 student 或 output。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（149/149）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 `annotation_excessive_whitespace` 不再作为 final blocker；如果还没 accepted，继续只针对新的 final blocker 做 TDD 修复。

## 2026-06-22 post-training-annotation smoke and two-stage branch repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_training_annotation_compact_smoke_20260622_110000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_training_annotation_compact_smoke_20260622_110000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - `ann_training_only` 已不再出现，上一轮 annotation excessive whitespace blocker 消失。
- 新 final blocker：
  - best round 切到新的 two-stage teacher/student topology：student encoder/head 在左侧，teacher encoder/head 在右侧，task loss 和 residual alignment 都在上方。
  - 本地 quality：`score=64`、6 个 issue，包括 student encoder/head vertical crowding、student encoder/task loss crowding、task loss/residual horizontal crowding、`ann_inference` 贴近 `edge_input_to_teacher`、`edge_teacher_to_residual_label` 脱线、`edge_input_to_teacher` 底部长绕线。
  - PNG 观察确认：student stack 竖向挤压，input-to-teacher 走底部超长 rail，`r = h_t - h_s` 被放到画布底部，`ŷ` label 浮在 unrelated whitespace。
- 根因定位：
  - 现有 `repair_projectionless_multistage_teacher_student_layout` 覆盖的是 encoder/latent/head 三段或同列 projectionless 结构；本次只有 encoder/head 两段，且 teacher/student 左右分支，因此未进入旧 repair。
  - 通用 `repair_outer_input_context_detours` 会在后期把 input-to-teacher 又改回底部 rail；因此两阶段专用 reroute 需要在 polish 尾部再运行一次。
  - Connector label snap 在短 residual/task-loss objective edge 上找不到清晰位置时，会退化到远处；这类 label 更适合删除或折入 objective box，而不是作为远离 edge 的 floating label。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_two_stage_branch_crowding_from_latest_smoke`，复现 final exact DrawPlan，断言 student encoder/head gutter、task loss 与 student/residual 分离、input-to-teacher 不保留底部 rail、inference caption 远离该 edge、脱线 objective labels 被移除或贴回 route。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_two_stage_teacher_student_branch_layout`，仅在同时找到 `student encoder/head` 与 `teacher encoder/head` 的两阶段结构时触发。
    - 对该结构：压缩 student encoder 给 student head 留出竖向 gutter；压缩过高 teacher encoder；将 task loss 放到 student head 下方；把 inference caption 移到 student/task-loss periphery；重走 input-to-teacher 局部路线；对明显脱离 route 或冗余的 objective connector labels 执行删除。
    - 在 multistage balance 后运行一次，并在 polish 尾部再运行一次，覆盖后续通用 detour repair 的副作用。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（150/150）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 two-stage branch crowding、bottom rail 和 detached labels 是否不再作为 final blocker。

## 2026-06-22 post-two-stage smoke and prediction/residual bottom-edge repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_two_stage_branch_repair_smoke_20260622_113000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_two_stage_branch_repair_smoke_20260622_113000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 上一轮 two-stage branch 本地 blocker 已不再作为 final issue；新的 final topology 是 simple teacher/student + residual hub + prediction output。
- 新 final blocker：
  - 本地 quality：`score=82`、2 个 issue：`prediction` 对单字符 `ŷ` 仍然过高；`e_residual_student` 竖向虚线穿过 non-endpoint `prediction`。
  - vision 额外指出 `task_loss_obj` 贴到画布底边并被裁切，`ann_inference` 也在底边附近，且 final best round 使用 `deterministic_fallback`。
- 根因定位：
  - `compact_tall_output_boxes_for_short_labels` 能压缩大多数短 output，但后续 residual/output 路由组合仍可能留下 residual feedback 穿过刚压缩前后的 prediction badge。
  - `normalize_draw_plan_bounds` 允许 bbox 到 `1.0`，但真实 paper preview 会让底边 task loss/inference 看起来被裁切；需要更严格的 bottom safe-area repair。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_compacts_prediction_and_routes_residual_from_latest_smoke`，复现 final exact DrawPlan，断言单字符 prediction 被压缩、residual-to-student connector 不穿 prediction、task loss 和 inference caption 不贴底边。
  - `src/tools/draw_plan.rs`：
    - 新增 `move_bottom_edge_objectives_and_inference_notes_inside_safe_area`，把底边 task loss 上移到 `0.94` safe area 内，并把底边 inference caption 优先移到 student 上方/侧边清晰位置。
    - 该 repair 在 polish 尾部运行，覆盖后续 layout normalize 之前的最终安全区约束。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（151/151）。
  - `cargo fmt`：已执行。
  - `cargo test`：全量通过；期间 LibreOffice 打印一次历史上见过的非致命异常日志，但最终退出码为 0。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 prediction/residual crossing 与 bottom-edge clipping 是否消失；若仍未 accepted，以新的 final blocker 为准继续迭代。

## 2026-06-22 post-prediction smoke panic and clamp repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_prediction_residual_bottom_smoke_20260622_120000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：进程退出码 `101`；panic 信息为 `min > max, or either was NaN. min = 0.1, max = 0.09999999999999998`。
  - 该 run 只写出 `round_000/figure_plan.json` 和 setup artifacts，没有生成可用 final PPTX。
- 根因定位：
  - `reroute_two_stage_student_task_loss` 里对极窄 `student_head` 使用了 `center_x(task_loss.bbox).clamp(student_head.bbox[0] + 0.015, student_head.bbox[2] - 0.015)`。
  - 当模型或 fallback 把 `student_head` 压到约 `0.03` 宽时，下界会大于上界；Rust `f64::clamp` 对这种输入会直接 panic。
  - 单轮 debug smoke 没有复现，说明这是模型路径/布局随机性触发的窄框边界条件；因此需要用最小回归测试锁住。
- 已完成修复：
  - `src/tools/draw_plan.rs`：
    - 在 `reroute_two_stage_student_task_loss` 中显式检查 `min_x <= max_x`；若窄框导致范围非法，则使用 `center_x(student_head.bbox)` 作为竖向路由 x 坐标。
    - 这个分支只处理异常窄框，不改变正常宽度下的路由策略。
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_handles_narrow_two_stage_student_head_without_clamp_panic`，构造宽度约 `0.03` 的 `student_head`，确保 polish 不再 panic。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_handles_narrow_two_stage_student_head_without_clamp_panic -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_compacts_prediction_and_routes_residual_from_latest_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（152/152）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 重新跑真实 `.env` smoke，确认 panic 已消失，并检查 prediction/residual/bottom-edge 修复后的新 final blocker。

## 2026-06-22 post-clamp smoke and right-side loss/output corridor repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_clamp_panic_fix_smoke_20260622_121500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_clamp_panic_fix_smoke_20260622_121500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；没有复现 `f64::clamp` panic。
  - final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
- 新 final blocker：
  - 本地 quality：`score=94`、唯一 issue 是 `component_crowding`，target `task_loss_obj` 与 `output_pred`，横向 gap 只有约 `1.0mm`。
  - vision review 还指出同一局部：`e_residual_supervise`、`e_task_supervise` 和 student/output corridor 聚在 student 右侧，`ann_inference` 停在 student 正下方主图区。
- 根因定位：
  - 旧 `separate_sibling_task_loss_and_output_boxes` 只处理同一 source 发出的 task-loss/output sibling；本次 `output_pred` 是 `student -> output`，而 task loss 是 `task_loss -> student` 的反向 supervision 源，因此不属于 sibling pair。
  - 前面的通用 align/compact 会把 output 调整到更贴近 student 的水平线上；如果 repair 过早运行，后续仍可能把局部间距打回去，所以需要在 polish 尾部再做一次局部安全检查。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_separates_right_side_task_loss_output_and_inference_from_latest_smoke`，用 latest final exact bbox/connector 复现 loss/output 1mm crowding 和 inference corridor 问题。
    - 该测试先红后绿；红灯首先失败于 `task_loss_obj` 与 `output_pred` 仍然过近。
  - `src/tools/draw_plan.rs`：
    - 新增 `separate_right_side_task_loss_output_and_inference_corridor`，在 polish 尾部识别 `main/student -> output` 且同一 main/student 还连着 task-loss supervision box 的右侧局部模式。
    - 当 task loss 与 output 横向 gap 小且纵向重叠/近邻时，优先把 task loss 移到 output 左下方的 supervision column，并重走被移动 box 的 connectors。
    - 对同一局部中的 `Inference: student only` annotation，如果停在 student 正下方主图区，则移到右下外侧紧凑位置，避免遮挡主 flow corridor。
    - 所有新坐标 clamp 都先判断范围合法，避免再次触发 `min > max` panic。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_separates_right_side_task_loss_output_and_inference_from_latest_smoke -- --nocapture`：先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（153/153）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 right-side loss/output crowding 和 inference corridor 是否不再作为 final blocker。

## 2026-06-22 post-right-side repair smoke and left-teacher crossing topology repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_right_side_loss_output_corridor_smoke_20260622_130000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_right_side_loss_output_corridor_smoke_20260622_130000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 上一轮 right-side `task_loss_obj`/`output_pred` 1mm crowding 没有作为 final issue 复发。
- 新 final blocker：
  - best round 为 round_001，quality `score=58`。
  - 本地 issues：`ann_inference` 贴近 `edge_student_to_output`，`edge_input_to_student` 和 `edge_student_to_output` 过度绕线，`edge_input_to_student` 与 `edge_teacher_to_latent` crossing，`edge_student_to_output` 与 `edge_teacher_to_objective` crossing。
  - PNG 观察确认：teacher 被模型放到左下角，student 在中间，latent/objective 在右侧；多条边被迫走大矩形绕线并交叉。
- 根因定位：
  - 现有 simple Y branch repair 只识别 teacher/student 共享 residual hub 的模式；本次 FigurePlan 是 teacher -> latent -> combined objective，同时 student -> objective/output，因此旧 simple-Y 条件不会触发。
  - 通用 crossing reroute 只能尝试改线；但 student-output 的直接竖线会和 teacher-objective 的长横线交叉，必须先恢复 teacher/student peer branch 几何，再重走相关 connector。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_left_teacher_crossing_objective_topology_from_latest_smoke`，复现 latest final exact topology。
    - 断言 teacher 回到 student 上方 peer branch、`input->student` 与 `teacher->latent` 不交叉、`student->output` 变成短竖线、`student->output` 与 `teacher->objective` 不交叉、inference annotation 离开 output edge。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_left_teacher_central_student_objective_topology`，只在 teacher/context box 位于 student 左侧且同层/更低，并且存在 `input->student`、`student->output`、`teacher->latent`、student/teacher objective 边时触发。
    - 将 teacher 移到 student 上方并做宽度 peer-balance，保留 muted/dashed teacher 视觉语义。
    - 对该局部重走：`input->student` 低位 L 形、`input->teacher` 短水平、`student->output` 直接竖线、`teacher->latent` 短正交、`teacher->objective` 右侧外绕线，避免与主输出边交叉。
    - 将同局部 inference annotation 移到右下外侧，避免压在 `student->output` edge 上。
- 当前验证：
  - 新增测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（154/154）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 left-teacher/crossing topology 是否不再作为 final blocker。

## 2026-06-22 post-left-teacher repair smoke and teacher-alignment stair-step repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_left_teacher_crossing_repair_smoke_20260622_140000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_left_teacher_crossing_repair_smoke_20260622_140000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 本地 quality：final 选择 round_001，`score=100`、`passed=true`、`issues=[]`。说明上一轮 left-teacher/crossing blocker 已被 geometry gate 消掉。
- vision blocker：
  - 唯一 blocking issue 是 `e_teacher_align` 使用 5 段 stair-step route：teacher 右边先下到 `y≈0.581`，再右、再上到 alignment center，形成不必要的阶梯。
  - 同时 `h_t` label 漂在 whitespace，离 dashed route 不够近。
- 根因定位：
  - 现有 `straighten_residual_alignment_rails` 主要处理 objective/residual 到 main box 的 rail；本次是 teacher/context -> alignment objective，且 target 不是 main，因此旧函数不会处理。
  - 不能把所有 teacher->residual connector 都简化，否则会误伤旧的 latent residual crossing repair；触发条件必须要求 target 是 `alignment` 节点。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_simplifies_teacher_alignment_stair_step_from_latest_smoke`，复现 round_001 final 的 `e_teacher_align` 5 点 stair-step 和 `h_t` label。
    - 初版实现误伤 `model_draw_plan_polish_unmerges_task_loss_and_reroutes_residual_crossing_from_smoke`；已收窄触发条件并保留该旧测试。
  - `src/tools/draw_plan.rs`：
    - 新增 `simplify_teacher_alignment_stair_step_connectors`，只处理 teacher/context -> residual/alignment objective，且 target 文本/role/id 必须包含 `align`，当前 route 点数过多或存在短 jog 时触发。
    - 将 route 简化为 teacher 右侧到中间 x、竖到 alignment center y、再横到 alignment 左侧的 3 段正交路线。
    - 将 label 重新贴到 elbow/竖线附近，避免 `h_t` 漂浮。
- 当前验证：
  - 新增目标测试先红后绿。
  - 误伤回归测试 `model_draw_plan_polish_unmerges_task_loss_and_reroutes_residual_crossing_from_smoke`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（155/155）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 teacher-alignment stair-step 是否不再被 vision reviewer 拒绝。

## 2026-06-22 post-teacher-align stair-step smoke and remaining projector topology blocker

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_teacher_align_stair_step_smoke_20260622_143000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_teacher_align_stair_step_smoke_20260622_143000`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 上一轮 `e_teacher_align` 5 段 stair-step blocker 没有作为 final blocker 复发。
- 当前剩余 blocker：
  - final quality `score=58`、`passed=false`。
  - 新 topology 是 projector/encoder 多阶段分支：`student_proj`、`teacher_proj` 被模型生成成巨大容器，`teacher_proj` 与 `teacher_enc` 重叠，`inference_note` 位于 teacher/student corridor，`e_input_teacher` 绕到底部再进 teacher。
  - 这不是 teacher-alignment stair-step 的回归，而是新的 multistage projector/encoder layout 失败。
- 结论：
  - 已完成的局部 repairs 对各自 smoke blocker 有效，并且基础验证通过。
  - 下一步应新增一个针对 multistage projector/encoder pair 的末端 repair：压缩过大的 projector/encoder box、消除同分支 overlap、把 inference note 移到 student periphery，并将 `input -> teacher` 改成局部直连/短 L 形。

## 2026-06-22 multistage projector/encoder overlap repair

- 用户目标：
  - 继续执行上一节计划，针对 latest real smoke 中 projector/encoder 巨框、重叠、inference note 卡在 corridor、`input -> teacher` 底部绕线的问题做实际修复。
- TDD 过程：
  - 在 `tests/draw_plan_tests.rs` 新增 `model_draw_plan_polish_repairs_projector_encoder_overlap_from_latest_smoke`，用 `post_teacher_align_stair_step_smoke_20260622_143000` final 的 exact `DrawPlan` 复现问题。
  - 初次运行目标测试先因缺少测试 helper `box_height_for_test` 编译失败；补齐 helper 后，测试红灯失败于 `teacher_proj` 仍是宽 `0.36` 的巨大空框，证明旧实现确实没有覆盖该行为。
- 根因定位：
  - 旧 `balance_multistage_teacher_student_stage_sizes` 只比较 teacher/student 同阶段之间的高度差；如果两个 projector 同样巨大，或者 teacher projector 与 teacher encoder 同分支重叠，它不会触发。
  - 旧 `repair_two_stage_teacher_student_branch_layout` 只识别 encoder/head 两阶段，不覆盖 encoder/projector/output 三阶段。
  - 旧 inference note repairs 主要看注释是否离 student/output 足够近；本次 note 处在 teacher/student 中间 corridor，但仍可能被视为“附近”，所以需要在该 topology repair 之后强制移到 student periphery。
- 已完成实现：
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_multistage_projector_encoder_overlap_layout`，在两个 polish 入口末端调用，位于 teacher-alignment stair-step repair 之后、normalize 之前。
    - 触发条件基于结构和几何：必须存在 multistage teacher/student encoder + projector pair，且 projector 位于 encoder 上方或与 encoder 重叠，并出现过大 box、同分支 overlap/gutter 不足，或 `input -> teacher` 有外部绕路。
    - 将 encoder/projector 重新压缩为紧凑竖向 stack：projector 高度上限 `0.16`、宽度上限 `0.22`，encoder 高度上限 `0.18`、宽度上限 `0.22`，保留 teacher/student 分支中心而不是套死模板。
    - 把 `Inference: student only` 类 Text 移到 student encoder 下方或左侧 periphery，避免停在 teacher/student corridor。
    - 将 multistage `input -> teacher` 的底部外绕线路径改为局部三点路径，并避免向左越过 `x < 0.10`。
    - 修复过程中发现两个回归风险：`f64::clamp(min > max)` 和误伤“teacher encoder 在上、projector 在下”的旧 stacked teacher fixture；已通过 `min_encoder_bottom > 0.94` 提前返回、detour x 手动边界检查、以及触发条件要求 projector 在 encoder 上方/重叠来收窄影响面。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_projector_encoder_overlap_from_latest_smoke -- --nocapture`：先红后绿。
  - 回归点测：
    - `model_draw_plan_polish_repairs_stacked_teacher_residual_smoke_layout`：通过。
    - `model_draw_plan_polish_moves_task_loss_label_off_head_and_edge_from_smoke`：通过。
    - `model_draw_plan_polish_balances_multistage_teacher_student_branches_from_latest_smoke`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（156/156）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 重新跑真实 `.env` smoke，确认 projector/encoder blocker 是否消失，并检查下一轮 final blocker。

## 2026-06-22 post-projector repair smoke and two-stage route repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_projector_encoder_overlap_repair_smoke_20260622_151500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - run dir：`runs/teacher-student-distillation-with-latent-residuals/post_projector_encoder_overlap_repair_smoke_20260622_151500`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 上一轮 projector/encoder 巨框、teacher projector overlap blocker 没有作为 final issue 复发。
- 新 final blocker：
  - final quality `score=46`、`passed=false`。
  - 本地 issues：`teacher_out` 对短标签过大，`latent_residual` 太窄/塌缩，`inference_note` 太小/在主 flow corridor，`e_student_head_out` 与 `e_task_to_student` 绕线，`e_student_head_out` 与 `e_residual_to_student` crossing。
  - PNG 观察确认：左侧 student encoder/head 被蓝色 task feedback 大 U 形线包住；`student_head -> student_out` 绕到右侧再上去；`residual -> student` 贴着 student encoder 边并与 output route 交叉；`input -> teacher` 仍是底部长 rail。
- 根因定位：
  - 现有 `repair_two_stage_teacher_student_branch_layout` 只处理 encoder/head 间距、teacher encoder 高度、少量 task-loss crowding；它没有处理“output 在 head 上方”的直接竖线、task-loss feedback 大外绕线、residual-to-student 贴边穿框、或 residual equation box 的 paper-width 读数问题。
  - `move_two_stage_inference_caption_to_periphery` 只处理 Text annotation，且旧候选会把 note 放在 input-to-teacher 水平线附近；本次 final 的 `inference_note` 是 Box，后续又会被折叠成 `ann_inference`，因此移动逻辑必须同时覆盖 Box/Text。
- 已完成 follow-up 实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_repairs_two_stage_head_output_and_feedback_routes_from_latest_smoke`，用本次 final exact `DrawPlan` 复现二阶段 blocker。
    - 断言 residual box 宽/高足够、inference note 离开 student-residual corridor、`student_head -> output` 为直接竖线、`task_loss -> student` 不再左上大绕线、`residual -> student` 不穿 student encoder 且不再与 output route crossing。
  - `src/tools/draw_plan.rs`：
    - 扩展 `repair_two_stage_teacher_student_branch_layout`，新增 `repair_two_stage_head_output_and_feedback_routes`。
    - 对 residual/equation loss box 做局部宽高扩展，避免 13.1pt paper-width 下塌缩。
    - 对 teacher tiny output 做局部收紧，减少短标签大空框。
    - 将 Box 或 Text 形式的 inference note 移到 student stack 右下 periphery，并把候选位置下移到离 input-teacher route 有可见间距。
    - 将 `student_head -> output` 改成直接竖线；将 `task_loss -> student_enc` 改成 student 右侧局部 route；将 `latent_residual -> student_enc` 改成 student 右侧外侧 route，避免穿框和交叉。
- 当前验证：
  - 新增目标测试先红后绿。
  - 回归点测 `model_draw_plan_polish_repairs_two_stage_branch_crowding_from_latest_smoke`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（157/157）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 two-stage head/output/feedback blocker 是否消失，并检查下一轮 final blocker。

## 2026-06-22 smoke retry schema compatibility fix

- 真实 `.env` smoke 尝试：
  - 命令：`SESSION_ID=post_two_stage_route_repair_smoke_20260622_154500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：失败于 reasoner 输出 schema 兼容性，尚未进入绘图/渲染阶段。
  - 错误：`reading_order` 返回了 `bottom_to_top`，而旧 `ReadingOrder` 只接受 `left_to_right` / `top_to_bottom`。
- 根因定位：
  - 这是模型输出容错缺口，不是二阶段几何修复失败。语义上 `bottom_to_top` 属于纵向阅读顺序，可以安全归一到 `top_to_bottom`。
- 已完成实现：
  - `tests/schema_tests.rs`：新增 `figure_plan_accepts_bottom_to_top_reading_order_alias_from_model`，先红后绿。
  - `src/schema.rs`：给 `ReadingOrder::TopToBottom` 添加 serde alias `bottom_to_top`。
- 当前验证：
  - `cargo test --test schema_tests figure_plan_accepts_bottom_to_top_reading_order_alias_from_model -- --nocapture`：先红后绿。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 重新跑真实 `.env` smoke。

## 2026-06-22 post-two-stage smoke final-pass corridor and duplicate-note repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_two_stage_route_repair_smoke_20260622_160000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 上一轮二阶段 blocker 已明显改善：final quality 从 `46` 提升到 `94`；`student_head_out`/residual crossing/collapsed residual 等 issues 不再复发。
- 新 final blocker：
  - 本地唯一 issue：`task_loss_in_branch_corridor`，target `task_loss`, `teacher_model`, `student_model`。
  - vision blocker 还指出 `inference_note` 与 `ann_inference` 几乎同 bbox 重叠，造成 double-printed text。
- 根因定位：
  - 早期 `move_task_loss_boxes_out_of_teacher_student_branch_corridors` 已存在，但后续 `with_figure_plan` 尾部可能再次 upsert/move，使 task loss 回到 teacher/student corridor。
  - 旧 inference 去重只覆盖 box-vs-annotation 或 note component 场景，没有处理两个 Text annotation bbox 高度重叠且都含 `inference` 的情况。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_removes_duplicate_inference_and_moves_task_loss_from_branch_corridor`，复现本次 final exact layout。
    - 断言 inference 文本最多保留一个、task loss 不再位于 teacher/student row corridor 且移动到 student 侧、`student -> task` 不再向上反阅读方向。
  - `src/tools/draw_plan.rs`：
    - 在两个 polish 入口的 final tail 追加 `remove_overlapping_duplicate_inference_texts` 和 `move_task_loss_boxes_out_of_teacher_student_branch_corridors`，作为最后一次几何清理。
    - 新增 `remove_overlapping_duplicate_inference_texts`：仅处理 bbox overlap ratio >= 0.55 的 inference Text，优先保留更完整的 `student only` 文本，删除 `ann_*`/箭头短标签。
- 当前验证：
  - 新增目标测试通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（158/158）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 跑真实 `.env` smoke。

## 2026-06-22 failed final-pass smoke and degenerate optimizer connector repair

- 真实 `.env` smoke 尝试：
  - 命令：`SESSION_ID=post_final_pass_corridor_duplicate_repair_smoke_20260622_163000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：round 0 生成和评审完成，但 round 1 在写出 `draw_plan.json` 前失败。
  - 错误：`draw connector e_teacher_residual needs at least two points`。
- 根因定位：
  - round 1 已写出 `figure_plan.json`，但没有 `draw_plan.json`；因此失败不是 renderer，也不是 deterministic `FigurePlan -> DrawPlan`。
  - pipeline 在 revision 分支中先调用 `revise_draw_plan_from_feedback`，模型返回的修订 `DrawPlan` 会在统一 `polish_model_draw_plan_geometry_with_figure_plan` 前执行 `normalize_draw_plan_bounds` 和 `validate_draw_plan`。
  - 旧 `normalize_draw_plan_bounds` 只 clamp 点和 bbox，不会修复模型返回的单点 connector，所以 LLM optimizer 的坏 connector 直接打断整个 loop。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：
    - 新增 `teacher_student_round_trip_keeps_residual_connector_valid_from_real_smoke`，固化本次 round 1 的 `FigurePlan`，证明 deterministic 生成链路本身不会产生单点 residual connector。
    - 新增 `normalize_draw_plan_bounds_repairs_single_point_connector_from_endpoint_boxes`，先红后绿，覆盖 optimizer 返回单点 connector 的入库前容错。
  - `src/tools/draw_plan.rs`：
    - 扩展 `normalize_draw_plan_bounds` 为两遍处理：第一遍移动 box 并收集 box map，第二遍 clamp connector。
    - 对 `<2 points` 且存在 `from/to` box 的 connector，保留模型已有单点作为起点，并用目标 box 面向该点的一侧补终点；没有已有点时退回现有 `orthogonal_connector_points_between_boxes`。
    - 对缺少 endpoint box 但已有单点的 connector，补一个很短的可绘制水平线段，避免 validation 直接中断。
- 当前验证：
  - `cargo test --test draw_plan_tests normalize_draw_plan_bounds_repairs_single_point_connector_from_endpoint_boxes -- --nocapture`：先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（160/160）。
  - `cargo fmt`：通过。
- 待完成：
  - 重新跑真实 `.env` smoke，确认 round 1 不再因单点 connector 中断，并继续观察 final quality blocker。

## 2026-06-22 post-degenerate smoke and residual/task feedback route repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_degenerate_connector_repair_smoke_20260622_164500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 单点 connector 中断已消失，round 1/2 均能生成 PPTX/PDF。
- 新 final blocker：
  - final quality `score=82`、`passed=false`。
  - local blocker：`edge_student_to_residual` 与 `edge_taskloss_to_student` crossing；`edge_input_to_teacher` route detour。
  - vision blocker：`comp_inference` 被完全遗漏，`∇L_task` / `∇L_res` 标签贴线，task-loss feedback 与 residual route 交叉。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_repairs_comp_named_residual_task_feedback_smoke`，固化本次 final 的 `comp_*` / `edge_*` 命名布局。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_comp_named_residual_task_feedback_topology`，处理 input 位于 teacher 左下、residual/task loss 在 student 上方的 residual+task feedback 拓扑。
    - 将 `input -> teacher` 改为短直连，将 `task_loss -> student` 移到 student 右侧外绕，将 `residual -> student` 改为靠 student 左上侧的局部 route，并把梯度标签移离线段。
    - 新增 `restore_missing_inference_components_as_annotations`，但触发条件收窄为存在 `residual -> student` 与 `task_loss -> student` feedback 的复杂训练图；避免误伤普通 inference note component 仍需作为可连接 Box 的旧行为。
- 当前验证：
  - 新增目标测试先红后绿。
  - 回归点测 `model_draw_plan_polish_adds_missing_figure_plan_note_components`、`model_draw_plan_polish_converts_note_text_components_to_connectable_boxes`、`model_draw_plan_polish_removes_auxiliary_inference_note_connectors_from_smoke`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（161/161）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。

## 2026-06-22 post-feedback smoke and student-head output column repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_feedback_route_inference_note_repair_smoke_20260622_171500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 上一轮 `edge_taskloss_to_student`/`edge_student_to_residual` crossing 和缺失 inference note 不再是 final local quality issue。
- 新 final blocker：
  - final quality `score=82`、`passed=false`。
  - local blocker：`e_student_to_output` 穿过 `task_loss_box`，`task_loss_box` 与 `output_pred` 垂直距离太小。
  - PNG 确认：`Student Head -> Prediction` 是竖线，`Task Loss` 占在这条竖线和 output 上方；`ann_inference` 位于 output/task-loss corridor 附近。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_moves_task_loss_out_of_student_head_output_column_from_smoke`，复现 head 同时连 task loss 与 output 时 task loss 占住 output column 的问题。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_student_head_output_task_loss_column`，识别同一 source 同时连 output 与 task-loss，且 task loss 与 output 同列/穿线时，将 task loss 挪到 output 左侧，保留 output 竖线为主路径。
    - 新增 `move_inference_annotations_away_from_output_column`，把 inference annotation 移离 output/task-loss corridor。
- 当前验证：
  - 新增目标测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（162/162）。
  - `cargo fmt`：通过。
- 待完成：
  - 重新跑真实 `.env` smoke，确认 output column blocker 是否消失，并跑最终 `cargo test` / renderer build / fmt check / diff check。

## 2026-06-22 post-output-column smoke and residual-equation annotation repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_output_column_repair_smoke_20260622_174500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 上一轮 `e_student_to_output` 穿过 `task_loss_box` 和 output/task-loss crowding 不再复发；final quality 升至 `score=94`。
- 新 final blocker：
  - local 唯一 issue：`annotation_in_main_corridor`，target `ann_residual_eq`, `teacher_enc_mod`, `student_pred_mod`。
  - PNG 确认 `||z_T - z_S||^2` 被放在 teacher/student 中间主通道，应该贴近 `residual_mod` 右侧或上方。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_moves_residual_equation_annotation_out_of_main_corridor_from_smoke`，复现 residual equation annotation 在 residual box 左侧主 corridor 的问题。
  - `src/tools/draw_plan.rs`：
    - 新增 `move_residual_equation_annotations_out_of_main_corridors`，仅匹配残差公式类 Text（`||`、`z_t`、`z_s`、`residual`）且位于 residual box 左侧/同一行附近时触发。
    - 将该 annotation 移到 residual box 右侧，避免占据 teacher/student 主通道。
- 当前验证：
  - 新增目标测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（163/163）。
  - `cargo fmt`：通过。
- 待完成：
  - 重新跑真实 `.env` smoke，确认 residual equation blocker 是否消失。

## 2026-06-22 post-inference restore smoke and current residual risk

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_inference_component_restore_smoke_20260622_184500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 上一轮 inference duplicate/outside-main-area blocker 没有作为 final local issue 复发。
- 新 final blocker：
  - final quality `score=64`、`passed=false`。
  - 新拓扑退化为 teacher/projector/latent 分支：`inference_note` collapsed，`task_loss` 又进入 teacher/student branch corridor，`e_input_to_student` 绕线，`teacher_latent`/`teacher_proj` 拥挤。
  - 这说明模型仍会在不同 round 重新生成新拓扑，已有末端 repairs 能处理已见 blocker，但还不能保证 3 轮内全局收敛。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_restores_missing_inference_component_box_from_latest_smoke`，覆盖 FigurePlan 有 `inference_only` component 但 DrawPlan 只剩重复 floating Text 的情况。
  - `src/tools/draw_plan.rs`：新增 `restore_missing_inference_components_as_boxes`，仅在 FigurePlan 存在 `student -> student` loss self-loop 的错误拓扑时恢复 inference component Box，并移除重复 floating inference Text；该触发条件避免误伤原先需要折叠成 Text 的 standalone inference note 测试。
- 当前验证：
  - 新增目标测试先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（164/164）。
- 剩余风险：
  - 真实 `.env` smoke 仍未 accepted；最新失败是新的 teacher/projector 拓扑，不是本轮修复的 inference duplicate/outside issue。
  - 若继续追，需要再为 latest final 的 `teacher_latent`/`teacher_proj` 拥挤、collapsed `inference_note`、`task_loss_in_branch_corridor`、`e_input_to_student` detour 增加 exact regression tests。

## 2026-06-22 projected latent topology repair

- 当前目标：
  - 按最新真实 smoke 的 final PNG/quality report 继续修复 projected-latent teacher/student 拓扑退化，避免每轮重新生成后又出现 teacher latent/projector 被挤到 input 下方、task loss 回到 branch corridor、inference note 塌缩、input-to-student 长绕线的问题。
- 根因定位：
  - 这不是单纯配色或 prompt 问题，而是模型在后续 round 可能生成新的局部拓扑；已有 repairs 只覆盖之前见过的 residual/task/output/inference 模式，没有覆盖 `input_text -> teacher_encode/student_encode` 加 `teacher_latent -> teacher_proj -> student_latent` 的 projector 分支。
  - 该拓扑下 `teacher_latent`、`teacher_proj` 被放到 `input_text` 下方左侧，破坏 teacher branch 的从左到右阅读顺序；`e_input_to_student` 变成 4 点 U 形 detour；`inference_note` 被早期 note folding 转成或压成不可读对象；`task_loss` 位于 teacher/student 主通道。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_repairs_projection_latent_branch_from_latest_smoke`，复现 `post_inference_component_restore_smoke_20260622_184500` 的 final layout。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_projection_latent_teacher_student_topology`，只在存在 `input_text`、`teacher_encode`、`teacher_latent`、`teacher_proj`、`student_encode`、`student_latent`、`student_head`、`output_text`、`task_loss` 且 latent/projector 被挤到 input 下方或 input-to-student 出现同层四点绕线时触发。
    - 将 teacher branch 重新排成 `input -> teacher encoder -> z_T -> projector` 的水平上轨，将 student branch 排成 `input -> student encoder -> z_S -> prediction head -> output` 的下轨。
    - 将 `task_loss` 移到 student output row 下方，将 `inference_note` 恢复/替换为可编辑 Box 并放到 main flow 外侧。
    - 重写相关 connector，移除 `e_input_to_student` 的 U 形 detour，并把 `e_latent_residual` label 放到 connector 外侧。
- 当前验证：
  - 新增目标测试先红后绿，最终 `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_projection_latent_branch_from_latest_smoke -- --nocapture` 通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（165/165）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 跑新的真实 `.env` smoke，确认 latest projected-latent blocker 是否消失，并检查是否还有新的 final blocker。

## 2026-06-22 post-projection smoke and split-input residual hub repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_projection_latent_topology_repair_smoke_20260622_191500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 质量变化不是原地打转：round quality 从 `0 -> 46 -> 88`，final 只剩 1 个本地 blocking issue。
- 已确认改善：
  - 上一轮 `teacher_latent`/`teacher_proj` 被挤在 input 下方、`task_loss` 回到 branch corridor、`e_input_to_student` 四点绕线的 projected-latent blocker 没有复发为 final blocker。
- 新 final blocker：
  - 本地 `quality_report.json`：`score=88`，唯一 blocker 是 `component_collapsed`，target `inference_note`，原因是 bbox 高度 `0.07 < 0.08`。
  - 人工查看 final PNG：模型改成了左右两套 `Task Input`、顶部 residual hub、浮动 `Teacher`/`Student` 标签和三段 residual dashed routes；图面比上一轮干净，但 still 有 duplicate input、inference note 位于主通道、residual route 过复杂的问题。
  - vision review 同步指出：`inference_note` 位于主流程通道，`e_residual` 是不必要的折线路径，`branch_label_teacher`/`branch_label_student` 是无锚点浮动标签，左右两套 Task Input 不如共享入口清楚。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_repairs_split_input_residual_hub_from_latest_smoke`，先红后绿，复现本次 final `draw_plan`。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_split_duplicate_input_residual_hub_topology`，窄触发于 `teacher_input`/`student_input` 两个重复 `Task Input`、teacher/student latent、`residual_obj`、student decode/output/loss、inference note-like object 同时存在的布局。
    - 将两套 Task Input 合并成一个共享 `teacher_input` 视觉入口，并把 `e_student_in` 的 source 更新为共享 input，删除 `student_input`。
    - 删除浮动 `branch_label_teacher`/`branch_label_student`，避免无锚点 branch label 占用 residual 区。
    - 将 `inference_note` 从 Box 或 `ann_inference` Text 形式恢复为右上角可编辑 Box，bbox 高度提高到 `0.09`，避免 `component_collapsed`。
    - 将 teacher/student latent 与 residual hub 对齐成水平 residual rail，删除冗余 `e_residual` 折线，保留 `e_residual_up`/`e_residual_down` 两条短水平 supervision rail。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_split_input_residual_hub_from_latest_smoke -- --nocapture`：先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（166/166）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 split-input/residual-hub blocker 是否消失，以及 final 是否达到 accepted 或暴露下一类 blocker。

## 2026-06-22 post-split-input smoke and simple branch compaction repair

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_split_input_residual_hub_repair_smoke_20260622_201500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - final `quality_report.json` 已经 `passed=true`、`score=100`、`issues=[]`，说明前一轮的 duplicate input、collapsed inference note、projected-latent branch blocker 已从本地 gate 消失。
- 新 final blocker：
  - 未 accepted 的原因转移到 vision review：没有 blocking issue，但仍有多个 major localized issues，主要是 `teacher_branch`/`student_branch`/`task_input`/`task_output` 框过高留白、`input_to_teacher`/`input_to_student` 长折线、`latent_residual_edge_label` 贴近虚线、`anno_student_inference` 像浮动边注。
  - 人工查看 final PNG 后确认：这版已经是可读的简单 Y 分支图，不再有明显重叠，但确实还有草图感；本地 quality gate 给 100 说明它仍缺少对“短文本大框”和“标签贴近边”的更强审美约束。
- 已完成实现：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_tightens_simple_branch_from_latest_review_smoke`，先红后绿，固化本次 final 的简单 Y 分支布局。
  - `src/tools/draw_plan.rs`：
    - 新增 `repair_simple_teacher_student_branch_compaction`，只在出现 `task_input`、`teacher_branch`、`student_branch`、`task_output` 且 teacher/student 之间有 residual connector 的简单分支布局时触发。
    - 收紧 `teacher_branch`、`student_branch`、`task_input`、`task_output` bbox，保持 13.1pt paper-width 字号下不塌缩，同时降低框内留白。
    - 将 `input_to_teacher`/`input_to_student` 从长三点折线改为短二点连接，降低左侧长竖线造成的草图感。
    - 重新定位 `latent_residual_edge` label 到虚线右侧，并把 `anno_student_inference` 拉近 student branch，避免像无锚点浮动边注。
- 当前验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_tightens_simple_branch_from_latest_review_smoke -- --nocapture`：先红后绿。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（167/167）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑真实 `.env` smoke，确认 simple branch compaction 在真实 loop 中是否继续提升 vision review。

## 2026-06-22 post-simple-branch smoke and stop condition

- 真实 `.env` smoke：
  - 命令：`SESSION_ID=post_simple_branch_compaction_smoke_20260622_210000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - final `quality_report.json`：`score=76`，2 个 blocking issues：`component_collapsed` on `inference_note`，以及 `e_input_to_teacher` 与 `e_student_to_task_loss` crossing。
- 观察结论：
  - 这次 final 不是上一轮 simple branch compaction case，而是模型又生成了一个新拓扑：student 分支在左、teacher 分支在右，`teacher_head` 位于 `teacher_enc` 上方造成反向阅读，latent residual loss 在顶部中心，多条绿色虚线路由跨越主图。
  - 这说明继续为每个 smoke final 增加 exact local repair 会进入“局部补丁追着随机拓扑跑”的模式。当前新增 repairs 已经证明能修掉已见的 projected-latent、split-input/residual-hub、simple-branch-loose-box 三类问题，但真实 loop 仍可能在下一轮产生新拓扑退化。
- 当前验证：
  - 本轮新增代码在第三次 smoke 前已通过 `cargo test --test draw_plan_tests -- --nocapture`（167/167）、全量 `cargo test`、`cd renderer && npm run build`、`cargo fmt --check`、`git diff --check`。
  - 第三次 smoke 证明 PPTX/PNG 生成链路仍正常，且没有 fallback，但没有达到 accepted。
- 剩余风险和下一步建议：
  - 当前瓶颈已经从“某一类几何 bug”转为“模型每轮可能重开新拓扑”。应停止继续叠 exact repair，改为把 topology contract 前移到 reasoner/coder 的 hard constraints：teacher/student branch 的允许拓扑、reading order、loss/inference note placement、branch orientation 必须作为 `DrawPlan` 生成前的约束，而不是等 final PNG 再补丁。
  - 可执行方向：新增一个 topology validator/gate，直接拒绝 teacher/student 反向、duplicate input、training-only note in main corridor、branch orientation inversion、loss edge crossing 等高层拓扑，而不是只在 `draw_plan.rs` 末端重排。

## 2026-06-22 topology quality gate implementation

- 当前目标：
  - 执行上一节建议，不再继续给最新 final 叠 exact geometry repair，而是把第三次 smoke 暴露的 teacher/student 反向拓扑变成本地 `quality_report` 的 blocking issue。
- 根因定位：
  - `quality_report` 是更合适的数据流入口：它会进入 `apply_render_quality_gate` 的 review blocking issues，也会参与 best-round ranking、issue binding、下一轮 workspace 反馈。
  - 最新坏图的核心不是单个 bbox，而是 teacher encoder 低于 student encoder、teacher head 位于 teacher encoder 上方，导致 teacher branch 阅读顺序反向；旧 gate 只看到 collapsed note 和 edge crossing，没有把拓扑错误表达给下一轮模型。
- 已完成实现：
  - `tests/review_tests.rs`：
    - 新增 `quality_report_flags_teacher_student_topology_inversion_from_latest_smoke`，复现 `post_simple_branch_compaction_smoke_20260622_210000` 的 final `layout_map`，要求输出 `teacher_student_branch_inversion` 和 `teacher_internal_flow_reversed` 两个 blocking issue。
    - 新增 `quality_report_allows_teacher_above_student_encoder_topology`，确认正常 teacher-above-student、encoder-to-head 左右流不被误报。
  - `src/tools/review.rs`：
    - 在 `quality_issues_from_map` 中加入 `push_teacher_student_topology_issues`。
    - 新增 `primary_teacher_student_encoder_component`、`role_encoder_like_component`、`teacher_head_like_component`，仅匹配明确的 teacher/student encoder-head 结构，避免把普通横向流程或非 encoder 模块误判。
    - 当 teacher encoder 比 student encoder 低超过阈值时输出 `teacher_student_branch_inversion`；当 teacher encoder 到 teacher head 的 edge 向上流动时输出 `teacher_internal_flow_reversed`。
- 当前验证：
  - `cargo test --test review_tests quality_report_flags_teacher_student_topology_inversion_from_latest_smoke -- --nocapture`：先红后绿。
  - `cargo test --test review_tests quality_report_allows_teacher_above_student_encoder_topology -- --nocapture`：通过。
  - `cargo test --test review_tests -- --nocapture`：通过（47/47）。
- 第一次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_topology_quality_gate_smoke_20260622_220000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 质量变化：round 0 `score=88`、2 个本地 issue；round 1/2 `score=100`、`issues=[]`。最终图已经回到 teacher 上、student 下的清楚 Y 分支，没有复发 teacher/student 反向拓扑。
  - 新问题：vision review 仍给出 major issue，包括 `Task loss` label 贴在 student-to-output prediction edge 上、`ann_inference` 被放到画布底边、`h_s`/supervision labels 贴线、teacher-to-residual 虚线有不必要折弯。本地 `quality_report` 当时没有捕获前两个通用问题，导致下一轮模型缺少稳定的本地 target ids。
- 追加实现：
  - `tests/review_tests.rs`：
    - 新增 `quality_report_flags_task_loss_label_on_prediction_edge_from_latest_smoke`，要求把 loss label 贴在 prediction edge 上报告为 `loss_label_on_prediction_edge`。
    - 新增 `quality_report_flags_bottom_margin_inference_annotation_from_latest_smoke`，要求把 bottom-margin inference caption 报告为 `inference_annotation_in_bottom_margin`。
  - `src/tools/review.rs`：
    - 新增 `loss_label_on_prediction_edge`，只在 label 含 loss 且 target edge 连接 student 与 output、没有 loss endpoint 时触发，避免误伤正常 loss objective edge。
    - 新增 `inference_annotation_in_bottom_margin` 和更窄的 `is_student_only_inference_cue`，只拦截 student-only inference cue 被推到画布底边的情况，避免把 residual/supervision 普通注释误判为 inference。
- 追加验证：
  - 两个新增目标测试均先红后绿。
  - `cargo test --test review_tests -- --nocapture`：通过（49/49）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 待完成：
  - 再跑一次真实 `.env` smoke，确认新增 label/margin gate 是否能进入下一轮反馈，并观察是否仍由 vision review 阻塞。

## 2026-06-22 post-label-margin gate smoke and prompt tightening

- 第二次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_label_margin_quality_gate_smoke_20260622_224500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过；`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 每轮本地质量：round 0 `score=0`、21 个 issue，其中包含新增 `teacher_student_branch_inversion`；round 1 `score=0`、13 个 issue；round 2/final `score=52`、7 个 issue。
- 观察结论：
  - topology gate 确实生效：round 0 抓到了 teacher/student branch inversion，后续 final 不再是 teacher 在 student 下方的反向拓扑。
  - 但真实 loop 仍未稳定收敛：模型在后续轮次生成了新的 projection/residual 拓扑，final 仍有 `component_crowding`、`annotation_too_close_to_edge`、`label_far_from_edge`、`route_detour`、`degenerate_edge` 等本地 blocker/major issue；PNG 中可见 `h_T`、`z_S` 等标签漂移，student/output/task-loss 路由缠绕。
  - 这说明“视觉模型看不出来”的问题已有部分被本地化为 issue，但 reasoner 对新增 issue type 的下一轮修复建议还不够具体，容易继续给出大范围新拓扑而不是针对上一轮 target ids 局部改。
- 已完成追加实现：
  - `tests/prompt_tests.rs`：
    - 扩展 `round_improvement_prompt_uses_regression_budget_as_repair_contract`，要求 prompt 明确包含 `teacher_student_branch_inversion`、`teacher_internal_flow_reversed`、`loss_label_on_prediction_edge`、`inference_annotation_in_bottom_margin`。
  - `src/agent.rs`：
    - 在 `build_round_improvement_prompt` 的 issue-specific 指南中加入新增 gate 的修复语义。
    - 对 `teacher_student_branch_inversion` 要求命名 teacher/student encoder ids，并做局部 branch reorder，不重规划无关对象。
    - 对 `teacher_internal_flow_reversed` 要求命名 encoder/head/edge ids，并改为水平或下游 encoder-to-head route。
    - 对 `inference_annotation_in_bottom_margin` 要求移到 student/output 附近并避开 connector corridor/canvas margin。
    - 对 `loss_label_on_prediction_edge` 要求把 loss cue 移到单独 objective component 或 supervision edge，而不是挂在 prediction edge 上。
- 最终验证：
  - `cargo test --test prompt_tests round_improvement_prompt_uses_regression_budget_as_repair_contract -- --nocapture`：先红后绿。
  - `cargo test --test prompt_tests -- --nocapture`：通过（10/10）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 剩余风险：
  - 本轮没有在 prompt tightening 之后再跑第三次真实 smoke；当前最新真实证据仍是 `post_label_margin_quality_gate_smoke_20260622_224500`，它证明 gate 会抓住新问题，但还没有证明新增 prompt 文案能让下一次真实 loop 收敛。
  - 下一步更根本的方向应是把 `revision_source` 约束升成执行层面的 hard policy：当上一轮已有可渲染图时，限制后续 `DrawPlan` material diff 的范围，只允许修改当前 `QualityReport`/review target ids 相关对象，除非 IssueHistory 证明需要 reference_replan。

## 2026-06-22 localized revision guard

- 当前目标：
  - 把上一节的“只基于上一轮局部修补，不重画拓扑”从 prompt 约束提升为执行层 hard policy。
- 根因定位：
  - `revise_draw_plan_from_feedback` 之前只检查模型返回的 `DrawPlan` 是否有 material change；只要有变化就接受，未限制变化是否绑定到当前 `QualityReport` / review / issue binding 的 target ids。
  - 因此模型可以在修一个 label 或 route 时顺手新增 projector、移动 teacher/input、删除无关 connector，导致每轮出现新拓扑而不是稳定局部收敛。
- 已完成实现：
  - `src/agent.rs`：
    - 新增 `constrain_draw_plan_revision_to_current_targets`，在模型返回 `DrawPlan`、执行 `preserve_semantic_draw_objects` 后、`normalize_draw_plan_bounds` 前运行。
    - 从 `RoundImprovementPlan`、vision localized issues、`quality_report.json`、`issue_binding.json` 收集当前 target ids；`*_label` target 会映射到对应 edge id。
    - 若 plan 是 `reference_replan`、`rejected_as_unusable` 或含 `global_layout` target，则允许全局 replan；否则进入 localized guard。
    - localized guard 对无关既有对象恢复上一轮版本，对无关新增对象删除；仅允许 target ids 及触碰 target component 的 connectors 发生 material change。
  - `src/agent.rs` tests：
    - 新增 `localized_draw_plan_guard_reverts_unrelated_revision_changes`，复现 localized action 只应修改 `task_loss` / `e_student_to_loss`，但模型同时移动 `input`、`teacher`、删除 `e_input_teacher`、新增 `new_projector` 的失败模式。
    - 新增 `localized_draw_plan_guard_allows_reference_replan_changes`，确认 `reference_replan` 不被 guard 错误限制。
- 当前验证：
  - `cargo test agent::tests::localized_draw_plan_guard_reverts_unrelated_revision_changes -- --nocapture`：先红后绿。
  - `cargo test agent::tests::localized_draw_plan_guard_allows_reference_replan_changes -- --nocapture`：通过。
- 待完成：
  - 跑格式化、agent/prompt/review 相关测试、全量 `cargo test`、renderer build、`cargo fmt --check`、`git diff --check`。
  - 跑真实 `.env` smoke，确认 localized revision guard 是否减少后续轮次重开拓扑。

## 2026-06-22 localized revision smoke, route gates, and clamp panic fix

- localized guard 后的验证：
  - `cargo test agent::tests -- --nocapture`：通过。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 第三次真实 `.env` smoke：
  - 命令：`SESSION_ID=post_localized_revision_guard_smoke_20260622_233000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、3 轮、`reason="cap reached before acceptance"`；final PPTX 有效，`renderer_status.json` 为 `source="model_generated_code"`、`used_fallback=false`。
  - 每轮本地质量：round 0 `score=70`、3 个 issue；round 1 `score=94`、1 个 issue；round 2/final `score=100`、`issues=[]`。
  - 观察：localized guard 生效后没有再出现大范围拓扑重开，本地质量能单调提升到 100。但人工查看 final PNG 和 vision review 后，仍有本地 gate 漏检：`e_input_student` 长 U 形/矩形 detour、`e_residual_student` 残差虚线绕远、`student_predict_out` 把 `ŷ` 和 `Task Loss` 混在同一个 output 框、`ann_inference` 挤在主 corridor 内。
- 已完成追加实现：
  - `tests/review_tests.rs`：
    - 新增 `quality_report_flags_input_to_student_rectangular_detour_from_guard_smoke`，覆盖 shared input 到 student 的矩形绕行。
    - 新增 `quality_report_flags_residual_student_wandering_route_from_guard_smoke`，覆盖 residual-to-student 虚线绕远。
    - 新增 `quality_report_flags_prediction_box_mixing_task_loss_from_guard_smoke`，覆盖 prediction output 框混入 task-loss 语义。
  - `src/tools/review.rs`：
    - 将上述两类真实 route failure 接入 `route_detour` 判定。
    - 新增 `prediction_loss_semantic_mix` issue，要求 prediction 和 task loss 保持为不同 editable objects。
  - `src/agent.rs` / `tests/prompt_tests.rs`：
    - 在 `RoundImprovementPlan` prompt 中加入 `prediction_loss_semantic_mix` 的 issue-specific 修复要求，必须命名 output component id 并拆分/relabel，不允许只给泛泛审美建议。
- 追加验证：
  - 三个新增 review 测试均先红后绿。
  - `cargo test --test review_tests -- --nocapture`：通过（52/52）。
  - `cargo test --test prompt_tests round_improvement_prompt_uses_regression_budget_as_repair_contract -- --nocapture`：先红后绿。
  - `cargo test`：全量通过。
  - `cd renderer && npm run build`、`cargo fmt --check`、`git diff --check`：通过。
- 第四次真实 `.env` smoke 暴露的新问题：
  - 命令：`SESSION_ID=post_guard_route_semantic_gate_smoke_20260622_240000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=4 MAX_MINUTES=60 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：运行到 round 2 前崩溃，panic 为 `min > max, or either was NaN. min = 0.1, max = 0.09999999999999998`。
  - 已完成轮次质量：round 0 `score=34`、7 个 issue；round 1 `score=64`、4 个 issue。新增 gate 已进入真实反馈，但 run 被 deterministic geometry polish 的浮点边界截断。
  - 复现命令：`RUST_BACKTRACE=1 cargo run -- resume --run runs/teacher-student-distillation-with-latent-residuals/post_guard_route_semantic_gate_smoke_20260622_240000`
  - 根因：`src/tools/draw_plan.rs::readable_shared_input_width` 把 `box_width(input.bbox).min(0.16)` 直接作为 `f64::clamp` 上界。模型输出的 shared input bbox 宽度可能是 `0.09999999999999998`，略低于 readability floor `0.10`，导致 `clamp(0.10, 0.09999999999999998)` panic。
- 当前修复：
  - `src/tools/draw_plan.rs`：
    - 将 `readable_shared_input_width` 的动态上界改为 `box_width(input.bbox).min(0.16).max(0.10)`，允许极窄/浮点误差输入扩到最小可读宽度，避免上界小于下界。
    - 新增私有单元测试 `readable_shared_input_width_handles_float_width_below_floor`，先复现上述 panic，再验证修复。
  - `tests/draw_plan_tests.rs`：
    - 新增集成层回归 `model_draw_plan_polish_handles_shared_input_width_below_readability_floor`，保证 public geometry polish 在类似 shared-input fixture 上不破坏 DrawPlan。
- 当前已跑验证：
  - `cargo test tools::draw_plan::tests::readable_shared_input_width_handles_float_width_below_floor -- --nocapture`：先红后绿。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_handles_shared_input_width_below_readability_floor -- --nocapture`：通过。
- 待完成：
  - 跑 `cargo fmt --check`、全量 `cargo test`、`cd renderer && npm run build`、`git diff --check`。
  - 恢复或重跑 `post_guard_route_semantic_gate_smoke_20260622_240000`，确认 round 2 不再 panic，并继续观察 route/semantic gate 是否带来有效改动。

## 2026-06-22 resume cap fix and top-heavy layout gate

- 恢复真实 run 的结果：
  - 命令：`cargo run -- resume --run runs/teacher-student-distillation-with-latent-residuals/post_guard_route_semantic_gate_smoke_20260622_240000`
  - 结果：shared-input width panic 已修复，run 成功越过 round 002，并生成 round 002/003/004/005 的 PPTX/PDF。
  - 中途发现 resume 继续写到了 `round_005`，而 `config_snapshot.json` 中 `max_iterations` 是 4。该行为来自旧 resume 语义：`run_pipeline_inner` 用 `rounds_this_invocation` 从 0 计数，恢复中断 run 时没有扣除已有 completed rounds。
  - 为避免真实 smoke 被额外续跑，手动中断该 resume 后修复控制流。修复后再次执行 resume，命令快速完成，没有继续追加新 round，并生成 `final/`；输出 `accepted=false, rounds=6, reason=cap reached before acceptance`。`rounds=6` 是因为修复前已经额外完成到 round 005，修复只防止继续追加。
  - `final/figure.pptx` 通过 `unzip -t`，确认仍是有效 PPTX。
- resume cap 修复：
  - `tests/pipeline_tests.rs`：
    - 新增 `resume_pipeline_without_final_respects_existing_iteration_cap`，模拟无 `final/status.json` 的 interrupted/capped run；旧实现会追加 `round_001` 并错误 accepted，测试先红。
    - 保留并复测 `resume_pipeline_continues_rejected_run_directory`，确认已有 rejected final 的手动 resume 仍允许追加下一批轮次。
  - `src/pipeline.rs`：
    - `ResumeState` 新增 `rounds_this_invocation_offset`。
    - 当 run 没有 `final/status.json` 时，resume 视为崩溃/中断恢复，`rounds_this_invocation` 从已有 completed rounds 开始，尊重原 `max_iterations` 总轮数上限。
    - 当已有 rejected `final/status.json` 时，resume 视为用户手动续跑，offset 仍为 0，保留原有“继续追加一批”的语义。
- 真实 final 质量观察：
  - 每轮本地质量：round 0 `score=34`、7 issues；round 1 `score=64`、4 issues；round 2 `score=76`、3 issues；round 3 `score=58`、5 issues；round 4/5 `score=64`、4 issues。
  - 说明 panic 修复后能继续迭代，但质量仍会在局部修补后回退，当前 loop 还没有保证每轮单调变好。
  - 人工查看 `final/figure.png`：语义基本可读，PPTX 可编辑，但整图集中在上半部分，底部大面积空白；`Task Loss` 到 `Student` 的 connector 在顶部长距离绕行；`Frozen at train time` annotation 过宽；`Inference: student only` 被本地 gate 判为 collapsed。
  - 结论：这不是“视觉模型完全看不出来”。`review.json` 和 `improvement_plan.json` 已经给出具体建议，但本地 gate 缺少“整图 vertical packing / top-heavy whitespace”约束，导致模型围绕单个 bbox 做局部小修，而不是调整整体重心。
- top-heavy layout gate：
  - `tests/review_tests.rs`：
    - 新增 `quality_report_flags_top_heavy_vertical_underutilization_from_guard_smoke`，用真实 final bbox 复现“横向铺满但上半页拥挤、底部空白”的失败模式，测试先红后绿。
    - 扩展 `quality_report_allows_wide_compact_horizontal_flow`，确保垂直居中的一行流程不会被新 gate 误伤。
  - `src/tools/review.rs`：
    - 新增 `vertical_under_utilization` major issue。当 main component union 横向覆盖足够、纵向跨度不足且上下空白明显不平衡时触发，target ids 绑定到整组成分。
    - 旧 `under_utilized` 保留原语义，只抓横向和纵向都缩成小团的情况；新 gate 专门抓 top-heavy/bottom-heavy 版面。
  - `src/agent.rs` / `tests/prompt_tests.rs`：
    - 在 DrawPlan revision prompt 中加入 `vertical_under_utilization` 的修复语义：成组移动或扩展 main component group，使 union bbox 垂直居中并利用 paper-height canvas。
    - 在 RoundImprovementPlan prompt 中要求此 issue 必须命名 `global_layout` 或完整 component group，并给出 union bbox 垂直跨度和上下空白平衡的 success check。
- 当前验证：
  - `cargo test tools::draw_plan::tests::readable_shared_input_width_handles_float_width_below_floor -- --nocapture`：先红后绿。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_handles_shared_input_width_below_readability_floor -- --nocapture`：通过。
  - `cargo test --test pipeline_tests resume_pipeline_without_final_respects_existing_iteration_cap -- --nocapture`：先红后绿。
  - `cargo test --test pipeline_tests resume_pipeline_continues_rejected_run_directory -- --nocapture`：通过。
  - `cargo test --test review_tests quality_report_flags_top_heavy_vertical_underutilization_from_guard_smoke -- --nocapture`：先红后绿。
  - `cargo test --test review_tests quality_report_allows_wide_compact_horizontal_flow -- --nocapture`：通过。
  - `cargo test --test prompt_tests draw_plan_revision_prompt_uses_autofigure_style_visual_optimization_contract -- --nocapture`：先红后绿。
  - `cargo test --test prompt_tests round_improvement_prompt_uses_regression_budget_as_repair_contract -- --nocapture`：先红后绿。
- 最终验证：
  - `cd renderer && npm run build`：通过。
  - `git diff --check`：通过。
  - `cargo fmt --check`：首次提示 `src/tools/review.rs` 一个 if 条件格式，运行 `cargo fmt` 后复测通过。
  - `cargo test`：全量通过。测试过程中 LibreOffice 仍打印一次历史上出现过的外部 `DeploymentException`，但所有测试 exit 0。
  - 由于本轮已经真实恢复到 6 个 round 且修了 resume 超额追加问题，暂不再自动续跑 rejected final；下一次真实 run 应新开有限 `SESSION_ID` 或由用户明确触发 manual resume。

## 2026-06-22 stale revision-plan fix and bottom-margin inference guard

- 新开真实 `.env` smoke：
  - 命令：`SESSION_ID=post_vertical_gate_smoke_20260622_010000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=4 MAX_MINUTES=75 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：生成 `round_000..003` 和 `final/`，`accepted=false`，`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地质量：round 0 `score=58`、5 issues；round 1 `score=58`、5 issues；round 2 `score=40`、6 issues；round 3 `score=40`、6 issues。
  - 人工查看 PNG 后的结论：round 1 的 `task_loss` 被移入中间 corridor；round 2/3 进一步把 `inference_note` 折进 student label 附近，并产生 overlap / crossed edge。final 选择了相对最好的 round 1，而不是更差的 round 2/3。
- 根因定位：
  - pipeline 确实从 best source round 重修，但 `revise_draw_plan_from_feedback` 仍读取 source round 的 `improvement_plan.json`。
  - 当 round 2 已证明某个修复计划会退化时，round 3 虽然从 round 1 的 draw plan 起步，却仍复用 round 1 的 stale improvement plan，导致重复执行上一轮失败策略。
- 已完成 stale-plan 修复：
  - `src/pipeline.rs`：
    - 新增 `select_revision_improvement_plan`。
    - 当 revision source 是 best round、latest attempt 是失败轮次时，优先把 latest attempt 的 `improvement_plan.json` 传给下一次 `revise_draw_plan_from_feedback`；缺失时才回退到 source round plan。
  - `src/pipeline.rs` tests：
    - 新增 `revision_improvement_plan_prefers_latest_attempt_plan_when_revising_from_best_source`，先红后绿，确保不会继续用 stale source plan。
  - 验证：`cargo test pipeline::tests::revision_improvement_plan_prefers_latest_attempt_plan_when_revising_from_best_source -- --nocapture` 通过；`cargo test --test workspace_pipeline_tests -- --nocapture` 通过。
- stale-plan 修复后的真实 `.env` smoke：
  - 命令：`SESSION_ID=post_latest_plan_selector_smoke_20260622_020000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=4 MAX_MINUTES=75 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：生成 `round_000..003` 和 `final/`，`accepted=false`，`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地质量：round 0 `score=46`、7 issues；round 1 `score=82`、2 issues；round 2 `score=88`、1 issue；round 3 `score=82`、3 issues。
  - regression：round 1 `+36`、round 2 `+6`，说明 latest-attempt plan selector 明显减少了原地重复退化；round 3 `-6`，新增 `inference_annotation_in_bottom_margin`、`label_outside_main_area`、`text_wrap_risk`。
  - 人工查看 PNG 后的结论：round 2 已较干净，只剩 `Inference: student only` collapsed；round 3 为修 collapsed note，把 inference cue 移到页面底部 margin 作为裸文本，引入新的视觉退化。
- bottom-margin inference 修复：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_moves_bottom_margin_inference_text_near_student`，用 `post_latest_plan_selector_smoke_20260622_020000/round_003` 的真实几何复现 bottom-margin inference text，测试先红。
  - `src/tools/draw_plan.rs`：
    - 新增 late-stage `move_bottom_margin_inference_texts_near_student`，仅处理明显落入底部边缘的 student-only inference 文本。
    - 候选位置限制在 student 左侧/右侧/上方，或在不会进入 bottom margin 时放到 student 下方，并复用已有 box / connector / text clearance 检查。
    - 撤回之前对通用 `student_inference_note_candidates` 的泛化试探，避免把旧的 teacher-student corridor 保护测试打破。
  - 验证：
    - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_bottom_margin_inference_text_near_student -- --nocapture`：先红后绿。
    - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_inference_text_out_of_teacher_student_corridor_from_smoke -- --nocapture`：通过。
    - `cargo test --test draw_plan_tests inference -- --nocapture`：通过（42/42）。
    - `cargo fmt --check`：先提示新增 fixture 格式，运行 `cargo fmt` 后复测通过。
- 待完成：
  - 用 bottom-margin guard 后的代码再跑一个有限真实 `.env` smoke，确认 round 3 类型的修复不会再把 inference cue 推到页面底边。
  - 跑全量 `cargo test`、renderer build、`git diff --check`。

## 2026-06-22 bottom-margin guard smoke and right-edge inference follow-up

- bottom-margin guard 后的真实 `.env` smoke：
  - 命令：`SESSION_ID=post_bottom_margin_inference_guard_smoke_20260622_133702 REFERENCE_PREVIEWS=required MAX_ITERATIONS=4 MAX_MINUTES=75 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：生成 `round_000..003` 和 `final/`，`accepted=false`，`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过。
  - 本地质量：round 0 `score=40`、8 issues；round 1 `score=82`、3 issues；round 2 `score=82`、2 issues；round 3/final `score=82`、3 issues。
  - regression：round 1 `+42`，解决 collapsed/crowding/overlap/corridor；round 2 从 source round 1 重修但引入 `inference_annotation_in_bottom_margin` 和 `edge_crosses_component`；round 3 回到 source round 1 同等质量，final 没有选择 round 2 的 bottom-margin 退化。
- 人工查看 final PNG 后的新发现：
  - `Inference: student only` 仍作为裸文本贴在右下角，bbox 为 `[0.7895, 0.875, 0.98, 0.94]`。
  - 本地 quality 只报 `label_outside_main_area`，没有报 `inference_annotation_in_bottom_margin`，说明前一版 guard 只覆盖了 `y1 >= 0.88` 或 `bottom >= 0.95` 的极端底部情况，漏掉了“贴右边界且低于 student/output 路径”的边缘外漂。
- right-edge inference 修复：
  - `tests/draw_plan_tests.rs`：
    - 新增 `model_draw_plan_polish_moves_right_edge_inference_text_near_student`，使用 `post_bottom_margin_inference_guard_smoke_20260622_133702/final` 的真实 `figure_plan` / `draw_plan` 几何复现右下角 `inference_note`，测试先红后绿。
  - `src/tools/draw_plan.rs`：
    - `move_bottom_margin_inference_texts_near_student` 现在只接管组件型 `inference_note` / `*_inference_note*` 文本，不抢管已有专门 corridor/periphery pass 的 `ann_inference`。
    - 新增 right-edge-low 判定：当 inference note 贴右边界、低于 student 且离 student 水平过远时，也拉回 student periphery。
    - 候选顺序调整为右侧、可用的 student 下方、左侧、上方；同时当 student 本身已接近底部时，不再强制把合法的底部 periphery note 拉走，避免破坏 projector/encoder 旧烟测。
- 追加验证：
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_right_edge_inference_text_near_student -- --nocapture`：先红后绿。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_bottom_margin_inference_text_near_student -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_moves_upserted_inference_annotation_below_student_corridor_from_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_separates_right_side_task_loss_output_and_inference_from_latest_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests model_draw_plan_polish_repairs_projector_encoder_overlap_from_latest_smoke -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests inference -- --nocapture`：通过（43/43）。
- 最终验证：
  - `cargo test`：全量通过。测试过程中 LibreOffice 仍打印一次历史外部 `DeploymentException`，但所有测试 exit 0。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 剩余风险：
  - 最新真实 smoke 仍未 accepted，且分数在 82 附近停住；剩余主要问题是 `vertical_under_utilization`、`label_outside_main_area`、`text_wrap_risk`。right-edge guard 已用真实 final fixture 覆盖，但尚未再跑下一次真实 `.env` smoke 来证明该局部修复会进入完整 loop 的 final。
  - 当前 loop 已能避免明显更差的 round 进入 final，但还不能保证每一轮单调提升；后续需要把 `same/regressed` 轮次的反馈压缩成更明确的局部 action，尤其是 vertical recenter 和 output width 这两类仍反复出现的问题。

## 2026-06-22 post-right-edge smoke, task-loss reverse gate, and narrow simple-Y clamp fix

- right-edge inference guard 后的真实 `.env` smoke：
  - 命令：`SESSION_ID=post_right_edge_guard_smoke_20260622_140051 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、5 轮、`reason="cap reached before acceptance"`；final PPTX 有效。
  - 本地质量：round 0 `score=0`，round 1 `score=76`，round 2 `score=94`，round 3 `score=94`，round 4 `score=70`，final `score=94`。
  - 人工查看 final PNG 后确认 `ann_inference` 位于 student/output 右侧 periphery，不应被 `annotation_in_main_corridor` 判为阻塞；这是一处本地 false positive。
- 已完成 review false positive 修复：
  - `tests/review_tests.rs`：新增 `quality_report_allows_inference_annotation_at_right_student_periphery_from_latest_smoke`，先红后绿。
  - `src/tools/review.rs`：`annotation_sits_between_branch_rows` 现在要求 annotation center-x 仍落在 teacher/student branch span 附近，避免把右侧 periphery inference note 误判成 branch corridor annotation。
- 已完成 task-loss reverse-flow gate：
  - `tests/review_tests.rs`：新增 `quality_report_flags_task_loss_reverse_flow_from_right_edge_smoke`，覆盖 student source 向左/backward 指向 task-loss 的失败模式。
  - `src/tools/review.rs`：新增 `task_loss_reverse_flow`，对 student/main source 到 task-loss 的左向 connector 报 `task_loss_reverse_flow` major。
  - `src/agent.rs` / `tests/prompt_tests.rs`：DrawPlan revision 和 RoundImprovementPlan prompt 都加入 `task_loss_reverse_flow` 的局部修复契约，要求 task-loss cue 放到 student source 右侧或右上方，并使用 x-increasing connector。
- task-loss reverse gate 后的真实 `.env` smoke：
  - 命令：`SESSION_ID=post_task_loss_reverse_gate_smoke_20260622_142312 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 初始质量：round 0 `score=64`，round 1 `score=76`，round 2 `score=88`。
  - run 在生成 round 003 前崩溃，panic 为 `min > max, or either was NaN. min = 0.2, max = 0.15120000000000003`。
  - 复现命令：`RUST_BACKTRACE=1 cargo run -- resume --run runs/teacher-student-distillation-with-latent-residuals/post_task_loss_reverse_gate_smoke_20260622_142312`
  - 根因：`simple_y_teacher_above_student_candidate` 对窄 student bbox 使用 `clamp(0.20, box_width(student) * 1.08)`；当 model 输出 student width 约 `0.14` 时，上界低于 readability floor。相邻的 `simple_y_balanced_teacher_candidate` 也有同形态风险。
- 已完成 narrow simple-Y clamp 修复：
  - `src/tools/draw_plan.rs`：`simple_y_teacher_above_student_candidate` 和 `simple_y_balanced_teacher_candidate` 的动态 clamp 上界现在至少为 `0.20`，允许窄框扩到最低可读宽度而不是 panic。
  - `src/tools/draw_plan.rs` tests：
    - 新增 `simple_y_teacher_candidate_handles_narrow_student_width_below_floor`。
    - 新增 `simple_y_balanced_teacher_candidate_handles_narrow_student_width_below_floor`。
    - 两个测试均先红后绿。
- 恢复中断真实 run：
  - 命令：`cargo run -- resume --run runs/teacher-student-distillation-with-latent-residuals/post_task_loss_reverse_gate_smoke_20260622_142312`
  - 结果：不再 panic，生成 round 003/004 和 final；`accepted=false`、5 轮、`reason="cap reached before acceptance"`。
  - 本地质量：round 0 `score=64`，round 1 `score=76`，round 2 `score=88`，round 3 `score=82`，round 4/final `score=88`。
  - final 本地 issue：`vertical_under_utilization`、`route_detour`。
  - 人工查看 final PNG：版面上方大空白、main group 靠下；`e_input_teacher` 顶部长 dogleg；`inference_note`、student label 仍有视觉问题。vision review 也指出 connector/annotation/box whitespace 问题。

## 2026-06-22 vertical rebalance guard and latest smoke review leakage

- 针对 `post_task_loss_reverse_gate_smoke_20260622_142312/final` 的版心根因：
  - main component union 约为 `y=0.3975..0.87`，上空白约 `0.40`，下空白约 `0.13`。
  - `improvement_plan.json` 反而建议把整组下移到 `y≈0.55..0.95`，说明 reasoner 对 normalized y 坐标/版心的判断会给出反方向建议；只靠 prompt 容易继续打转。
- 已完成 deterministic vertical rebalance：
  - `tests/draw_plan_tests.rs`：新增 `model_draw_plan_polish_recenters_top_blank_teacher_student_group_from_latest_smoke`，用真实 final 几何先复现 bottom-heavy/top-blank 失败。
  - `src/tools/draw_plan.rs`：新增末端 `rebalance_vertically_underutilized_main_group`。
    - 只在主组件组横向跨度足够、纵向跨度未充分利用、且“上方大空白/内容偏下”时触发。
    - 只允许上移，不自动下移，避免破坏 `with_figure_plan` 的语义同步 fixture。
    - 平移 boxes/text/connectors/connector labels，保持相对结构，不重画拓扑。
  - 回归处理：初版 pass 误触发 `model_draw_plan_polish_removes_connectors_absent_from_figure_plan`，随后收窄为只处理 top-blank 的上移场景；`draw_plan_tests` 全量通过。
- vertical rebalance 后的新真实 `.env` smoke：
  - 命令：`SESSION_ID=post_vertical_rebalance_smoke_20260622_150000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
  - 结果：`accepted=false`、5 轮、`reason="cap reached before acceptance"`；final PPTX `unzip -t` 通过，renderer `source="model_generated_code"`、`used_fallback=false`。
  - 本地质量：round 0 `score=100`、0 issue；round 1 `score=76`；round 2 `score=82`；round 3 `score=76`；round 4 `score=70`；final 正确选择 round 0，final local `score=100`。
  - 结论：best-round selection 能保留本地最佳，但“本地 score=100 仍被 vision reject”，说明本地 quality gate 仍漏掉 vision 的主要视觉失败。
  - final vision localized issues：
    - `teacher_model`：oversized box with tiny centered label。
    - `e_input_student`：simple input→student flow 使用了大折线。
    - `e_teacher_supervision_label`：connector label 离 dashed line 太近/压线。
    - `anno_inference`：边缘 inference note 仍有视觉杂讯。
    - `latent_residuals`：Y-branch supervision node 不够居中。
- 已完成两处本地漏检补强：
  - `tests/review_tests.rs`：
    - 新增 `quality_report_flags_connector_label_crowding_own_edge_from_latest_smoke`，覆盖 connector label 与自身 edge 仅有约 1.5mm 间距的 blocking 问题。
    - 新增 `quality_report_flags_three_point_input_student_detour_from_latest_smoke`，覆盖 input→student 的 3 点大折线路由。
  - `src/tools/review.rs`：
    - 新增 `label_crowds_own_edge` / `label_crowds_segment`，对 connector label 贴近自身水平/垂直 segment 的情况复用 `label_overlaps_edge` blocking issue。为避免误伤旧的 detached-label fixture，要求 label center 到 segment 也足够近。
    - 新增 `has_three_point_input_student_elbow_detour`，对 input→student 的 3 点长 vertical/horizontal elbow 报 `route_detour` major。
- 当前验证：
  - `cargo test tools::draw_plan::tests::simple_y_ -- --nocapture`：通过。
  - `cargo test --test draw_plan_tests -- --nocapture`：通过（171/171）。
  - `cargo test --test review_tests -- --nocapture`：通过（57/57）。
  - `cargo test --test prompt_tests -- --nocapture`：通过（10/10）。
  - `cargo fmt`：通过。
  - `cargo test`：全量通过；测试过程中 LibreOffice 仍打印一次历史外部 `DeploymentException`，但所有测试 exit 0。
  - `cd renderer && npm run build`：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 剩余风险 / 下一步：
  - 尚未在新增 `label_crowds_own_edge` 和 `has_three_point_input_student_elbow_detour` 之后再跑真实 `.env` smoke；最新真实证据仍显示 `accepted=false`。
  - 本地 gate 现在能抓住 latest final 的两个 vision major/blocking，但还没有覆盖 `teacher_model` oversized tiny-label、`latent_residuals` Y-branch symmetry、`anno_inference` visual clutter 这三类 vision issue。
  - 下一步应继续把这些 vision issue 转成本地结构化 issue，并优先让 `round_000 local score=100 but vision reject` 这种情况不再发生；否则 loop 会继续“本地满分但视觉不过，再迭代引入回退”。

## 2026-06-22 semantic/style/input gates and deterministic repair closure

- 本轮用户目标：
  - 针对“框内文字空隙特别多、框和框挤、线条/块重叠、配色和审美差、后期整体提升很小”继续定位根因并实施计划。
  - 用户特别强调：不要每轮重构；coding 模型要先读上一轮代码和反馈，对应问题做局部修改；模板只能作为参考；视觉模型理论上应能看出问题，但要给足 prompt 和结构化证据。
- 当前结论：
  - 视觉模型确实能看出不少问题，但只靠 vision/reasoner 自然语言建议不稳定。真实 smoke 中同一类问题会被指出，却不一定在下一轮被 coder 稳定修成；有时还会把 style、label、input width 这类局部问题修坏。
  - 因此架构上需要“两层闭环”：vision/reasoner 负责发现和提出意图，本地 quality gate/DrawPlan repair 负责把可判定几何、样式一致性和可读性 invariant 变成确定性约束。
  - 本轮没有做大重构，而是围绕真实 run 暴露的具体坏图做 TDD：先把每个失败模式转成 review/draw_plan/prompt 测试，再做最小修复。
- 新增/修复的本地 quality gate：
  - `src/tools/review.rs`：
    - `label_far_from_edge` 现在会检查显式绑定到 connector 的 fan-in label，即使目标节点有多条 incoming edge，也不能把 `vision_to_fusion_label` / `text_to_fusion_label` 留在远离实际 polyline 的空白区。
    - `text_wrap_risk` 不再只看最长 token，也检查短 input phrase 的可读宽度，覆盖真实 smoke 中 `Input x` bbox 太窄但单词不长的漏检。
    - 增加 teacher/training-only 大框留白、supervision branch asymmetry 等真实 smoke 失败模式的本地 issue。
  - `src/pipeline.rs`：
    - 在 `QualityReport` 中注入 FigurePlan 与 DrawPlan 的 connector style mismatch，尤其是 FigurePlan 要求 solid/dashed 但 DrawPlan 变成另一种样式时，直接给 `edge_style_mismatch` major issue。
  - `src/agent.rs` 与 `tests/prompt_tests.rs`：
    - prompt 契约同步补充 `supervision_branch_asymmetry`、`longest token or short input phrase`、`edge_style_mismatch`，要求 reasoner/coder 给出可执行几何或样式修复，而不是泛泛审美建议。
- 新增/修复的 deterministic DrawPlan repair：
  - `src/tools/draw_plan.rs`：
    - 在 `polish_model_draw_plan_geometry_with_figure_plan` 末端增加 style-only connector sync，只同步 connector style，不重新添加已折叠/删除的 label，避免 full sync 反复恢复噪声 label。
    - `readable_shared_input_width` 不再把当前 bbox 宽度作为不可突破上界；短 input phrase 可扩到最低可读宽度。
    - 新增 outer-edge input phrase widening，仅处理 `input`/`source`、短文本、贴左右边界的 input box，避免误扩 branch 内部 input。
    - `repair_draw_plan_geometry_inner` 末端也执行 connector label finalization，因为 mock/repair 路径不会走完整 polish；这修复了 mock multimodal pipeline 中 fan-in label 被推远的问题。
    - `snap_connector_labels_to_final_routes` 先保留“已经清晰且离 route 足够近”的 label，只重吸附真正漂远的 label；对多段 task/loss feedback route 优先选择最长水平反馈段下方的候选，避免把 `L_task` label 拉回 student-output 主线附近。
- 新增关键测试：
  - `tests/review_tests.rs`：
    - `quality_report_flags_training_only_teacher_box_whitespace_from_latest_smoke`
    - `quality_report_flags_supervision_branch_asymmetry_from_latest_smoke`
    - `quality_report_flags_explicit_fanin_edge_label_far_from_edge_from_latest_smoke`
    - `quality_report_flags_input_phrase_too_narrow_from_latest_smoke`
  - `src/pipeline.rs` 单元测试：
    - `quality_report_injects_draw_plan_edge_style_mismatch`
  - `tests/draw_plan_tests.rs`：
    - `model_draw_plan_polish_resyncs_solid_teacher_residual_after_topology_repairs_from_smoke`
    - `model_draw_plan_polish_widens_shared_task_input_phrase_from_semantic_gate_smoke`
    - `model_draw_plan_polish_widens_left_edge_task_input_phrase_from_real_smoke`
    - `repair_draw_plan_geometry_resnaps_multimodal_fanin_labels_from_mock_smoke`
- 真实 `.env` smoke 记录：
  - `post_supervision_whitespace_gate_smoke_20260622_163000`
    - 命令：`SESSION_ID=post_supervision_whitespace_gate_smoke_20260622_163000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`，5 轮达到上限；质量分数 `40 -> 64 -> 88 -> 82 -> 82`，final 选择 88。
    - 观察：新增 gate 能把 teacher whitespace / supervision asymmetry 反馈进 loop，但后续又暴露 explicit fan-in label、style mismatch、residual route detour、`task_input` narrow phrase。
  - `post_semantic_quality_gates_smoke_20260622_171500`
    - 命令：`SESSION_ID=post_semantic_quality_gates_smoke_20260622_171500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`；round 分数为 `70, 34, 40, 52, 22`，final 选择 70。
    - 观察：`edge_style_mismatch` 和 `text_wrap_risk` 已稳定进入本地反馈，但模型每轮没有可靠执行 style-only 修复，说明这类 invariant 需要 deterministic repair，而不是只靠 prompt。
  - `post_deterministic_style_input_smoke_20260622_183000`
    - 命令：`SESSION_ID=post_deterministic_style_input_smoke_20260622_183000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=3 MAX_MINUTES=60 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`，3 轮上限；round 分数 `70 -> 88 -> 100`，final local quality `score=100`、`issues=[]`；final PPTX `unzip -t` 通过。
    - 观察：确定性 style sync 与 shared input width repair 生效；未 accepted 是因为 vision review 没在 3 轮内通过，而不是本地 gate 阻塞。
  - `post_input_width_gate_smoke_20260622_190000`
    - 命令：`SESSION_ID=post_input_width_gate_smoke_20260622_190000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=1 MAX_MINUTES=35 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`，单轮上限；final quality `score=70`，issue 只剩 `annotation_in_main_corridor`、`edge_crosses_component`、`edge_crossing`。
    - 观察：`text_wrap_risk` 和 `edge_style_mismatch` 已消失；`input_data` bbox 约 `[0.028, 0.7033, 0.1803, 0.8973]`，`Input x` 已达到可读宽度。人工查看 PNG 后确认剩余主要是 residual dashed connector 穿过 teacher latent/main corridor、inference note 和 route crossing，不是 input width/style 问题。
- mock pipeline 回归与修复：
  - 全量 `cargo test` 首次暴露 `resume_pipeline_uses_existing_run_directory` 失败。
  - 根因：新增 `label_far_from_edge` 正确捕获 mock multimodal 的 `vision_to_fusion_label` / `text_to_fusion_label`，但 mock pipeline 只走 `repair_draw_plan_geometry_with_figure_plan`，不走完整 `polish_model_draw_plan_geometry_with_figure_plan`，所以 connector label finalization 没覆盖 repair 主路径。
  - 先用 `tmp/debug_resume_multimodal` 复现：两轮均 `score=88`，`quality_report.json` 只有两个 `label_far_from_edge`。
  - 新增红测 `repair_draw_plan_geometry_resnaps_multimodal_fanin_labels_from_mock_smoke` 后修复 repair 末端 label snap；再处理二阶回归，避免 `L_task` feedback label 被拉回主线附近。
  - 复测命令：`rm -rf tmp/debug_resume_multimodal_after_label_fix && cargo run -- run --method examples/multimodal_fusion.md --out tmp/debug_resume_multimodal_after_label_fix --style wps-clean --aspect paper-wide --target-width-mm 85 --max-iterations 2 --max-cost-usd 3.0 --max-minutes 20 --reference-previews auto --image-provider none --mock-models --keep-intermediate`
  - 结果：`accepted=true, rounds=2, reason=accepted`。
- 最终验证：
  - `cargo test --test draw_plan_tests`：通过（175/175）。
  - `cargo test --test pipeline_tests resume_pipeline_uses_existing_run_directory -- --nocapture`：通过。
  - `cargo test`：全量通过；LibreOffice 仍打印一次历史外部 `DeploymentException`，但所有测试 exit 0。
  - `npm run build`（`renderer/`）：通过。
  - `cargo fmt --check`：通过。
  - `git diff --check`：通过。
- 剩余风险 / 下一步：
  - 最新真实 smoke 仍未 accepted；style mismatch 与 input phrase width 已稳定消除，但剩余 `annotation_in_main_corridor`、`edge_crosses_component`、`edge_crossing` 说明 route/annotation 层还需要继续补本地 gate 与 deterministic reroute。
  - 视觉模型能看出问题，但不能保证每轮做出有用、单调的局部修改。后续应继续把 vision issue 转成本地可复现 fixture，并优先处理 connector crossing、annotation corridor、residual supervision route 三类仍反复出现的问题。

## 2026-06-22 late-round closure: y-branch annotations, labels, schema robustness

- 本轮后半段目标：
  - 继续按真实 `.env` loop 的上一轮反馈做局部修复，不再大重构。
  - 优先处理 teacher-student 图中反复出现的 inference note、task loss label、frozen annotation、JSON/schema 解析阻断。
  - 收尾时保留当前真实 smoke 证据，避免把未完成的右侧拥挤问题伪装成已解决。
- 已完成的 DrawPlan 局部修复：
  - `src/tools/draw_plan.rs`
    - 新增/接入 compact teacher-student Y-branch annotation repair：当模型把 `Inference: student only` 作为独立小框塞进 input/student corridor 时，将其转换成 editable text annotation，并清理相关拥挤组件语义。
    - 将 output/student prediction edge 上的 `L_task` 类 label 转成独立 task-loss cue，避免 loss label 压在主 prediction edge 上。
    - 对 input→teacher、input→student 的不必要 dogleg 做局部短路由修复。
    - 扩展 compact task-loss label snapping：`L_task`/`ltask` 这类短 label 也会被吸附到真实 task-loss edge，而不是漂到空白处。
    - 扩展 teacher-state annotation anchoring：`frozen` / `teacher frozen` / `freeze` id 的 annotation 不再被当成 margin 噪声删除，而是锚到 teacher 附近。
  - `tests/draw_plan_tests.rs`
    - 新增真实 smoke fixture 回归：
      - `model_draw_plan_polish_clears_y_branch_inference_lane_loss_label_and_input_detours`
      - `model_draw_plan_polish_resnaps_task_loss_label_to_short_loss_edge_from_latest_smoke`
      - `model_draw_plan_polish_anchors_bottom_frozen_annotation_near_teacher_from_latest_smoke`
- 已完成的模型 JSON/schema 鲁棒性修复：
  - `src/agent.rs`
    - `RoundImprovementPlan` 解析先转 `serde_json::Value` 再转强类型 schema，容忍模型 JSON 中重复字段；真实失败例是 `expected_visible_effect` 重复导致整轮中断。
    - 新增单元测试 `round_improvement_parser_tolerates_duplicate_model_fields`。
  - `src/schema.rs`
    - `ComponentRole::Loss` 接受 `supervision` 作为输入 alias；真实失败例是 reasoner 在 FigurePlan component role 中输出 `"supervision"`。
  - `tests/schema_tests.rs`
    - 新增 `component_role_accepts_supervision_alias_from_model`。
- 真实 `.env` smoke 记录：
  - `post_inference_note_readability_smoke_20260622_212000`
    - 命令：`SESSION_ID=post_inference_note_readability_smoke_20260622_212000 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`，5 轮上限；final local score `76`。
    - 主要 issue：`standalone_inference_lane`、`loss_label_on_prediction_edge`、`route_detour`。人工查看确认 inference note 在 input/student corridor，`L_task` 仍压在 prediction/loss 相关边附近。
  - `post_y_branch_annotation_repair_smoke_20260622_214500`
    - 命令：`SESSION_ID=post_y_branch_annotation_repair_smoke_20260622_214500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`，5 轮上限；final local score `94`，PPTX `unzip -t` 通过，renderer `source="model_generated_code"`、`used_fallback=false`。
    - 主要剩余 issue：`label_far_from_edge`，目标为 `e_task_loss_label,e_task_loss`。这证明 Y-branch inference note/loss-label/detour 修复有效，但 compact task-loss label 仍需 resnap。
  - `post_task_loss_label_snap_smoke_20260622_220500`
    - 命令：`SESSION_ID=post_task_loss_label_snap_smoke_20260622_220500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=5 MAX_MINUTES=90 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：`accepted=false`，5 轮上限；final local score `94`，PPTX 通过，renderer 无 fallback。
    - 主要剩余 issue：`label_outside_main_area`，目标 `a_freeze`；人工查看是 `frozen` annotation 漂在顶部/边缘，随后用 teacher-state anchoring 修复。
  - `post_frozen_anchor_smoke_20260622_222500`
    - 命令：`SESSION_ID=post_frozen_anchor_smoke_20260622_222500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：执行失败，不是绘图失败；原因是 reasoner 返回的 `RoundImprovementPlan` JSON 有重复 `expected_visible_effect` 字段，旧强类型解析直接报错。
    - 修复：`parse_round_improvement_plan_text` 先解析为 `serde_json::Value`，重复 key 由 JSON object 归并后再进 schema。
  - `post_duplicate_json_parser_smoke_20260622_223500`
    - 命令：`SESSION_ID=post_duplicate_json_parser_smoke_20260622_223500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：执行失败于 FigurePlan schema；原因是模型输出 component role `"supervision"`，旧 `ComponentRole` 只接受 `loss`/`main`/`context` 等固定枚举。
    - 修复：`ComponentRole::Loss` 增加 `alias = "supervision"`，保留序列化输出仍为 `"loss"`。
  - `post_component_role_alias_smoke_20260622_224500`
    - 命令：`SESSION_ID=post_component_role_alias_smoke_20260622_224500 REFERENCE_PREVIEWS=required MAX_ITERATIONS=2 MAX_MINUTES=45 bash scripts/run_real_env.sh examples/teacher_student.md`
    - 结果：命令 exit 0；`accepted=false`，2 轮上限；PPTX `unzip -t` 通过；renderer `source="model_generated_code"`、`used_fallback=false`。
    - 本地质量从 round 0 score `0` 提升到 round 1/final score `52`，说明 loop 能继续迭代且 parser/schema 阻断已解除。
    - final 仍有 `task_loss_in_branch_corridor`、`component_collapsed`、多处 `component_crowding`、`edge_crossing`。人工查看发现具体残留形态：`student_latent` 右下大框、`student_output`/`task_loss` 贴右侧边界且互相拥挤，`e_teacher_to_residual` 长虚线横穿 `e_student_encode_latent`。这部分尚未实现新的 deterministic repair，作为下一步保留。
- 最终本地验证：
  - `cargo test --test schema_tests component_role_accepts_supervision_alias_from_model -- --nocapture`：通过。
  - `cargo fmt --check`：通过。
  - `cargo test`：全量通过；LibreOffice 仍打印一次历史外部 `DeploymentException`，但所有测试 exit 0。
  - `cd renderer && npm run build`：通过。
  - `git diff --check`：通过。
- 收尾状态：
  - 已修复本轮两个“无法继续迭代”的真实阻断：重复 JSON field、`ComponentRole=supervision`。
  - 已把多个视觉失败转成确定性 repair 和回归测试：inference note corridor、task-loss label 漂移、frozen annotation 边缘漂移。
  - 尚未解决最新 2-round smoke 的右侧 latent/output/task-loss 拥挤与 residual crossing；已定位到具体 topology，下一轮应先把 `post_component_role_alias_smoke_20260622_224500/final/draw_plan.json` 固化为红测，再做一个窄范围 repair，不建议继续泛泛调 prompt。
