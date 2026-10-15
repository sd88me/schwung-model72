#!/usr/bin/env bash
# Install build/model-72-module.tar.gz on an Ableton Move running Schwung (staged: upload under a temp name, then unpack).
#   scripts/deploy.sh [host, default move.local] [--reboot]
# The native DSP is only loaded at boot, so a first install or a new dsp.so needs a reboot of the Move (--reboot does it; save your set first).
# Does nothing on its own to a device: run it yourself once the Move is on the network. Run scripts/test.sh and scripts/build.sh first.
set -euo pipefail
cd "$(dirname "$0")/.."
HOST="${1:-move.local}"; REBOOT="${2:-}"
[ "$HOST" = "--reboot" ] && { HOST=move.local; REBOOT=--reboot; }
case "$HOST" in 192.168.1.44) echo "192.168.1.44 is the Akai Force, not a Move" >&2; exit 1;; esac
TAR=build/model-72-module.tar.gz
[ -f "$TAR" ] || { echo "build it first: scripts/build.sh" >&2; exit 1; }
DEST=/data/UserData/schwung/modules/sound_generators
scp "$TAR" "ableton@$HOST:/data/UserData/model-72-module.tar.gz.new"
ssh "ableton@$HOST" "set -e
  mv /data/UserData/model-72-module.tar.gz.new /data/UserData/model-72-module.tar.gz
  rm -rf $DEST/model-72
  tar -xzf /data/UserData/model-72-module.tar.gz -C $DEST
  ls -l $DEST/model-72
  touch /data/UserData/schwung/debug_log_on 2>/dev/null || echo \"(could not enable debug logging as ableton; try: ssh root@$HOST touch /data/UserData/schwung/debug_log_on)\""
if [ "$REBOOT" = "--reboot" ]; then ssh "root@$HOST" reboot || true; echo "Rebooting the Move."
else echo "Reboot the Move (ssh root@$HOST reboot) so the new dsp.so is loaded, then add Model-72 to a track from the chain."; fi
echo "Logs: ssh ableton@$HOST 'tail -f /data/UserData/schwung/debug.log'   (grep model-72)"
