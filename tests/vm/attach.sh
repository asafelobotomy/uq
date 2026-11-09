#!/usr/bin/env bash
# Run driver functions (lib.sh) against the VM a `run.sh --keep` (or --boot-only) left running:
#   tests/vm/attach.sh vgh hyprctl layers -j
#   tests/vm/attach.sh shot mytest          # -> that run's directory
#   tests/vm/attach.sh vm_quit
#   tests/vm/attach.sh eval 'run_checks "^L1$"; print_summary'   # re-run checks on the kept VM
set -uo pipefail
UQVM_DIR=$(cd "$(dirname "$0")" && pwd)
. "$UQVM_DIR/lib.sh"
env_file=$UQVM_HOME/last-run.env
[ -f "$env_file" ] || die "no kept VM ($env_file missing)"
# shellcheck disable=SC1090
. "$env_file"
RUN_DIR=$VM_DIR
RESULTS=${RESULTS:-$RUN_DIR/results.tsv}
vm_running || die "the kept VM is not running"
if [ -f "$UQVM_DIR/checks/common.sh" ]; then
    . "$UQVM_DIR/checks/common.sh"
    # shellcheck disable=SC1090
    for f in "$UQVM_DIR"/checks/[0-9]*.sh; do [ -f "$f" ] && . "$f"; done
fi
"$@"
