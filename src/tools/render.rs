use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

pub fn scan_generated_typescript(code: &str) -> Result<()> {
    let lower = code.to_lowercase();
    let forbidden = [
        "child_process",
        " from \"http\"",
        " from 'http'",
        " from \"https\"",
        " from 'https'",
        " from \"net\"",
        " from 'net'",
        " from \"dns\"",
        " from 'dns'",
        " from \"fs\"",
        " from 'fs'",
        "require(\"fs\")",
        "require('fs')",
        "process.env",
        "fetch(",
        "import(\"net\")",
        "import('net')",
        "import(\"child_process\")",
        "import('child_process')",
    ];

    if forbidden.iter().any(|needle| lower.contains(needle)) {
        return Err(anyhow!(
            "unsafe generated TypeScript rejected by static scan"
        ));
    }

    for import in collect_static_imports(code) {
        if !is_allowed_generated_import(&import) {
            return Err(anyhow!(
                "unsafe generated TypeScript import is not an allowed local runtime/helper import: {import}"
            ));
        }
    }

    Ok(())
}

pub fn validate_generated_runtime_contract(code: &str) -> Result<()> {
    let forbidden_methods = [
        "getDrawPlan",
        "getSlide",
        "getPptx",
        "getPresentation",
        "track",
        "write",
    ];
    for method in forbidden_methods {
        let needle = format!(".{method}(");
        if code.contains(&needle) {
            return Err(anyhow!(
                "generated TypeScript violates DrawPlan runtime contract: unsupported runtime method {method}; use createDrawPlanRuntimeFromEnv() followed by runtime.renderDrawPlan()"
            ));
        }
    }
    Ok(())
}

fn collect_static_imports(code: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let mut current: Option<String> = None;

    for line in code.lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        if let Some(import) = current.as_mut() {
            import.push(' ');
            import.push_str(line);
            if line.ends_with(';') {
                imports.push(import.clone());
                current = None;
            }
            continue;
        }
        if line.starts_with("import ") {
            if line.ends_with(';') {
                imports.push(line.to_string());
            } else {
                current = Some(line.to_string());
            }
        }
    }

    if let Some(import) = current {
        imports.push(import);
    }
    imports
}

fn is_allowed_generated_import(import: &str) -> bool {
    if import.contains("renderer/src/runtime") || import.contains("./runtime") {
        return true;
    }

    let Some(source) = import_source(import) else {
        return false;
    };
    if !source.starts_with("./") {
        return false;
    }
    let local = &source[2..];
    !local.is_empty()
        && !local.contains('/')
        && !local.contains('\\')
        && !local.contains("..")
        && (local.ends_with(".ts") || !local.contains('.'))
}

fn import_source(import: &str) -> Option<&str> {
    let source_start = if let Some(from_index) = import.rfind(" from ") {
        from_index + " from ".len()
    } else {
        "import ".len()
    };
    let source = import[source_start..].trim().trim_end_matches(';').trim();
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
}

pub fn run_node_renderer(
    code: &str,
    round_dir: &Path,
    renderer_root: &Path,
    timeout: Duration,
    allow_placeholder: bool,
) -> Result<()> {
    scan_generated_typescript(code)?;
    validate_generated_runtime_contract(code)?;
    fs::create_dir_all(round_dir)?;
    let figure_ts = round_dir.join("figure.ts");
    fs::write(&figure_ts, code)?;
    let figure_ts = figure_ts
        .canonicalize()
        .context("failed to canonicalize generated figure.ts path")?;
    let round_dir = round_dir
        .canonicalize()
        .context("failed to canonicalize round directory")?;

    let tsx_bin = renderer_root.join("node_modules/.bin/tsx");
    let result = if tsx_bin.exists() {
        run_with_timeout(
            Command::new(tsx_bin)
                .arg(&figure_ts)
                .current_dir(&round_dir)
                .env("METHODFIG_RENDER_OUT_DIR", &round_dir)
                .env(
                    "METHODFIG_RENDER_PAYLOAD_PATH",
                    round_dir.join("renderer_payload.json"),
                )
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
            timeout,
        )
    } else {
        Err(anyhow!(
            "renderer dependency missing: {}",
            tsx_bin.display()
        ))
    };

    match result {
        Ok(output) if output.status.success() => {
            let pptx = round_dir.join("figure.pptx");
            let layout_map = round_dir.join("layout_map.json");
            if !pptx.exists() || !layout_map.exists() {
                return Err(anyhow!(
                    "renderer did not create figure.pptx and layout_map.json"
                ));
            }
            fs::write(
                round_dir.join("figure.ts.log"),
                String::from_utf8_lossy(&output.stdout).as_bytes(),
            )?;
            Ok(())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if allow_placeholder {
                write_placeholder_render(&round_dir, &format!("Node renderer failed: {stderr}"))?;
                Ok(())
            } else {
                Err(anyhow!("Node renderer failed: {stderr}"))
            }
        }
        Err(error) => {
            if allow_placeholder {
                write_placeholder_render(&round_dir, &error.to_string())?;
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

pub fn run_node_renderer_with_fallback(
    primary_code: &str,
    fallback_code: &str,
    round_dir: &Path,
    renderer_root: &Path,
    timeout: Duration,
    allow_placeholder: bool,
) -> Result<()> {
    match run_node_renderer(
        primary_code,
        round_dir,
        renderer_root,
        timeout,
        allow_placeholder,
    ) {
        Ok(()) => Ok(()),
        Err(primary_error) => {
            fs::create_dir_all(round_dir)?;
            fs::write(
                round_dir.join("figure.model_error.log"),
                primary_error.to_string(),
            )?;
            run_node_renderer(
                fallback_code,
                round_dir,
                renderer_root,
                timeout,
                allow_placeholder,
            )
            .with_context(|| {
                format!("model-generated renderer failed, and deterministic fallback also failed; model error: {primary_error}")
            })
        }
    }
}

fn run_with_timeout(command: &mut Command, _timeout: Duration) -> Result<std::process::Output> {
    let timeout = _timeout;
    let mut child = command.spawn().context("failed to run Node renderer")?;
    let start = std::time::Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child
                .wait_with_output()
                .context("failed to collect Node renderer output");
        }
        if start.elapsed() >= timeout {
            child.kill().ok();
            let output = child
                .wait_with_output()
                .context("failed to collect timed-out Node renderer output")?;
            return Err(anyhow!(
                "Node renderer exceeded timeout of {} seconds; stderr: {}",
                timeout.as_secs(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn write_placeholder_render(round_dir: &Path, reason: &str) -> Result<()> {
    fs::write(
        round_dir.join("figure.pptx"),
        format!("methodfig placeholder pptx; renderer unavailable: {reason}\n"),
    )?;
    fs::write(
        round_dir.join("layout_map.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "objects": [],
            "placeholder": true,
            "reason": reason
        }))?,
    )?;
    fs::write(round_dir.join("figure.ts.log"), reason)?;
    Ok(())
}

pub fn default_renderer_root() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("renderer"))
}
