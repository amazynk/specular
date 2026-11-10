#!/usr/bin/env bash
# Cargo runner for test and bench binaries (Linux): the Vulkan loader is told not to load the
# drivers for GPUs this machine does not have, as the app does in `main`
# (`specular_gfx::gpu::vulkan_drivers_to_skip`). Without it every test binary that makes a wgpu
# instance loads and unloads every installed driver (intel, nouveau, virtio, gfxstream...).
#
# Unlike the app, lavapipe (`lvp`) stays: the goldens render on it.
#
# The runner for x86_64 Linux in .cargo/config.toml; `tools/ci/gdb-runner.sh` sources it, so
# a runner set in CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER should too.
# Only binaries under a `deps/` folder (cargo test and bench) are touched; `cargo run` starts
# the app, which decides for itself. A driver choice already made (any of the loader's
# variables set) is left alone.
#
#   tools/ci/vk-runner.sh <binary> [args...]

vk_drivers_to_skip() {
    local v
    for v in VK_LOADER_DRIVERS_SELECT VK_LOADER_DRIVERS_DISABLE VK_DRIVER_FILES VK_ICD_FILENAMES; do
        [[ -n "${!v+x}" ]] && return 1
    done
    local kernel=() d l
    for d in /sys/class/drm/renderD*; do
        [[ -e "$d" ]] || continue
        l=$(readlink "$d/device/driver" 2>/dev/null) || continue
        kernel+=("${l##*/}")
    done
    # Vulkan driver families and the kernel drivers they run on. Kept equal to FAMILIES in
    # crates/specular-gfx/src/gpu.rs (its test `the_test_runner_knows_every_family`).
    # families:begin
    local families=(
        "*radeon*:amdgpu,radeon"
        "*intel*:i915,xe"
        "*nouveau*:nouveau"
        "*asahi*:asahi"
        "*broadcom*:v3d,vc4"
        "*freedreno*:msm"
        "*panfrost*:panfrost,panthor"
        "*powervr*:powervr,pvrsrvkm"
        "*virtio*:virtio_gpu,virtio-pci"
        "*gfxstream*:virtio_gpu,virtio-pci"
        "*nvidia*:nvidia"
    )
    # families:end
    local known=0 skip=() f glob ks k present
    for f in "${families[@]}"; do
        glob=${f%%:*}
        ks=",${f#*:},"
        present=0
        for k in "${kernel[@]}"; do
            [[ "$ks" == *",$k,"* ]] && present=1
        done
        if ((present)); then
            known=1
        else
            skip+=("$glob")
        fi
    done
    ((known)) || return 1
    local IFS=,
    printf '%s' "${skip[*]}"
}

# Sets VK_LOADER_DRIVERS_DISABLE for the binary at $1 (see above).
vk_runner_env() {
    local skip
    if [[ "$(uname -s)" == Linux && "$1" == */deps/* ]] && skip=$(vk_drivers_to_skip); then
        export VK_LOADER_DRIVERS_DISABLE="$skip"
    fi
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    vk_runner_env "$1"
    exec "$@"
fi
