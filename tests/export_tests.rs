use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Mutex, OnceLock};

use methodfig::tools::export::{create_review_images, export_round};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn export_round_invokes_soffice_and_pdftoppm_chain() {
    let _guard = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let bin = temp.path().join("bin");
    let round = temp.path().join("round");
    fs::create_dir_all(&bin).expect("bin");
    fs::create_dir_all(&round).expect("round");
    fs::write(round.join("figure.pptx"), "pptx").expect("pptx");

    write_executable(
        &bin.join("soffice"),
        r#"#!/bin/sh
outdir=""
input=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --outdir)
      shift
      outdir="$1"
      ;;
    *.pptx)
      input="$1"
      ;;
  esac
  shift
done
base="$(basename "$input" .pptx)"
printf '%%PDF-1.4\nfake\n' > "$outdir/$base.pdf"
"#,
    );
    write_executable(
        &bin.join("pdftoppm"),
        r#"#!/bin/sh
prefix=""
for arg in "$@"; do
  prefix="$arg"
done
printf 'fakepng\n' > "$prefix-1.png"
"#,
    );

    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{old_path}", bin.display()));
    let result = export_round(&round, 85, false);
    std::env::set_var("PATH", old_path);

    result.expect("fake export should succeed");
    assert!(round.join("figure.pdf").exists());
    assert!(round.join("figure.png").exists());
    assert!(round.join("figure_85mm_preview.png").exists());
    assert!(round.join("figure_review_overlay.png").exists());
}

#[test]
fn create_review_images_resizes_to_target_paper_width_and_draws_overlay() {
    let temp = tempfile::tempdir().expect("tempdir");
    let round = temp.path();
    let image = image::RgbaImage::from_pixel(200, 100, image::Rgba([255, 255, 255, 255]));
    image.save(round.join("figure.png")).expect("save png");
    fs::write(
        round.join("layout_map.json"),
        serde_json::to_vec(&serde_json::json!({
            "objects": [{"id": "module", "kind": "component", "bbox": [0.1, 0.2, 0.8, 0.7]}]
        }))
        .unwrap(),
    )
    .expect("layout map");

    create_review_images(round, 85).expect("review images");

    let preview = image::open(round.join("figure_85mm_preview.png"))
        .expect("preview")
        .to_rgba8();
    let overlay = image::open(round.join("figure_review_overlay.png"))
        .expect("overlay")
        .to_rgba8();
    assert_eq!(preview.width(), 1004);
    assert_eq!(preview.height(), 502);
    assert_ne!(preview.get_pixel(100, 100), overlay.get_pixel(100, 100));
}

fn write_executable(path: &std::path::Path, content: &str) {
    fs::write(path, content).expect("write script");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}
