#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

method_path="${1:-examples/teacher_student.md}"
out_root="${OUT_ROOT:-runs}"
style="${STYLE:-wps-clean}"
aspect="${ASPECT:-paper-wide}"
target_width_mm="${TARGET_WIDTH_MM:-85}"
max_cost_usd="${MAX_COST_USD:-3.00}"
max_minutes="${MAX_MINUTES:-60}"

if [[ ! -f "$method_path" ]]; then
  echo "method file not found: $method_path" >&2
  exit 1
fi

summary="$(
  awk '
    BEGIN { found = 0 }
    /^[[:space:]]*# +/ && !found {
      sub(/^[[:space:]]*# +/, "")
      print
      found = 1
      exit
    }
    /^[[:space:]]*$/ { next }
    !found {
      print
      found = 1
      exit
    }
  ' "$method_path"
)"

if [[ -z "$summary" ]]; then
  summary="$(basename "${method_path%.*}")"
fi

slug="$(
  printf '%s' "$summary" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^[:alnum:]]+/-/g; s/^-+//; s/-+$//; s/-{2,}/-/g'
)"

if [[ -z "$slug" ]]; then
  slug="$(basename "${method_path%.*}" | tr '[:upper:]' '[:lower:]')"
fi

timestamp="$(date +%Y%m%d_%H%M%S)"
out_dir="${out_root}/${slug}_${timestamp}"

mkdir -p "$out_root"
echo "method: $method_path"
echo "run dir: $out_dir"

cargo run -- run \
  --method "$method_path" \
  --out "$out_dir" \
  --style "$style" \
  --aspect "$aspect" \
  --target-width-mm "$target_width_mm" \
  --max-cost-usd "10000" \
  --max-minutes "10000" \
  --image-provider none \
  --keep-intermediate
