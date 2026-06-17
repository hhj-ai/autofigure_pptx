use std::process::Command;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::tools::export::command_path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub severity: CheckSeverity,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CheckSeverity {
    Info,
    Warning,
    Error,
}

impl DoctorReport {
    pub fn has_errors(&self) -> bool {
        self.checks
            .iter()
            .any(|check| !check.ok && check.severity == CheckSeverity::Error)
    }

    pub fn to_human_string(&self) -> String {
        self.checks
            .iter()
            .map(|check| {
                let status = if check.ok { "ok" } else { "missing" };
                format!("[{status}] {}: {}", check.name, check.message)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn run_doctor() -> Result<DoctorReport> {
    let mut checks = Vec::new();
    for command in ["node", "npm", "soffice", "pdftoppm"] {
        match command_path(command) {
            Ok(path) => checks.push(DoctorCheck {
                name: command.to_string(),
                ok: true,
                severity: CheckSeverity::Error,
                message: path.display().to_string(),
            }),
            Err(error) => checks.push(DoctorCheck {
                name: command.to_string(),
                ok: false,
                severity: CheckSeverity::Error,
                message: error.to_string(),
            }),
        }
    }

    for font in font_names_to_check() {
        checks.push(check_font(font));
    }

    let config = AppConfig::from_env()?;
    for (name, configured) in [
        ("reasoner model", config.reasoner.is_configured()),
        ("coder model", config.coder.is_configured()),
        ("vision model", config.vision.is_configured()),
        ("image model", config.image.is_configured()),
    ] {
        checks.push(DoctorCheck {
            name: name.to_string(),
            ok: configured,
            severity: CheckSeverity::Warning,
            message: if configured {
                "configured in environment".to_string()
            } else {
                "not configured; use --mock-models for local dry runs".to_string()
            },
        });
    }

    Ok(DoctorReport { checks })
}

pub fn font_names_to_check() -> Vec<&'static str> {
    vec![
        "Microsoft YaHei",
        "DengXian",
        "SimHei",
        "SimSun",
        "Arial",
        "Calibri",
    ]
}

fn check_font(font: &str) -> DoctorCheck {
    let output = Command::new("fc-match").arg(font).output();
    match output {
        Ok(output) if output.status.success() => {
            let message = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let matched = message.to_lowercase().contains(&font.to_lowercase());
            DoctorCheck {
                name: format!("font {font}"),
                ok: matched,
                severity: CheckSeverity::Warning,
                message: if matched {
                    message
                } else {
                    format!("fontconfig resolved to fallback instead of requested font: {message}")
                },
            }
        }
        _ => DoctorCheck {
            name: format!("font {font}"),
            ok: false,
            severity: CheckSeverity::Warning,
            message: "fontconfig unavailable or font not found; WPS/Windows users may still have this font".to_string(),
        },
    }
}
