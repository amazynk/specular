#!/usr/bin/env bash
# Fetches Lensfun's lens database (data files only, CC BY-SA 3.0) at a pinned commit, checks
# every file's SHA-256 and writes the compressed bundle Specular ships:
#
#   crates/specular-lensfun/data/lensfun-db.zst   the XML files, each as "name\0content\0",
#                                                 in name order, zstd-compressed
#   crates/specular-lensfun/data/COPYING.CC_BY-SA_3.0
#
#   tools/fetch-lensfun.sh            fetch, check, bundle
#   tools/fetch-lensfun.sh --sums     print the name/SHA-256 lines of the pinned commit
#                                     (to update COMMIT and FILES below; review the diff)
#
# Only the data/ directory of the Lensfun repository is read. The library (LGPL) is never
# fetched. See crates/specular-lensfun/data/PROVENANCE.md.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
COMMIT=52cf1a7b0e82e07e0ecaa3391ee5106a6dd1ee54
BASE="https://raw.githubusercontent.com/lensfun/lensfun/$COMMIT/data"
DATA=crates/specular-lensfun/data
WORK="$ROOT/target/lensfun-fetch"
rm -rf "$WORK"
mkdir -p "$WORK/db"

# name sha256
FILES=$(cat <<'EOF'
6x6.xml ca13c2756b32d728675372695c072517a0e38af5eff2ecd3e17599dee7fc58b3
actioncams.xml 79a5da0322574b95690464444c9d9ba1da0e1f827da44fd6bff120a0ec10e4b4
compact-canon.xml bae47e4b3a0f8852641038ae2dcca5b28ed7965e65ca795fd676f9de857c4670
compact-casio.xml ab35433879c4574ef8f97cd4e3d114d7ff8db4e7279da9d502fc9e911b51ee27
compact-fujifilm.xml 58af9ebef2410c428ae2c5d01003d2247330d47d918823e63522b4d897a997d1
compact-kodak.xml 377018ccd74f58e966b37a11e5a8689be4268153bebece9f1f2a0dbfa8234845
compact-konica-minolta.xml 832a7473951ab625e10c48af6cf60f89b57280016d45729e2584cf9e41ad3cc7
compact-leica.xml 8e4676679377576dc19e7900ce86fc69b0248ea1d4538aa22ee1ee09431a3be3
compact-nikon.xml 5393bdb8223e13d21b85818e96571e97c035470b49b0452dfcedc2e1ff512d3b
compact-olympus.xml 7637d469ad54a02ebce9492a9aa9424a2ff1ec07b0398158076f762fdc08389b
compact-panasonic.xml f191fc2a5d49fb50c06f4c399ee8c1a83a05f93a0efaecd352ec5c1479b8e60b
compact-pentax.xml 596fae3edaadcf2d90ae930fc1bec42597ca0b2bd11842e686e1b915f3736390
compact-ricoh.xml 46df96ebc367d5170cb6b7dfc3742093f0ad946264518fc5488573ba052f29d7
compact-samsung.xml 36e3a778046d56385c387d48474cd32a89a5da94d99e640b5ce716ce747f898d
compact-sigma.xml 0ddc265e3e580da05dcd4664d1b9c2464838d74dd02519c826520950c8f1759e
compact-sony.xml 6b1e5ec3710915f04eceffb87eafdfded4b56d5b4bc58d5086ae59f7d3c066f0
contax.xml 5eba12e6fd4913470afb6fe25fee47a263084b6cec830297273909872f47669a
generic.xml cd572d4110981546b2c4cd296c2ff16755b30063fa136d8b06ad2ca13ab4c59f
mil-canon.xml 2de8e9467a6af6f83cc9da834d3d3b94d1aaf54519f60c68ac6bc02780a77516
mil-fujifilm.xml cc8fd1d595bf6b57ea2e4f0bbdc30f5ac8af4867fad29efe445a79c90c2d2177
mil-hasselblad.xml 13e1fa55d36c2ed7d22964c21ce6c21d887bfe4506e840e425da1ce4157c3756
mil-leica.xml c691037e2e0743388473c2a0545e340462c412b7e3b57e451f4f9c89dde2388a
mil-nikon.xml 3814e192a9f599847eefeea6163d26186d8107cb794f33e4724803e476e45cea
mil-olympus.xml 989a3d88ef07a286efc2755df214792fdf71d60366932a1b369a77ad39cf2abb
mil-panasonic.xml eb5f01f33e65721f3900786d5970c7e220dd042aaa56968be33c009a358ff3c5
mil-pentax.xml 675a94f52794868ad9f1d79e17b7128783c34b3db23e9eac346c5963812a16e5
mil-samsung.xml 10130e9e125e0882a16fd3033cfed0175e9c14cb7d35e630cdb94736f27e5dc3
mil-samyang.xml ae3bf262f12facdc6f81b837cf6d36fdde2a930d2bfb0918d5abaa47fcfea3ab
mil-sigma.xml 34082bae4f6b476f8b75ea03d2f88a0485b1c5f7ce6bde87c7d729742f7b81ba
mil-sony.xml f9f7e5d7461ac58f76eb55dcc5b211f2e9a451402ddb4a9ecb12d1e4dc04f2b8
mil-tamron.xml 64077167f06e25b1a9393eaefd8ceb9b288c7ad20b046c2d97c83fbca536b6b6
mil-tokina.xml addb27bd82b0d729644b9da1342f92401f63cc5945309ed17f38f4b7af1649e3
mil-zeiss.xml ef8a238904fc9ea6fdb6171863dfd70cd94814af19ecd8c31ee000ca1864589d
misc.xml 2000a347d9dc37d908ea5b56c760de5b2b4ab475857959ae2e901c31339b01bb
om-system.xml ab7cfcfd4c4023f7d0affd87a971e93a391d4d11952b9bf0436aa7967e68880e
rf-leica.xml f93636e1980ad308d1873cc3ac078cdc77c0d529d777cd45c5c8d0e4781981e3
slr-canon.xml 5244e6df8f2522f829c57e2073313ce31a624e9ed9605310fe43613b9d588c99
slr-hasselblad.xml 97602c467699904bf101179ea2fa24f05f3e1879250498a6ba306cd95a3b7a6b
slr-konica-minolta.xml 5639ca94f121f609247c2c120b65830b3ef7784db6ecfa7e8eb3d29ff5046c1b
slr-leica.xml c5e74cb511712e99ed60a066e85c842fea58f693c5039c3819485619f5293d0e
slr-nikon.xml aa770005b85567721038456183cfdbf6b6cea51b33a399b291f447287b9e91db
slr-olympus.xml f7693b6779edd8ed10d7dc23113669255bfa4987cea7db0e843f7f5f2bbbe615
slr-panasonic.xml cd79acb49d9ce5f2a4cecc572b3e117f03da380bed2dcbfd52327192d8ce2381
slr-pentax.xml 93be1fe9263e06d7d69a8a6ae10cb52f23cad08ddc44f57f749e1ca288bc0c6e
slr-ricoh.xml c835798136a53535fdf4f81312ac9aad0ea3de3ef732c183d8cc4d2f432011d9
slr-samsung.xml dc0a29e424ff96e4316ddeb76126fa6f6c73a8836905a3e1f56f0c493826148f
slr-samyang.xml fb1585bea91f5f59e1fae8811dd76c92252ee5d52e3647df2aff4695048443b5
slr-schneider.xml de607cfe4ba6e7debc61e282ce39b72825c07601572e14af2bc9108ba67faf81
slr-sigma.xml f55f41c144fbb72e3c51d2dfef40e918930d37db1b4150bd02bd746406c036d2
slr-soligor.xml 1aba0aab19bce87ea86175b6551cb5306a512eb54be1698b89991b86008f6a04
slr-sony.xml e5ca7f532afe23ff380bf2f9e5458f687bed603355d73a254d85c4e692942fdc
slr-tamron.xml c62090789cebe6bbb663111c6e97beeb1111852bb735088e29cf8782df4bab45
slr-tokina.xml b1213eda81a613bc3976340398c6b8ce8b7dc29423476fc565d5f8e5cf612ecf
slr-ussr.xml 1fd6acc8a8f40373379bea2899ffd96fc34b22d9b044d0474dabd4c89af44efb
slr-vivitar.xml c5c67e3fe38e8fa19044acadc7730047690435f66aac563f55ffe04997a88111
slr-zeiss.xml 49aa211a41fd5fe391d26af5fa65bdf9428d60863ffff15d3916fa1c520dc101
COPYING.CC_BY-SA_3.0 3f941b3b89cf7b8370ceb83cc76d2120d471b58735d8ca60238a751a48d7f72f
EOF
)

fetch() { # url dest
    echo "Fetching $(basename "$2") from Lensfun ($COMMIT)" >&2
    curl -fL --retry 3 --progress-bar -o "$2" "$1"
}

if [[ "${1:-}" == "--sums" ]]; then
    fetch "https://api.github.com/repos/lensfun/lensfun/contents/data/db?ref=$COMMIT" "$WORK/list.json"
    python3 -I -c 'import json,sys; [print(e["name"]) for e in json.load(open(sys.argv[1])) if e["name"].endswith(".xml")]' \
        "$WORK/list.json" > "$WORK/names"
    while read -r n; do
        fetch "$BASE/db/$n" "$WORK/db/$n"
        echo "$n $(sha256sum "$WORK/db/$n" | cut -d' ' -f1)"
    done < "$WORK/names"
    fetch "$BASE/COPYING.CC_BY-SA_3.0" "$WORK/COPYING"
    echo "COPYING.CC_BY-SA_3.0 $(sha256sum "$WORK/COPYING" | cut -d' ' -f1)"
    exit 0
fi

check() { # file sha
    local got
    got=$(sha256sum "$1" | cut -d' ' -f1)
    [[ "$got" == "$2" ]] || { echo "SHA-256 mismatch for $1: $got, expected $2" >&2; exit 1; }
}

[[ -n "$FILES" ]] || { echo "FILES is empty: run with --sums first" >&2; exit 1; }
while read -r n sum; do
    [[ -z "$n" ]] && continue
    if [[ "$n" == COPYING* ]]; then
        fetch "$BASE/$n" "$WORK/$n"
        check "$WORK/$n" "$sum"
    else
        fetch "$BASE/db/$n" "$WORK/db/$n"
        check "$WORK/db/$n" "$sum"
    fi
done <<< "$FILES"

python3 -I - "$WORK/db" "$WORK/bundle" <<'EOF'
import os, sys
src, out = sys.argv[1:3]
with open(out, "wb") as o:
    for n in sorted(os.listdir(src)):
        o.write(n.encode() + b"\0")
        o.write(open(os.path.join(src, n), "rb").read() + b"\0")
EOF
mkdir -p "$DATA"
zstd -q -19 -f "$WORK/bundle" -o "$DATA/lensfun-db.zst"
cp "$WORK/COPYING.CC_BY-SA_3.0" "$DATA/COPYING.CC_BY-SA_3.0"
echo "bundle sha256 $(sha256sum "$WORK/bundle" | cut -d' ' -f1) ($(stat -c %s "$WORK/bundle") bytes)"
echo "wrote $DATA/lensfun-db.zst ($(stat -c %s "$DATA/lensfun-db.zst") bytes)"
rm -rf "$WORK"
