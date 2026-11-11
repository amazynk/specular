#!/usr/bin/env bash
# Installs Ubuntu packages on a CI runner, retrying. Hosted runners sometimes cannot resolve the
# Ubuntu mirrors for a while ("Temporary failure resolving 'azure.archive.ubuntu.com'"); a few
# tries some time apart get through instead of failing the job before it starts.
#
#   tools/ci/apt-install.sh PACKAGE...
set -euo pipefail

tries=5
opts=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
for ((try = 1; try <= tries; try++)); do
    # A stalled mirror would hang the job until its timeout: each try is cut off.
    if sudo timeout 300 apt-get update --error-on=any "${opts[@]}" &&
        sudo timeout 600 apt-get install -y --no-install-recommends "${opts[@]}" "$@"; then
        exit 0
    fi
    ((try < tries)) || break
    echo "apt-get failed (try $try of $tries); trying again in $((try * 20)) s" >&2
    sleep $((try * 20))
done
echo "apt-get failed $tries times" >&2
exit 1
