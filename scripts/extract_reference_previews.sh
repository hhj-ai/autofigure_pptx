#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

out_dir="${OUT_DIR:-templates/method_overview/reference_figures/assets}"
pdf_dir="${PDF_DIR:-tmp/reference_figures/pdfs}"
mkdir -p "$out_dir" "$pdf_dir"

download_pdf() {
  local url="$1"
  local out="$2"
  if [[ ! -f "$out" ]]; then
    curl -L --fail --silent --show-error "$url" -o "$out"
  fi
}

render_page_preview() {
  local pdf="$1"
  local page="$2"
  local out_png="$3"
  local prefix="${out_png%.png}"
  pdftoppm -f "$page" -l "$page" -singlefile -png -r 160 "$pdf" "$prefix"
}

download_pdf "https://arxiv.org/pdf/2002.05709" "$pdf_dir/simclr.pdf"
download_pdf "https://arxiv.org/pdf/2010.11929" "$pdf_dir/vit.pdf"
download_pdf "https://arxiv.org/pdf/2103.00020" "$pdf_dir/clip.pdf"
download_pdf "https://arxiv.org/pdf/1810.04805" "$pdf_dir/bert.pdf"

render_page_preview "$pdf_dir/simclr.pdf" 2 "$out_dir/simclr_contrastive_y_branch.png"
render_page_preview "$pdf_dir/vit.pdf" 3 "$out_dir/vit_patch_transformer.png"
render_page_preview "$pdf_dir/clip.pdf" 2 "$out_dir/clip_dual_encoder_contrastive.png"
render_page_preview "$pdf_dir/bert.pdf" 2 "$out_dir/bert_pretrain_finetune.png"

if command -v python3 >/dev/null 2>&1; then
  python3 - "$out_dir" <<'PY'
from pathlib import Path
import math
import sys

try:
    from PIL import Image, ImageDraw, ImageFont
except Exception:
    print("Pillow is not available; skipped synthetic NeurIPS award preview", file=sys.stderr)
    raise SystemExit(0)

out_path = Path(sys.argv[1]) / "neurips_2025_gated_attention_award.png"
out_path.parent.mkdir(parents=True, exist_ok=True)
img = Image.new("RGB", (1360, 760), "#f8fafc")
d = ImageDraw.Draw(img)

def font(path, size):
    try:
        return ImageFont.truetype(path, size)
    except Exception:
        return ImageFont.load_default()

title_font = font("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 42)
head_font = font("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 30)
body_font = font("/System/Library/Fonts/Supplemental/Arial.ttf", 24)
small_font = font("/System/Library/Fonts/Supplemental/Arial.ttf", 20)

d.text((56, 42), "Gated Attention Reference Grammar", fill="#0f172a", font=title_font)
d.text(
    (58, 95),
    "Architecture-first award-style method overview: baseline block -> minimal gate -> measurable effects",
    fill="#475569",
    font=body_font,
)

def rounded_rect(box, fill, outline="#334155", width=4, radius=28):
    d.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)

def center_text(box, lines, fill="#0f172a", spacing=8):
    x1, y1, x2, y2 = box
    rendered = []
    total = 0
    for line, f in lines:
        bb = d.textbbox((0, 0), line, font=f)
        w = bb[2] - bb[0]
        h = bb[3] - bb[1]
        rendered.append((line, f, w, h))
        total += h
    total += spacing * (len(rendered) - 1)
    y = y1 + (y2 - y1 - total) / 2
    for line, f, w, h in rendered:
        d.text((x1 + (x2 - x1 - w) / 2, y), line, fill=fill, font=f)
        y += h + spacing

def arrow(x1, y1, x2, y2, label=None):
    d.line((x1, y1, x2, y2), fill="#1e293b", width=7)
    angle = math.atan2(y2 - y1, x2 - x1)
    for delta in (2.55, -2.55):
        hx = x2 + 26 * math.cos(angle + delta)
        hy = y2 + 26 * math.sin(angle + delta)
        d.line((x2, y2, hx, hy), fill="#1e293b", width=7)
    if label:
        bb = d.textbbox((0, 0), label, font=small_font)
        w = bb[2] - bb[0] + 24
        h = bb[3] - bb[1] + 14
        mx = (x1 + x2) / 2 - w / 2
        my = (y1 + y2) / 2 - h / 2 - 34
        d.rounded_rectangle((mx, my, mx + w, my + h), radius=10, fill="#ffffff", outline="#cbd5e1", width=2)
        d.text((mx + 12, my + 7), label, fill="#475569", font=small_font)

baseline = (78, 270, 338, 520)
gate = (500, 230, 800, 560)
effects = (995, 205, 1284, 590)
rounded_rect(baseline, "#dbeafe")
rounded_rect(gate, "#fee2e2")
rounded_rect(effects, "#dcfce7")
center_text(baseline, [("Softmax", head_font), ("attention", head_font), ("baseline", body_font)])
center_text(gate, [("Insert", head_font), ("learned gate", head_font), ("sparse nonlinear path", body_font)])
center_text(effects, [("Effects", head_font), ("less attention sink", body_font), ("better stability", body_font), ("sparse activations", body_font)])
arrow(338, 395, 500, 395, "minimal change")
arrow(800, 395, 995, 395, "evidence")

x = 188
for text, fill in [
    ("intervention location visible", "#fee2e2"),
    ("compact ablation chips", "#e0f2fe"),
    ("no full-model clutter", "#ede9fe"),
]:
    bb = d.textbbox((0, 0), text, font=small_font)
    w = bb[2] - bb[0] + 34
    d.rounded_rectangle((x, 612, x + w, 656), radius=18, fill=fill, outline="#cbd5e1", width=2)
    d.text((x + 17, 623), text, fill="#334155", font=small_font)
    x += w + 24

for i in range(4):
    y = 300 + i * 42
    d.rounded_rectangle((120, y, 296, y + 24), radius=8, fill="#eff6ff", outline="#93c5fd", width=2)
for i in range(3):
    d.ellipse((560 + i * 62, 315, 600 + i * 62, 355), fill="#ffffff", outline="#ef4444", width=4)
    d.line((580 + i * 62, 355, 580 + i * 62, 475), fill="#ef4444", width=4)
for i in range(3):
    y = 300 + i * 70
    d.rounded_rectangle((1045, y, 1232, y + 42), radius=12, fill="#ffffff", outline="#86efac", width=3)

img.save(out_path)
PY
else
  echo "python3 not found; skipped synthetic NeurIPS award preview" >&2
fi

echo "Generated read-only reference previews under $out_dir"
find "$out_dir" -maxdepth 1 -type f -name '*.png' -print
echo
echo "Note: downloaded PDFs are cached under $pdf_dir and are not part of the tracked template pack."
