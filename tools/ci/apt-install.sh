#!/usr/bin/env bash
# Installs Ubuntu packages on a CI runner, retrying. Hosted runners sometimes cannot resolve the
# Ubuntu mirrors for a while ("Temporary failure resolving 'azure.archive.ubuntu.com'"); a few
# tries some time apart get through instead of failing the job before it starts.
#
#   tools/ci/apt-install.sh PACKAGE...
set -euo pipefail

tries=5
for ((try = 1; try <= tries; try++)); do
    if sudo apt-get update --error-on=any -o Acquire::Retries=3 &&
        sudo apt-get install -y --no-install-recommends -o Acquire::Retries=3 "$@"; then
        exit 0
    fi
    ((try < tries)) || break
    echo "apt-get failed (try $try of $tries); trying again in $((try * 20)) s" >&2
    sleep $((try * 20))
done
echo "apt-get failed $tries times" >&2
exit 1
