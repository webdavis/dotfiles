#!/usr/bin/env bash
set -euo pipefail
python3 -B "$(dirname "${BASH_SOURCE[0]}")/../helpers/fzf-git.py"
