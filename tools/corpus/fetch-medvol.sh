#!/usr/bin/env bash
# Fetch the medical research volume test corpus (NIfTI, Analyze, NRRD) into tools/corpus/medvol/
# (gitignored: nothing here is committed), and write its oracle. The specular-medvol corpus test
# reads it in place from SPECULAR_MEDVOL_CORPUS (a worktree has no corpus of its own), else from
# tools/corpus/medvol, and skips when it is absent.
#
# Usage: tools/corpus/fetch-medvol.sh [SECTION...]
#   SECTION is a function name below (src_nibabel, src_pynrrd, src_parrec, src_speed,
#   src_oracle). Default: src_nibabel, src_pynrrd, src_parrec, then src_oracle. src_speed (about
#   400 MB: a 256^3 int16 .nii.gz and a 512x512x300 float32 .nrrd for open timings) only runs
#   when named. src_parrec writes its own oracle (nibabel only), expected-parrec.tsv.
#   OUT defaults to $SPECULAR_MEDVOL_CORPUS, else tools/corpus/medvol.
#   PYTHON (default python3) must have numpy, nibabel, SimpleITK and pynrrd for the oracle and
#   the speed files. A venv under target/ is fine: python3 -m venv target/medvol-venv &&
#   target/medvol-venv/bin/pip install --no-cache-dir nibabel SimpleITK pynrrd; then
#   PYTHON=target/medvol-venv/bin/python.
#
# What it covers (about 1 MB):
#   nibabel   nibabel's test volumes at 5.3.2 (MIT): NIfTI-1 / -2, gzip, 4-D, an anatomical
#             with a rotated affine, scaled data
#   pynrrd    pynrrd's test volumes at v1.1.3 (MIT): attached and detached headers, raw, gzip,
#             bzip2 (refused), ASCII, byte skip -1 / -5, line skip, 1-D, 2-D, 4-D, custom fields
#   parrec    nibabel's Philips PAR/REC headers (most without their REC data: geometry only)
#             and its ECAT 7 sample, at 5.3.2 (MIT)
#   expected.tsv  the oracle (nibabel for NIfTI, SimpleITK for NRRD) for every file it reads,
#             and the files it refuses
#
# Downloads are untrusted data: each source gets its own new directory, every file is checked
# against its SHA-256 below, and nothing from them is run (the oracle reads them with python -I
# from this repository's script). Stops if free space drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MEDVOL_CORPUS:-$ROOT/tools/corpus/medvol}
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"

die() { echo "fetch-medvol: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# fetch BASEURL DEST LICENCE (SHA256 NAME)...
fetch() {
  local base=$1 dest=$2 lic=$3; shift 3
  mkdir -p "$OUT/$dest"
  while (( $# >= 2 )); do
    local want=$1 name=$2 out="$OUT/$dest/$2"; shift 2
    if [[ ! -s $out ]]; then
      check_space
      curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" "$base/$name"
      mv "$out.part" "$out"
    fi
    echo "$want  $out" | sha256sum -c --quiet - || die "checksum mismatch: $dest/$name"
    provenance "$dest/$name" "$base/$name" "$lic"
  done
}

src_nibabel() {      # nibabel's test data, 5.3.2 (MIT)
  fetch https://raw.githubusercontent.com/nipy/nibabel/7b5060e4bb105ed0c08c203db3dc5cf536d9c2aa/nibabel/tests/data \
    nibabel MIT \
    1c089f37b6597a38bb4157a1e1b3f7f13f1bc9d4e7a8cfdfaf91d85cd8f66594 anatomical.nii \
    42097dfbab9d2a036b41ae5c97a359591cf2cf5c3f8dc6ca6455c0b8a7f22696 example4d.nii.gz \
    a53e59e70eb0d8275a4fe347422a89551aee92d0eb1137a3444b1932a28c3fe2 example_nifti2.nii.gz \
    0591d9f8c21f1a0af46567c47f96307ae8faf6b70771a881f4cc477502af7b26 functional.nii \
    fd54cf0ce7b52935ed63e02490a07c4f5d949ab2572d13d2626001aeecab17cf reoriented_anat_moved.nii \
    1840a0022a316e2acacab3e18e716a15a140f2057ff88b7770a0ab3f9dd31cc3 resampled_anat_moved.nii \
    f9556106668e78b5a3ef56a0e7bcaa06a9d6a4f9a5aab0cd5548e198095a0bbd standard.nii.gz
}

src_pynrrd() {       # pynrrd's test data, v1.1.3 (MIT)
  fetch https://raw.githubusercontent.com/mhe/pynrrd/96bf87b78bde03987c09346ceffe0bb85a8f4214/nrrd/tests/data \
    pynrrd MIT \
    ba83a218c7fa607aa77f275e94c439e608150e0315d5595e6773fbb29cf1d47b BallBinary30x30x30.nhdr \
    ddb5614e1902d76535eefdeb4dcadc63469fc0cd8d62c3d5a96ebd506e621909 BallBinary30x30x30.nrrd \
    283a970d9df9586bf9c7f44175cbf60a845a3991c12a53a120113f1e1c0e8eac BallBinary30x30x30.raw \
    cc24cbf641f5b911d68a8ee945a10086467df06e1c28cd65589d6f133290e656 BallBinary30x30x30_byteskip_minus_five.nhdr \
    f57a009710cb86001c5d391bcefb0bf8e2697d5a93b22ff4b6fcea201c759510 BallBinary30x30x30_byteskip_minus_one.nhdr \
    4c7b636d3e9e7cdc0d07d6c56ce24a87316772df936fd1cc3341d517543274c8 BallBinary30x30x30_bz2.nrrd \
    eb8482379f6c05d1d5023a534f4f6107b2202fd0d6bbaf8e4d4a41d28cd53cae BallBinary30x30x30_gz.nrrd \
    2b118e6a90868ff2e6f0d3a36ebbcb66ed0edc92c3d3ab2142408a595df7dc19 BallBinary30x30x30_gz_byteskip_minus_one.nrrd \
    50576a2ed09f84af56df0aa9730faef131f1e139a201d7434f8b9ce21895f164 BallBinary30x30x30_gz_lineskip.nrrd \
    2148094a458e83337aa4caa0c4d6d4a7e202914469ec3b3d5a7d0798b77b0728 test1d_ascii.nrrd \
    6d102d06e96bb2efc98e5ffa8adcb2bbec3bc1064216840e0393895d72b0ff5f test2d_ascii.nrrd \
    9d3f4b175b6d1fc135fb49fa265003e3a6bfe2c407049c995bcf982d3b6ba630 test_customFields.nrrd \
    60c63673ab6f521316b078a5346bb76e5b6547d0c79d7aebabb03a92075db982 test_simple4d_raw.nrrd
}

src_parrec() {       # nibabel's PAR/REC and ECAT 7 test data, 5.3.2 (MIT)
  fetch https://raw.githubusercontent.com/nipy/nibabel/7b5060e4bb105ed0c08c203db3dc5cf536d9c2aa/nibabel/tests/data \
    parrec MIT \
    1581dd036c4c42d6deead83dcb04bfaaffbc09a6b6648be29de75d41dfaef5f6 ADC_Map.PAR \
    209ac1b2cb1669e833b85d2af96e1fac502b2dc1db2a10c919e2b303e943daf2 ASL_3D_Multiecho.PAR \
    1a9d6f4e3d075afe7d8fa9e2e872fd5cbc506443498ce4c670402d5d73adf6c4 DTI.PAR \
    e52ead532b022ce9926e546d7c3664cd93c204ae277a65701e7bdbd3997a15cb DTIv40.PAR \
    f0479b7b506b7e50bee4cb6595548778a7b2d148c71d729cd92d569c20685bff fieldmap.PAR \
    5f1533b64c3b08838de6198d22e5b3ad76a9f172b91cc4b3d1362e0de9c924b2 NA.PAR \
    5a51133ed7a31e87355a71fe9847259d8a3e929d1f2c15970ce4ee48bd482539 Phantom_EPI_3mm_cor_20APtrans_15RLrot_SENSE_15_1.PAR \
    d2743ff735c14aaba0106d4c54d73dcdeb31597aef5ed735080070880b303196 Phantom_EPI_3mm_cor_SENSE_8_1.PAR \
    73a715fd94ae9d2c8531dad26bb542c3babcd7418f1ec1c9b4a1393d44439007 Phantom_EPI_3mm_sag_15AP_SENSE_13_1.PAR \
    1106b4dab32d40aa77bc47e79a2fdfcac87f9cbbd56ecab5a44de03cb8a7278a Phantom_EPI_3mm_sag_15FH_SENSE_12_1.PAR \
    b4e36a0b85d59aa92b2b22d90860c4d65590ba573465b201a47bbee368074c23 Phantom_EPI_3mm_sag_15RL_SENSE_11_1.PAR \
    14bbd8529e734553330abe2ce1644bc60918c7d785089246ebf3feefa55b9002 Phantom_EPI_3mm_sag_SENSE_7_1.PAR \
    1b71b98e6656bf61a9dcc9e7f061100520a92b564d1e02f254443b0ae5c08d7f Phantom_EPI_3mm_tra_15FH_SENSE_9_1.PAR \
    4423a602c330b1ec6f7920c5c5c0666aa519a03691e0c74838bdc77b78f587b7 Phantom_EPI_3mm_tra_15RL_SENSE_10_1.PAR \
    ff05e688c4c12fe726c17a1eca240664e93aaa85d3922963e6c79740dce5b8ed Phantom_EPI_3mm_tra_-30AP_10RL_20FH_SENSE_14_1.PAR \
    157099a83a3d58e33517c1e22a0f8ef1951b4bc063cfe945528bdd7cb9510240 Phantom_EPI_3mm_tra_SENSE_6_1.PAR \
    f71ea4fc3dc7a22ef1cf1965c03d7750ca917c4c25cacabbc8c3fb65472d73f8 phantom_EPI_asc_CLEAR_2_1.PAR \
    4b28e98e37f64c86578ac4fbba812abe7b010465b68dacdcd98690820e76e705 phantom_fake_dualTR.PAR \
    df78b5ddfd3af0604dfee615b0d34f828c8b0cf10e65917667399c6ae4ac2798 phantom_fake_v4_1.PAR \
    56c8fadce40b36cfd2216e2c2311dfa7f4501f717772d76c6eb80d3b0766c102 phantom_fake_v4.PAR \
    7e2a99972e927b5ff0a5e70654c885838e6562c29c70c705b4dcc0c46849d3ea phantom_truncated.PAR \
    feedb5ed454ceb83b79454004e7059fec43c3be1f16b85a8e7cb8f4cc89a2384 phantom_varscale.PAR \
    12d9a3e7c49177a58836ee655e223ff0eb17c0e79b519c331ba8da372c373297 T1_3echo_mag_real_imag_phase.PAR \
    a3fea4783026fed08de0805f9f04a3c09b39060bf33f1420cb637fbe0d920c30 T1_dual_echo.PAR \
    a935ff6c5c3c143d62558fe18102137c47f9f0e3ea55305b1a9f3530ea0bfaf8 T1.PAR \
    738d1fec4924e72d1b99c6cd1666ebd9de06e8705310cde84af762639f2739ca T2-interleaved.PAR \
    3aeee51705aec1984c4fddc6805a1e5735b1bffe8cc183725474b8683faed154 T2_-interleaved.PAR \
    cc82866cba10615172624263a69a2ee0d094dec37a30adae0e1ee24ac0608a83 T2.PAR \
    d8ba7bd048a2f9c7915e569d3f735da3051295ce27b902ec197e2dbb4e2014e1 T2_.PAR \
    0eecedd11470492c64788077f045d5ed8645f876e6ae0a21a349967ddd2c97b3 umass_anonymized.PAR \
    f1749bd1c6c7ea3c7f9a4ed2b1ee41ef3e085e496f6662fce802254aba9c0985 variant_v4_2_header.PAR \
    6a27cb6d719ed93f5ce2df947c6f8dbfaba5d458cfc7fc985099dbe5424b2aac phantom_EPI_asc_CLEAR_2_1.REC \
    6a27cb6d719ed93f5ce2df947c6f8dbfaba5d458cfc7fc985099dbe5424b2aac phantom_truncated.REC \
    6a27cb6d719ed93f5ce2df947c6f8dbfaba5d458cfc7fc985099dbe5424b2aac phantom_varscale.REC \
    1879f6c555dd77f50e72b69b2984ddf11af3fe4e2156bcd0f10446657e9c1e1f tinypet.v
  # Its oracle (nibabel only), expected-parrec.tsv.
  "$PYTHON" -I "$ROOT/crates/formats/specular-medvol/gen/oracle_parrec.py" "$OUT" "$OUT/expected-parrec.tsv"
}

src_speed() {        # generated volumes for open timings (not downloaded)
  check_space
  "$PYTHON" -I "$ROOT/crates/formats/specular-medvol/gen/make_speed.py" "$OUT/speed"
}

src_oracle() {       # nibabel / SimpleITK over everything fetched
  "$PYTHON" -I "$ROOT/crates/formats/specular-medvol/gen/oracle.py" "$OUT" "$OUT/expected.tsv"
}

main() {
  if (( $# == 0 )); then
    set -- src_nibabel src_pynrrd src_parrec src_oracle
  fi
  : > "$OUT/.provenance.tsv"
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "no section $s"
    "$s"
  done
  echo "fetch-medvol: done ($OUT)"
}

main "$@"
