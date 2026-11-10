#!/bin/sh
# Fetches the public VP8 test vectors (vp80-00-comprehensive-*, vp80-01..06-*, with their
# per-frame .md5 lists) into tools/corpus/video2/specular-vp8/vectors/ (gitignored) for
# crates/formats/specular-vp8/tests/vectors.rs.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
dst="$here/video2/specular-vp8/vectors"
base=https://storage.googleapis.com/downloads.webmproject.org
mkdir -p "$dst"
list=$(curl -sSf "$base/?prefix=test_data/libvpx/vp80-&max-keys=1000" |
    grep -o '<Key>test_data/libvpx/vp80-[A-Za-z0-9_.-]*</Key>' |
    sed 's/<Key>test_data\/libvpx\///; s/<\/Key>//')
[ -n "$list" ] || { echo "no vectors listed" >&2; exit 1; }
for f in $list; do
    case "$f" in *.ivf|*.ivf.md5) ;; *) continue ;; esac
    [ -s "$dst/$f" ] && continue
    curl -sSf -o "$dst/$f.part" "$base/test_data/libvpx/$f"
    mv "$dst/$f.part" "$dst/$f"
done
echo "$(ls "$dst" | grep -c '\.ivf$') vectors in $dst"
