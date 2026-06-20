#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

method_path="${1:-examples/teacher_student.md}"
export MAX_ITERATIONS="${MAX_ITERATIONS:-0}"
export MAX_MINUTES="${MAX_MINUTES:-0}"

trap 'echo; echo "loop stopped"; exit 130' INT TERM

echo "========== methodfig single-session loop =========="
echo "method: ${method_path}"
echo "started: $(date '+%Y-%m-%d %H:%M:%S')"
echo "max_iterations: ${MAX_ITERATIONS} (0 means until accepted)"
echo "max_minutes: ${MAX_MINUTES} (0 means no time cap)"

bash scripts/run_real_env.sh "$method_path"
