#!/usr/bin/env bash
# Fetch the official Opus test vectors (RFC 8251 set) for specular-opus into
# tools/corpus/video2/specular-opus/ (gitignored: nothing here is committed).
#
# Usage: tools/corpus/specular-opus-fetch.sh
#   Then: cargo test -p specular-opus (the vector tests skip themselves when this is absent).
#
# What it holds (about 60 MB unpacked): testvector01..12.bit (opus_demo format: per packet a
# u32 BE length, a u32 BE final range and the packet), the reference decodes testvectorNN.dec
# (16-bit stereo 48 kHz) and testvectorNNm.dec (the same without the CELT 180-degree phase
# shift, RFC 8251 section 10). The archive is SHA-256 pinned.
#
# Downloads are untrusted data: this script only unpacks the archive (tar, no scripts run).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/tools/corpus/video2/specular-opus
URL=https://opus-codec.org/static/testvectors/opus_testvectors-rfc8251.tar.gz
SHA256=6b26a22f9ba87b2b836906a9bb7afec5f8e54d49553b1200382520ee6fedfa55
mkdir -p "$OUT"
avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
if (( avail_kb < 8 * 1024 * 1024 )); then
  echo "specular-opus-fetch: less than 8 GB free; stopping" >&2
  exit 1
fi
tgz=$OUT/opus_testvectors-rfc8251.tar.gz
if [[ ! -f $tgz ]]; then
  curl -sSfL -o "$tgz.part" "$URL"
  mv "$tgz.part" "$tgz"
fi
got=$(sha256sum "$tgz" | cut -d' ' -f1)
if [[ $got != "$SHA256" ]]; then
  echo "specular-opus-fetch: checksum mismatch ($got); delete $tgz to retry" >&2
  exit 1
fi
tar -xzf "$tgz" -C "$OUT" --no-same-owner --no-same-permissions
# The archive unpacks into a subdirectory; keep the files flat.
for d in "$OUT"/*/; do
  if compgen -G "$d/testvector*.bit" >/dev/null; then
    mv -f "$d"/testvector* "$OUT"/
    rmdir "$d" 2>/dev/null || true
  fi
done
ls "$OUT"/testvector*.bit >/dev/null
echo "specular-opus test vectors in $OUT"
