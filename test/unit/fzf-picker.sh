#!/usr/bin/env bash
set -euo pipefail
PYTHONDONTWRITEBYTECODE=1 python3 -B "$(dirname "${BASH_SOURCE[0]}")/../helpers/fzf-picker.py"
