#!/usr/bin/env bash
# Cargo runner that runs a test binary under gdb in batch mode, so a crash (SIGSEGV, SIGBUS,
# SIGILL, an abort) prints the faulting address, the instructions there and every thread's
# backtrace instead of only "signal: 11". A clean run exits with the binary's own status.
#
#   CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER=$PWD/tools/ci/gdb-runner.sh \
#       cargo test -p specular --bin specular
#
# Then grep the output for "===FATAL===". Costs about 1.2-2x wall time (load 20-90).
# Needs gdb on PATH; writes nothing (no core files). Backtraces name functions; build with
# CARGO_PROFILE_DEV_DEBUG=1 for file and line too.
set -euo pipefail
# The drivers for absent GPUs stay unloaded, as with the default runner.
# shellcheck source-path=SCRIPTDIR
. "$(dirname "${BASH_SOURCE[0]}")/vk-runner.sh"
vk_runner_env "$1"
cmds=$(mktemp "${CARGO_TARGET_DIR:-${TMPDIR:-/tmp}}/gdb-runner.XXXXXX")
trap 'rm -f "$cmds"' EXIT
cat >"$cmds" <<'EOF'
set pagination off
set print thread-events off
set print inferior-events off
handle SIGPIPE nostop noprint pass
handle SIGUSR1 nostop noprint pass
handle SIGUSR2 nostop noprint pass
handle SIGCHLD nostop noprint pass
handle SIGALRM nostop noprint pass
handle SIGPROF nostop noprint pass
handle SIGWINCH nostop noprint pass
handle SIG32 nostop noprint pass
handle SIG33 nostop noprint pass
handle SIG34 nostop noprint pass
handle SIG35 nostop noprint pass
run
if $_isvoid($_exitcode)
  echo \n===FATAL===\n
  p $_siginfo.si_signo
  p $_siginfo._sifields._sigfault.si_addr
  x/3i $pc
  info registers rip rsp
  bt 60
  thread apply all bt 25
  kill
  quit 139
else
  quit $_exitcode
end
EOF
gdb -q -nx -batch -x "$cmds" --args "$@"
