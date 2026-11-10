# macOS notes

Status: **runs, not yet measured.** The workspace builds, passes clippy and passes its test suite
on Apple Silicon (an M3 Max). The performance work below is in place but has not been measured
on a Mac yet; the measurement plan is in [What to measure on a real Mac](#what-to-measure-on-a-real-mac).
Intel Macs are the same code minus the core-cluster facts, and have not been run.

All macOS code sits behind `cfg(target_os = "macos")` in clearly named places. On other systems
every hook is a no-op, so shared code calls them without its own `cfg`:

| Where | What |
|---|---|
| `crates/specular-platform/` | Portable API in `src/lib.rs`; macOS work in `src/macos/{qos,sysctl,file,pressure,power,screen}.rs`. objc2 0.6 family (same as wgpu-hal), `dispatch2`, `libc`. |
| `crates/specular-core/src/qos.rs` + one line in `jobs.rs` | Hook: job priority → thread class, set by each worker before it runs a job. |
| `crates/specular-core/src/source.rs` | `F_RDADVISE` of the whole file when it is mapped. |
| `crates/specular-core/src/hw.rs` | CPU brand and physical cores from sysctl. |
| `crates/specular-gfx/src/macos.rs` | Present mode and drawable count overrides, BC/ASTC features, EDR hooks. |
| `crates/specular-gfx/src/unified.rs` | `MappedUpload`: decoder-writes-into-GPU-memory upload for unified memory (all platforms; not wired in yet). |
| `app/specular/src/main.rs`, `app.rs` | Main-thread QoS, rayon pool QoS, App Nap guard for benchmarks, `--hw-report` lines, prefetch/VRAM budgets under pressure. |
| `.cargo/apple-m2.toml` | Opt-in `-C target-cpu=apple-m2` config (not default). |

`unsafe`: six small FFI calls, each in UNSAFE.md (QoS, sysctl, two `fcntl`, dispatch source
creation, `endActivity`). Everything else (NSProcessInfo, NSScreen, CAMetalLayer via wgpu) uses
safe objc2/wgpu APIs.

Cross-checking from Linux: `cargo check --target aarch64-apple-darwin -p specular
--no-default-features`. The default `mimalloc` feature builds C code, which needs the macOS SDK
headers, so leave it on only on a Mac or with an SDK (osxcross).

## What is implemented

### 1. Threads on P/E cores (QoS)

macOS has no CPU affinity; the **QoS class** is what decides between performance and efficiency
cores on Apple Silicon.

| Thread | Class | How |
|---|---|---|
| Main (UI, render) | `USER_INTERACTIVE` | `main()` first thing. Threads spawned afterwards (gpu-init, fonts) start in it (pthreads inherit the creator's QoS: **verify on device**). |
| Loader workers, job `Immediate`/`Visible`/`Near` | `USER_INITIATED` | `specular-core` `qos::job_starting`, per job, free when unchanged. |
| Loader workers, job `Prefetch` | `UTILITY` | same |
| Loader workers, job `Idle` (thumbnails, indexing) | `BACKGROUND` | same (also throttles their disk I/O) |
| rayon global pool (decoders' `Parallelism::Auto`) | `USER_INITIATED` | `main.rs` builds the global pool with a `start_handler`. |

The decoders' private fixed-size rayon pools (`Parallelism::Threads(n)` in specular-jpeg) inherit
the QoS of the thread that first creates them; a `start_handler` can be added the same way.

`--hw-report` prints the core clusters from `hw.perflevelN.*` ("Performance 4 physical / 4
logical", "Efficiency 4 / 4"). Thread-pool sizing is unchanged (`logical − 1` job workers; rayon
uses all cores); see the measurements below for whether rayon should use only P-cores.

### 2. File I/O

- Files ≥ 64 KiB are memory-mapped (existing `Source`), advised `MADV_SEQUENTIAL`, and now also
  get **`F_RDADVISE` for the whole file** at open: an asynchronous read into the unified buffer
  cache (which backs mmap on macOS), so the decoder's page faults find data already in flight.
  Prefetch opens go through the same path. `SPECULAR_RDADVISE=0` disables it for A/B.
- `Source::prefetch` keeps `MADV_WILLNEED`.
- Hooks for later use: `specular_platform::set_uncached` (`F_NOCACHE`, for one-shot streaming
  reads of huge files such as thumbnailing a folder of RAWs, so they do not evict the page cache;
  it does not affect mmap) and `set_read_ahead` (`F_RDAHEAD` off for random access into tiled TIFF
  / archives).

### 3. GPU / Metal (wgpu 30)

- **Present mode.** Metal offers `Fifo` (vsync) and `Immediate` (`displaySyncEnabled = NO`); no
  `Mailbox`. The existing code already falls back to `Fifo`. In a window, WindowServer composites
  the layer, so `Immediate` should not tear and behaves like mailbox; in native full screen it may.
  Default stays `Fifo`; `SPECULAR_PRESENT=immediate` switches for measurement.
- **Drawables.** wgpu sets `maximumDrawableCount = frame latency + 1`; we use latency 2 (3
  drawables). `SPECULAR_FRAME_LATENCY=1` → 2 drawables, one frame less latency.
- **ProMotion.** `Fifo` on a CAMetalLayer presents at the panel's current rate up to 120 Hz;
  `--hw-report` prints `NSScreen.maximumFramesPerSecond`. The app's `present_now` path (frames
  ≥ 4 ms apart) assumes mailbox; on Metal Fifo it can queue up to two frames at 120 Hz, which is
  what the frame-pacing measurement below checks.
- **Texture compression.** BC and ASTC are requested when the adapter has them (Apple GPUs: both;
  Intel Macs: BC), for the compressed display cache (DESIGN.md §12.2). `--hw-report` lists them.
- **Unified memory.** wgpu's Metal backend creates `MAP_WRITE` buffers in shared storage.
  `specular_gfx::unified::MappedUpload` lets a decoder write finished row bands straight into that
  GPU-visible memory, then records one GPU blit; `Gpu::unified_memory()` says when it applies.
  This removes the CPU copy `write_texture` makes (≈200 MB for a 50 MP RGBA image) **only if the
  decoder writes into it**; copying a finished `Raster` in costs the same as today, so it is not
  wired into the renderer yet.
- **EDR / HDR hooks.** wgpu 30 already drives `wantsExtendedDynamicRangeContent` and the layer
  colour space from `SurfaceConfiguration::color_space`, and reads
  `maximumExtendedDynamicRangeColorComponentValue` through `Surface::display_hdr_info`. For the HDR
  work: `WindowSurface::edr_output()` returns the best EDR format/colour space (Rgba16Float +
  extended linear sRGB, else extended Display P3, else 10-bit PQ) and
  `WindowSurface::edr_headroom()` the live headroom (main thread). `specular_platform::main_screen()`
  gives refresh rate and current/potential EDR headroom without a surface.

### 4. System integration

- **Memory pressure:** a `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` source records Normal/Warn/Critical
  and wakes the event loop; `about_to_wait` re-runs `schedule()`. `prefetch_depth()` halves the
  depth on Warn and makes it 0 on Critical; `cache_budget()` halves/quarters the VRAM budget
  (on unified memory, VRAM is RAM).
- **Thermal / Low Power Mode:** `NSProcessInfo.thermalState` / `isLowPowerModeEnabled` read in
  `prefetch_depth()`: Serious → depth ≤ 1, Critical → 0, Low Power Mode → ≤ 1.
- **App Nap:** `begin_activity()` returns a guard (`UserInitiatedAllowingIdleSystemSleep`, plus
  `LatencyCritical` on request). Held during `--bench-open` now; slideshow and animation playback
  should hold one while playing.

### 5. Cold start

What costs time at launch on macOS, and what we do:

| Factor | Status |
|---|---|
| dyld: number of dylibs/frameworks | System frameworks live in the dyld shared cache (cheap). Third-party dylibs: none (all Rust is static). Check with `otool -L` that only system frameworks are linked; drop unused framework links (arboard/rfd/trash pull AppKit/Foundation only). |
| Metal shader compilation | wgpu translates WGSL → MSL and Metal compiles at pipeline creation. wgpu has no pipeline cache on Metal; Metal's own per-app shader cache makes the second launch warm. First launch after install/OS update pays it. Measure `surface_configured → image_renderer_ready` cold and warm. Possible later win: create pipelines on the gpu-init thread right after the device. |
| Font loading | Already off the critical path: bundled fonts first, system fonts (CoreText enumeration, can be 10–50 ms) on a background thread. |
| GPU switch on dual-GPU Intel MacBooks | `PowerPreference::HighPerformance` wakes the discrete GPU (can cost 100s of ms and battery). `NSSupportsAutomaticGraphicsSwitching` is set in Info.plist. Measure whether `LowPower` (integrated) starts faster there. Apple Silicon has one GPU. |
| QoS | Main thread at USER_INTERACTIVE from the first line (it is DEFAULT when launched from a terminal). |
| Rosetta | An x86_64 build under Rosetta needs macOS 15+ for AVX2 (x86-64-v3); the CPU check explains this. Ship universal or arm64 builds. |

### 6. SIMD

`aarch64-apple-darwin` already defaults to `target-cpu=apple-m1` (NEON, dotprod, fp16, crc, aes,
sha3, LSE), so `std::simd` code in the decoders compiles to NEON with no flags; no global change.
`.cargo/apple-m2.toml` is an opt-in config (adds i8mm, bf16) for local A/B. Intel Macs use the
workspace's x86-64-v3 config. The NEON check is in the measurement list.

## What to measure on a real Mac

Build: `cargo build --release -p specular` (arm64) and `--target x86_64-apple-darwin` on Intel.
Use 5+ runs each; report median and spread. `sudo purge` empties the file cache for cold runs.

1. **Hardware report:** `target/release/specular --hw-report`. Expect: Apple CPU brand, clusters
   (e.g. "Performance 4/4, Efficiency 4/4" on M1), Metal backend, BC+ASTC, unified memory yes,
   "120 Hz max" on ProMotion, EDR potential > 1 on XDR/HDR panels.
2. **Open latency (warm and cold):**
   `specular --bench-open FILE` for erlaufsee (34 MP), goats (105 MB), a 50 MP progressive JPEG.
   Cold: `sudo purge` before each. A/B: `SPECULAR_RDADVISE=0` vs default.
3. **QoS / P-E placement:** same bench with `SPECULAR_LOG=jobs`; in Instruments (System Trace,
   or `sudo powermetrics --samplers cpu_power -i 200`) check decode threads are on P-cores and
   prefetch on either. A/B by commenting the hook in `specular-core/src/qos.rs`. Also try
   `RAYON_NUM_THREADS=<P-cores>` vs all cores for the 50 MP decode (E-cores as stragglers).
4. **Hold benchmark** (key repeat through a folder): `--bench-open FOLDER/first.jpg --bench-hold 50`.
   Check that `Prefetch` at UTILITY does not slow the promoted image; if it does, map
   `Priority::Prefetch` to `UserInitiated` in `specular-core/src/qos.rs`.
5. **Frame pacing / ProMotion:** pan and zoom with `--set performance.frame_time_hud=true` on a
   120 Hz MacBook Pro; Instruments "Metal System Trace" / "Display" for present intervals and
   dropped frames. A/B: `SPECULAR_PRESENT=immediate`, `SPECULAR_FRAME_LATENCY=1`. Expect 8.3 ms
   intervals; input-to-photon one frame lower with latency 1.
6. **Cold start:** `SPECULAR_LOG=startup specular FILE` (first-pixels stage times), first launch
   after `sudo purge` and after a reboot (Metal shader cache cold:
   `rm -rf $(getconf DARWIN_USER_CACHE_DIR)/com.apple.metal/app.specular*` or the binary's dir), and
   warm. `DYLD_PRINT_STATISTICS=1` (or Instruments "App Launch") for pre-main time;
   `otool -L target/release/specular` for the linked framework list.
7. **Memory pressure:** `memory_pressure -S -l warn` then `-l critical` (stock macOS tool) while
   browsing; `SPECULAR_LOG=cache,prefetch` should show "memory pressure: Warn", fewer prefetches,
   evictions.
8. **Thermal / Low Power:** turn on Low Power Mode; prefetch depth should drop to 1
   (`SPECULAR_LOG=prefetch`).
9. **App Nap:** run a benchmark with the window hidden behind another app; timings must match
   the visible run.
10. **NEON:** `cargo rustc --release -p specular-jpeg --lib -- --emit=asm -C codegen-units=1` and
    check the IDCT/colour conversion for `.4s`/`.8h` vector instructions; compare decode time
    with `--config .cargo/apple-m2.toml` on M2+.

## Expected wins (hypotheses)

- QoS: visible decodes never land on E-cores while prefetch runs; prefetch stops stealing P-cores
  from the UI (smoother key-repeat on M-series with 4 E-cores). Battery: background work on E-cores.
- `F_RDADVISE`: cold opens of large files closer to the SSD's sequential bandwidth instead of
  page-fault-sized reads (largest on 100 MB+ files; zero on warm cache).
- App Nap guard: correct benchmark numbers with hidden windows.
- Pressure/thermal: no swap storms on 8 GB Macs; less throttling during long sessions.
- Unified-memory upload (once decoders write into it): one full-image memcpy less per upload.

## Risks

- QoS inheritance assumptions (spawned threads inherit the creator's class) must be confirmed;
  if not, set the class at the top of the gpu-init and font threads too.
- `UTILITY` prefetch may slow navigation to an image whose decode is in flight (see 4).
- `F_RDADVISE` of a whole huge file might compete with the decoder's own faults for I/O, or
  waste bandwidth when only the embedded thumbnail is needed; it is easy to cap (e.g. first
  64 MB) or remove.
- `Immediate` present mode may tear in native full screen; it stays opt-in.
- Rayon global pool built at start-up: a few thread spawns on the critical path (~0.1–0.3 ms).
- Nothing here has linked or run on macOS; first Mac build may surface SDK-level issues
  (framework links, objc2 selector availability on macOS 12).
