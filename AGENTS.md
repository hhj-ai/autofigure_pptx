# AGENTS.md

## 项目约束

- 默认用中文向用户汇报；代码标识符、命令、路径和 API 名称保持英文。
- 开始任务前先读 `AGENTS.md`、`README.md`、`goal.md`、`plan.md`，再检查相关代码。
- 多步骤修改必须持续更新 `plan.md`，记录实际执行、失败点、验证命令和剩余问题。
- 本地 `.env` 中配置的 LLM 对此项目视为免费资源。不要因为模型调用成本而回避真实 non-mock 验证；需要验证 agentic loop 时优先用真实 reasoner/coder/vision 路径。
- 不要打印、提交或转述 `.env` 中的 API key、base URL 之外的敏感配置值。
- 即使 LLM 调用可大胆使用，也必须保留安全边界：模型只能通过受控 workspace/manifest 读取和写入 artifacts，不能访问 `.env`、仓库外路径、任意网络、child process 或未声明文件。
- 产品核心不变：输出源格式必须是可编辑 PPTX；语义内容必须是 native PPTX shapes/text/lines。image model 只允许生成小 local assets，不能生成整张 figure、语义文字、公式或箭头。
- method overview 模板必须优先来自 `templates/method_overview/method_templates.json` 这类 PDF/SVG-derived abstract layout grammar；不要继续把核心论文图模板手写死在 Rust 逻辑里。可以抽象经典论文图的 slots/flows/style grammar，但不能把原论文整图作为 full-slide raster 或不可编辑图片贴进 PPTX。

## 验证偏好

- 结构性改动优先使用 TDD：先写能暴露旧行为的测试，再改实现。
- 重要重构至少运行 `cargo fmt --check`、`cargo test`、`cd renderer && npm run build`。
- 当用户要求“试一下”时，优先用真实 `.env` 运行 non-mock smoke，而不是只跑 `--mock-models`。
