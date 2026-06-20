#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

method_path="${1:-examples/teacher_student.md}"
out_root="${OUT_ROOT:-runs}"
style="${STYLE:-wps-clean}"
aspect="${ASPECT:-paper-wide}"
target_width_mm="${TARGET_WIDTH_MM:-85}"
max_cost_usd="${MAX_COST_USD:-10000}"
max_minutes="${MAX_MINUTES:-60}"
max_iterations="${MAX_ITERATIONS:-3}"
reference_previews="${REFERENCE_PREVIEWS:-auto}"

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

session_id="${SESSION_ID:-$(date +%Y%m%d_%H%M%S)}"
task_dir="${out_root}/${slug}"
out_dir="${RUN_DIR:-${task_dir}/${session_id}}"

mkdir -p "$(dirname "$out_dir")"
echo "method: $method_path"
echo "task: $slug"
echo "session: $session_id"
echo "run dir: $out_dir"
echo "reference_previews: $reference_previews"

if [[ -f "$out_dir/config_snapshot.json" ]]; then
  cmd=(cargo run -- resume --run "$out_dir")
else
  cmd=(
    cargo run -- run
    --method "$method_path"
    --out "$out_dir"
    --style "$style"
    --aspect "$aspect"
    --target-width-mm "$target_width_mm"
    --max-iterations "$max_iterations"
    --max-cost-usd "$max_cost_usd"
    --max-minutes "$max_minutes"
    --reference-previews "$reference_previews"
    --image-provider none
    --keep-intermediate
  )
fi

if "${cmd[@]}"
then
  update_symlink() {
    local target="$1"
    local link_path="$2"
    if [[ -L "$link_path" || ! -e "$link_path" ]]; then
      rm -f "$link_path"
      ln -s "$target" "$link_path"
    else
      echo "latest path exists and is not a symlink, leaving unchanged: $link_path" >&2
    fi
  }

  mkdir -p "$out_root"
  out_root_abs="$(cd "$out_root" && pwd)"
  out_dir_abs="$(cd "$out_dir" && pwd)"
  if [[ "$out_dir_abs" == "$out_root_abs"/* ]]; then
    global_latest_target="${out_dir_abs#"$out_root_abs"/}"
  else
    global_latest_target="$out_dir_abs"
  fi
  update_symlink "$global_latest_target" "${out_root}/latest"

  if [[ "$out_dir" == "$task_dir/"* ]]; then
    update_symlink "$(basename "$out_dir")" "${task_dir}/latest"
  fi

  final_dir="$out_dir/final"
  final_dir_abs="$(cd "$final_dir" && pwd)"
  pptx_abs="$final_dir_abs/figure.pptx"
  png_abs="$final_dir_abs/figure.png"
  status_abs="$final_dir_abs/status.json"

  echo "final dir: $final_dir_abs"
  echo "pptx: $pptx_abs"
  echo "png: $png_abs"
  echo "status: $status_abs"
  echo "latest: ${out_root}/latest"
  echo "latest pptx: $repo_root/${out_root}/latest/final/figure.pptx"
  if [[ "$out_dir" == "$task_dir/"* ]]; then
    echo "task latest: ${task_dir}/latest"
    echo "task latest pptx: $repo_root/${task_dir}/latest/final/figure.pptx"
  fi
  echo "open pptx: open \"$pptx_abs\""
else
  status=$?
  echo "run failed with exit code $status" >&2
  echo "generated pptx files under this run, if any:" >&2
  find "$out_dir" -maxdepth 3 -type f -name 'figure.pptx' -print >&2 || true
  exit "$status"
fi
