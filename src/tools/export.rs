use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use image::{imageops::FilterType, Rgba};
use serde::Deserialize;

pub fn export_round(round_dir: &Path, target_width_mm: u32, allow_placeholder: bool) -> Result<()> {
    let pptx = round_dir.join("figure.pptx");
    if !pptx.exists() {
        return Err(anyhow!("cannot export missing PPTX: {}", pptx.display()));
    }

    let real_export = command_path("soffice").and_then(|soffice| {
        let status = Command::new(soffice)
            .arg("--headless")
            .arg("--convert-to")
            .arg("pdf")
            .arg("--outdir")
            .arg(round_dir)
            .arg(&pptx)
            .status()
            .context("failed to run soffice")?;
        if !status.success() {
            return Err(anyhow!("soffice failed with status {status}"));
        }
        let pdf = round_dir.join("figure.pdf");
        if !pdf.exists() {
            return Err(anyhow!("soffice did not create figure.pdf"));
        }
        let pdftoppm = command_path("pdftoppm")?;
        let prefix = round_dir.join("figure");
        let status = Command::new(pdftoppm)
            .arg("-png")
            .arg("-r")
            .arg("300")
            .arg(&pdf)
            .arg(&prefix)
            .status()
            .context("failed to run pdftoppm")?;
        if !status.success() {
            return Err(anyhow!("pdftoppm failed with status {status}"));
        }
        let page_png = round_dir.join("figure-1.png");
        if !page_png.exists() {
            return Err(anyhow!("pdftoppm did not create figure-1.png"));
        }
        fs::copy(&page_png, round_dir.join("figure.png"))?;
        create_review_images(round_dir, target_width_mm)?;
        Ok(())
    });

    match real_export {
        Ok(()) => Ok(()),
        Err(error) if allow_placeholder => {
            write_placeholder_exports(round_dir, target_width_mm, &error.to_string())
        }
        Err(error) => Err(error),
    }
}

fn preview_path(round_dir: &Path, target_width_mm: u32) -> PathBuf {
    round_dir.join(format!("figure_{target_width_mm}mm_preview.png"))
}

pub fn create_review_images(round_dir: &Path, target_width_mm: u32) -> Result<()> {
    let figure_path = round_dir.join("figure.png");
    let image = match image::open(&figure_path) {
        Ok(image) => image.to_rgba8(),
        Err(error) => {
            fs::copy(&figure_path, preview_path(round_dir, target_width_mm))?;
            fs::copy(&figure_path, round_dir.join("figure_review_overlay.png"))?;
            tracing::warn!("failed to process review image, copied original instead: {error}");
            return Ok(());
        }
    };

    let target_width_px = target_width_px(target_width_mm);
    let scale = target_width_px as f64 / image.width().max(1) as f64;
    let target_height_px = ((image.height() as f64 * scale).round() as u32).max(1);
    let preview = image::imageops::resize(
        &image,
        target_width_px,
        target_height_px,
        FilterType::Lanczos3,
    );
    preview.save(preview_path(round_dir, target_width_mm))?;

    let mut overlay = preview.clone();
    if let Ok(layout_map) = read_layout_map(&round_dir.join("layout_map.json")) {
        draw_layout_boxes(&mut overlay, &layout_map.objects);
    }
    overlay.save(round_dir.join("figure_review_overlay.png"))?;
    Ok(())
}

fn target_width_px(target_width_mm: u32) -> u32 {
    ((target_width_mm as f64 / 25.4) * 300.0).round().max(1.0) as u32
}

fn read_layout_map(path: &Path) -> Result<LayoutMap> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn draw_layout_boxes(image: &mut image::RgbaImage, objects: &[LayoutObject]) {
    let color = Rgba([194, 65, 12, 255]);
    for object in objects {
        let [x1, y1, x2, y2] = object.bbox;
        let left = (x1.clamp(0.0, 1.0) * image.width() as f64).round() as i32;
        let top = (y1.clamp(0.0, 1.0) * image.height() as f64).round() as i32;
        let right = (x2.clamp(0.0, 1.0) * image.width() as f64).round() as i32;
        let bottom = (y2.clamp(0.0, 1.0) * image.height() as f64).round() as i32;
        draw_rect(image, left, top, right, bottom, color);
    }
}

fn draw_rect(
    image: &mut image::RgbaImage,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    color: Rgba<u8>,
) {
    for x in left..=right {
        put_pixel_checked(image, x, top, color);
        put_pixel_checked(image, x, bottom, color);
    }
    for y in top..=bottom {
        put_pixel_checked(image, left, y, color);
        put_pixel_checked(image, right, y, color);
    }
}

fn put_pixel_checked(image: &mut image::RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
        image.put_pixel(x as u32, y as u32, color);
    }
}

#[derive(Deserialize)]
struct LayoutMap {
    #[serde(default)]
    objects: Vec<LayoutObject>,
}

#[derive(Deserialize)]
struct LayoutObject {
    bbox: [f64; 4],
}

fn write_placeholder_exports(round_dir: &Path, target_width_mm: u32, reason: &str) -> Result<()> {
    fs::write(
        round_dir.join("figure.pdf"),
        format!("%PDF-1.4\n% methodfig placeholder export: {reason}\n"),
    )?;
    let png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGPQz5//HwAESwI9NWkaoQAAAABJRU5ErkJggg==")?;
    fs::write(round_dir.join("figure.png"), &png)?;
    fs::write(preview_path(round_dir, target_width_mm), &png)?;
    fs::write(round_dir.join("figure_review_overlay.png"), &png)?;
    Ok(())
}

pub fn command_path(command: &str) -> Result<PathBuf> {
    let output = Command::new("which")
        .arg(command)
        .output()
        .with_context(|| format!("failed to locate {command}"))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    Err(anyhow!("required command not found: {command}"))
}
