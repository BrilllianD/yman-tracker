#!/usr/bin/env bash
# A config with a `blocked` status, which `--waits-on` needs.
set -eu
yman=$2
source "$(dirname "$0")/../../v2-config.sh"
