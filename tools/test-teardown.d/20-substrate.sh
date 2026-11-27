#!/bin/sh
# Teardown hook: stops the substrate-daemon of tools/test-setup.d/20-substrate.sh by the PID it
# recorded in target/substrate/daemon.pid: SIGTERM, then SIGKILL after 10 s. The PID is signalled
# only while its command line names the daemon's socket, so a PID the kernel gave to another
# process since is left alone. Its transient systemd unit ends with it, and the execs in its
# cgroup with the unit. The daemon's log stays at target/substrate/daemon.log. Succeeds when there
# is nothing to stop.
set -eu

root=$(pwd)
dir=$root/target/substrate
socket=${WEFT_SUBSTRATE_SOCKET:-$dir/sock}
case $socket in /*) ;; *) socket=$root/$socket ;; esac
pidfile=$dir/daemon.pid

[ -f "$pidfile" ] || exit 0
pid=$(cat "$pidfile")
ours() {
  kill -0 "$pid" 2>/dev/null &&
    tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | grep -qx -- "$socket"
}
if [ -n "$pid" ] && ours; then
  kill -TERM "$pid" 2>/dev/null || true
  i=0
  while ours; do
    i=$((i + 1))
    if [ "$i" -gt 100 ]; then
      kill -KILL "$pid" 2>/dev/null || true
      break
    fi
    sleep 0.1
  done
  echo "20-substrate: stopped substrate-daemon $pid"
fi
rm -f "$pidfile" "$socket"
