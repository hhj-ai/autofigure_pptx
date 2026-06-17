use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::schema::FigurePlan;
use crate::style::StyleSpec;

pub fn generate_typescript(
    plan: &FigurePlan,
    style: &StyleSpec,
    round_dir: &Path,
    renderer_root: &Path,
    asset_paths: &BTreeMap<String, PathBuf>,
) -> Result<String> {
    let runtime_path = absolutize(renderer_root).join("src/runtime.ts");
    let out_dir = absolutize(round_dir);
    let payload = RenderPayload {
        out_dir,
        plan,
        style,
        asset_paths,
    };
    let payload_json = serde_json::to_string_pretty(&payload)?;
    Ok(format!(
        r#"import {{ createFigureRuntime }} from "{}";

// methodfig generated code. Stable IDs are preserved in layout_map.json.
const payload = {};
async function main() {{
  const runtime = createFigureRuntime(payload);
  await runtime.renderPlan();
}}

main().catch((error) => {{
  console.error(error);
  process.exit(1);
}});
"#,
        escape_ts_path(&runtime_path),
        payload_json
    ))
}

#[derive(Serialize)]
struct RenderPayload<'a> {
    out_dir: PathBuf,
    plan: &'a FigurePlan,
    style: &'a StyleSpec,
    asset_paths: &'a BTreeMap<String, PathBuf>,
}

fn escape_ts_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}
