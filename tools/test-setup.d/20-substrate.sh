#!/bin/sh
# Setup hook: one substrate-daemon for this test run (docs/implementation.md A6), the confinement
# the tests of product/cloud/crates/weft-platform start their execs under, and the node the
# weft-agent processes of the join tests observe.
#
#   1. The daemon: substrate-daemon of release $version from PATH when it answers that version;
#      otherwise the release archive below, checked against its SHA-256 and unpacked into
#      target/substrate/$version/.
#   2. On a GitHub Actions runner (GITHUB_ACTIONS=true) the runner is prepared as root, here and
#      not in the workflow, so the local gate and the forge run the same steps: bubblewrap and
#      /usr/bin/socat (the daemon's startup probe needs both); where Ubuntu restricts user
#      namespaces (kernel.apparmor_restrict_unprivileged_userns=1), the shipped profile
#      bwrap-userns-restrict unloaded and an AppArmor profile that grants them to /usr/bin/bwrap
#      alone loaded in its place; and a systemd user manager for the
#      runner user (linger), which step 3 asks for a delegated cgroup.
#   3. The daemon runs as a transient unit of the user's systemd manager with Delegate=yes: a
#      cgroup v2 subtree it owns, no root. Its first command moves itself into the child group
#      daemon/, so the delegation root stays free of processes, enables cpu, memory and pids for
#      the children and records its PID in target/substrate/daemon.pid; then it becomes the
#      daemon, with --cgroup-root on that root. RuntimeMaxSec bounds a daemon whose teardown never
#      ran. The hook waits for the socket file (at most $wait_limit s), then asks the daemon
#      (GET /v1/machine) for the exec floor its startup probe proved: the namespaces, no egress
#      and read-only host files. It fails when the socket does not appear, the daemon ends or the
#      floor is withheld, and then prints the daemon's log, the host's confinement facts
#      (bubblewrap, socat, the user-namespace sysctls, AppArmor profiles, the unit's cgroup), one
#      bubblewrap run as the daemon starts it, and the machine facts.
#
# The socket is $WEFT_SUBSTRATE_SOCKET when it is set, else target/substrate/sock: the value the
# root .cargo/config.toml ([env]) gives the tests in the same variable. State, workspaces and the
# log (daemon.log) are under target/substrate/; a second run while the daemon lives leaves it.
# tools/test-teardown.d/20-substrate.sh stops it by its PID.
set -eu

version=0.29.0
asset=substrate-daemon-$version-x86_64-unknown-linux-musl.tar.gz
sha256=163b214aba056950c2be1519eb947371497916997b53de6e8363fcaa1ac95bd4
url=https://github.com/beyond10x/substrate/releases/download/$version/$asset
wait_limit=30

root=$(pwd)
dir=$root/target/substrate
socket=${WEFT_SUBSTRATE_SOCKET:-$dir/sock}
case $socket in /*) ;; *) socket=$root/$socket ;; esac
pidfile=$dir/daemon.pid
log=$dir/daemon.log
run=$dir/run

say() { echo "20-substrate: $*"; }
fail() { echo "20-substrate: $*" >&2; exit 1; }

# A Unix socket path holds at most 107 bytes.
[ "$(printf %s "$socket" | wc -c)" -le 107 ] || fail "the socket path $socket is longer than 107 bytes"

mkdir -p "$dir"
chmod 700 "$dir"

alive() {
  [ -f "$pidfile" ] || return 1
  pid=$(cat "$pidfile")
  [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null &&
    tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | grep -qx -- "$socket"
}
if alive && [ -S "$socket" ]; then
  say "substrate-daemon $(cat "$pidfile") already serves $socket"
  exit 0
fi

# 1. The pinned daemon.
daemon=
if command -v substrate-daemon >/dev/null 2>&1 &&
  [ "$(substrate-daemon --version 2>/dev/null)" = "substrate-daemon $version" ]; then
  daemon=$(command -v substrate-daemon)
else
  daemon=$dir/$version/substrate-daemon
  if ! { [ -x "$daemon" ] && [ "$("$daemon" --version 2>/dev/null)" = "substrate-daemon $version" ]; }; then
    say "installing $asset"
    rm -rf "$dir/download"
    mkdir -p "$dir/download"
    curl -fsSL --retry 3 --max-time 60 -o "$dir/download/$asset" "$url" ||
      fail "the download of $url failed"
    echo "$sha256  $dir/download/$asset" | sha256sum -c - >/dev/null ||
      fail "$asset does not have the pinned SHA-256 $sha256"
    tar -xzf "$dir/download/$asset" -C "$dir/download"
    mkdir -p "$dir/$version"
    install -m 0755 "$dir/download/substrate-daemon-$version-x86_64-unknown-linux-musl/substrate-daemon" "$daemon"
    rm -rf "$dir/download"
  fi
fi
say "$("$daemon" --version) at $daemon"

# 2. A GitHub Actions runner gets what a workstation has.
uid=$(id -u)
if [ "${GITHUB_ACTIONS:-}" = true ]; then
  need=
  [ -x /usr/bin/bwrap ] || need="$need bubblewrap"
  [ -x /usr/bin/socat ] || need="$need socat"
  if [ -n "$need" ]; then
    say "installing$need"
    # shellcheck disable=SC2086
    sudo apt-get -o DPkg::Lock::Timeout=60 install -y -q --no-install-recommends $need >/dev/null ||
      { sudo apt-get -o DPkg::Lock::Timeout=60 update -q >/dev/null &&
        sudo apt-get -o DPkg::Lock::Timeout=60 install -y -q --no-install-recommends $need >/dev/null; } ||
      fail "apt-get could not install$need"
  fi
  if [ "$(sysctl -n kernel.apparmor_restrict_unprivileged_userns 2>/dev/null || echo 0)" = 1 ]; then
    # Ubuntu 26.04 loads bwrap-userns-restrict, which also attaches to /usr/bin/bwrap and leaves
    # it no capability in its namespace (bwrap: "loopback: Failed RTM_NEWADDR: Operation not
    # permitted"). It is unloaded so weft-bwrap is the one profile of /usr/bin/bwrap. The
    # restriction stays on for every other binary: kernel.apparmor_restrict_unprivileged_userns=0
    # would lift it for all of them.
    restrict=/etc/apparmor.d/bwrap-userns-restrict
    # A profile not loaded makes apparmor_parser -R fail; that is the state wanted, and the
    # readiness check below fails the hook if bubblewrap still cannot confine.
    if [ -f "$restrict" ]; then
      if sudo apparmor_parser -R "$restrict" 2>/dev/null; then
        say "unloaded the AppArmor profiles of $restrict"
      else
        say "the AppArmor profiles of $restrict are not loaded"
      fi
    fi
    if [ ! -f /etc/apparmor.d/weft-bwrap ]; then
      say "granting user namespaces to /usr/bin/bwrap (AppArmor profile weft-bwrap)"
      printf '%s\n' 'abi <abi/4.0>,' 'include <tunables/global>' \
        'profile weft-bwrap /usr/bin/bwrap flags=(unconfined) {' '  userns,' '}' |
        sudo tee /etc/apparmor.d/weft-bwrap >/dev/null
      sudo apparmor_parser -r -W /etc/apparmor.d/weft-bwrap
    fi
  fi
  sudo loginctl enable-linger "$(id -un)"
  i=0
  while [ ! -S "/run/user/$uid/bus" ]; do
    i=$((i + 1))
    [ "$i" -le 300 ] || fail "no systemd user manager bus at /run/user/$uid/bus after linger"
    sleep 0.1
  done
fi

for tool in /usr/bin/bwrap /usr/bin/socat systemd-run curl; do
  command -v "$tool" >/dev/null 2>&1 || fail "$tool is missing; the daemon cannot confine without it"
done
XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/run/user/$uid}
DBUS_SESSION_BUS_ADDRESS=${DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}
export XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS

# 3. The daemon in a delegated cgroup, one per repository root.
unit=weft-substrate-$(printf %s "$root" | sha256sum | cut -c1-12)
systemctl --user stop "$unit.service" >/dev/null 2>&1 || true
systemctl --user reset-failed "$unit.service" >/dev/null 2>&1 || true
rm -f "$socket" "$pidfile"
rm -rf "$run"
mkdir -p "$run/workspaces"
chmod 700 "$run"
: > "$log"

# systemd expands $$ and ${...} in the command line of a unit, so the script uses neither: the
# shell's own PID comes from /proc/self/stat (read is a builtin, no child), and "0" written to
# cgroup.procs moves the writing shell.
# shellcheck disable=SC2016
inner='set -eu
pidfile=$1 log=$2
shift 2
read -r pid _ < /proc/self/stat
cg=$(sed -n "s/^0:://p" /proc/self/cgroup)
top=/sys/fs/cgroup$cg
mkdir "$top/daemon"
echo 0 > "$top/daemon/cgroup.procs"
echo "+cpu +memory +pids" > "$top/cgroup.subtree_control"
echo "$pid" > "$pidfile"
exec "$@" --cgroup-root "$top" >> "$log" 2>&1'

systemd-run --user --quiet --collect --unit "$unit" -p Delegate=yes -p RuntimeMaxSec=900 \
  -- /bin/sh -c "$inner" sh "$pidfile" "$log" \
  "$daemon" --socket "$socket" --state "$run/state.db" --workspaces "$run/workspaces" \
  --deployment personal --allow-uid "$uid" ||
  fail "systemd-run could not start the daemon as a delegated user unit"

# What the host offers for confinement, printed when the daemon cannot run an exec.
diagnose() {
  # Each probe below may fail; that is what it reports, so a failure must not end the hook here.
  set +e
  {
    echo "--- daemon log ($log)"
    tail -n 60 "$log" || true
    echo "--- host"
    uname -r
    /usr/bin/bwrap --version 2>&1 || true
    /usr/bin/socat -V 2>/dev/null | sed -n 2p || true
    for key in kernel.apparmor_restrict_unprivileged_userns kernel.unprivileged_userns_clone \
      user.max_user_namespaces; do
      echo "$key=$(sysctl -n "$key" 2>/dev/null || echo absent)"
    done
    echo "apparmor profiles naming bwrap or userns: $(ls /etc/apparmor.d 2>/dev/null | grep -i -E 'bwrap|userns' | tr '\n' ' ')"
    if [ "${GITHUB_ACTIONS:-}" = true ]; then
      echo "loaded: $(sudo cat /sys/kernel/security/apparmor/profiles 2>&1 | grep -i -E 'bwrap|userns' | tr '\n' ' ')"
    fi
    cg=$(systemctl --user show -p ControlGroup --value "$unit.service" 2>/dev/null || true)
    echo "unit cgroup: ${cg:-none}"
    if [ -n "$cg" ]; then
      echo "  controllers: $(cat "/sys/fs/cgroup$cg/cgroup.controllers" 2>&1)"
      echo "  subtree_control: $(cat "/sys/fs/cgroup$cg/cgroup.subtree_control" 2>&1)"
      echo "  root procs: $(tr '\n' ' ' < "/sys/fs/cgroup$cg/cgroup.procs" 2>&1)"
    fi
    echo "--- bubblewrap as the daemon starts it (user namespace, nested ones disabled), in a user unit"
    systemd-run --user --quiet --collect --wait --pipe -- /usr/bin/bwrap --unshare-user \
      --disable-userns --assert-userns-disabled --unshare-ipc --unshare-pid --unshare-net \
      --unshare-uts --new-session --die-with-parent --ro-bind /usr /usr --ro-bind-try /bin /bin \
      --ro-bind-try /lib /lib --ro-bind-try /lib64 /lib64 --proc /proc --dev /dev --tmpfs /tmp \
      /usr/bin/true </dev/null 2>&1
    echo "exit $?"
    echo "--- socat's report of a vsock connect (the daemon's probe reads it under its seccomp)"
    /usr/bin/socat -u OPEN:/dev/null VSOCK-CONNECT:2:1234 </dev/null 2>&1 | tail -n 2
    echo "--- machine facts"
    printf '%s\n' "$1"
  } >&2
}

i=0
while [ ! -S "$socket" ]; do
  i=$((i + 1))
  if [ "$i" -gt $((wait_limit * 10)) ]; then
    diagnose ""
    fail "no socket at $socket after ${wait_limit}s"
  fi
  if [ -f "$pidfile" ] && ! kill -0 "$(cat "$pidfile")" 2>/dev/null; then
    diagnose ""
    fail "substrate-daemon ended before it served $socket"
  fi
  sleep 0.1
done

# Ready means the daemon publishes the exec floor its probe proved: the namespaces, no egress
# and read-only host files. Without them every exec is refused (exec.sandbox-unavailable).
machine=$(curl -fsS --max-time 10 --unix-socket "$socket" http://localhost/v1/machine 2>&1) || {
  diagnose "$machine"
  fail "the daemon does not answer GET /v1/machine on $socket"
}
for fact in '"exec.namespaces":{' '"exec.no-egress":true' '"exec.read-only-files":{'; do
  case $machine in
    *"$fact"*) ;;
    *)
      diagnose "$machine"
      fail "the daemon cannot run a confined exec: its probe withheld $fact"
      ;;
  esac
done
say "substrate-daemon $(cat "$pidfile") serves $socket with the exec floor (unit $unit, log $log)"
