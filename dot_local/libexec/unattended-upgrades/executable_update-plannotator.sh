#!/bin/bash

set -euo pipefail

# Keep harness configuration in chezmoi; upstream installs only the binary.
installer="$(curl -fsSL https://raw.githubusercontent.com/backnotprop/plannotator/main/scripts/install.sh)"
bash -c "$installer" -- --minimal </dev/null
