#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

out_dir="${OUT_DIR:-tmp/pdfs/method_templates}"
svg_dir="$out_dir/svg"

mkdir -p "$svg_dir"

download_pdf() {
  local url="$1"
  local out="$2"
  if [[ ! -f "$out" ]]; then
    curl -L --fail --silent --show-error "$url" -o "$out"
  fi
}

extract_svg_page() {
  local pdf="$1"
  local page="$2"
  local out="$3"
  pdftocairo -f "$page" -l "$page" -svg "$pdf" "$out"
}

download_pdf "https://arxiv.org/pdf/1706.03762" "$out_dir/attention_is_all_you_need.pdf"
download_pdf "https://arxiv.org/pdf/2002.05709" "$out_dir/simclr.pdf"
download_pdf "https://arxiv.org/pdf/2006.11239" "$out_dir/ddpm.pdf"
download_pdf "https://arxiv.org/pdf/1505.04597" "$out_dir/unet.pdf"

extract_svg_page "$out_dir/attention_is_all_you_need.pdf" 3 "$svg_dir/attention_transformer_page3.svg"
extract_svg_page "$out_dir/simclr.pdf" 2 "$svg_dir/simclr_page2.svg"
extract_svg_page "$out_dir/ddpm.pdf" 2 "$svg_dir/ddpm_page2.svg"
extract_svg_page "$out_dir/unet.pdf" 2 "$svg_dir/unet_page2.svg"

echo "Extracted SVG pages:"
find "$svg_dir" -maxdepth 1 -type f -name '*.svg' -print
echo
echo "SHA-256:"
shasum -a 256 "$svg_dir"/*.svg
echo
echo "Embedded bitmap inventory:"
pdfimages -f 3 -l 3 -list "$out_dir/attention_is_all_you_need.pdf"
pdfimages -f 2 -l 2 -list "$out_dir/simclr.pdf"
pdfimages -f 2 -l 2 -list "$out_dir/ddpm.pdf"
pdfimages -f 2 -l 2 -list "$out_dir/unet.pdf"
