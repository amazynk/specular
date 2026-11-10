//! Measures which post-entropy decode stages are worth moving to the GPU on this machine:
//! `cargo run --release -p specular-gfx --example decode_stages`
//!
//! 1. **Colour** (YCbCr → RGB, chroma upsampling): CPU conversion + RGBA upload versus plane
//!    upload + GPU conversion (what `specular-render` does), 4:2:0 and 4:4:4.
//! 2. **IDCT** (dequantise + IDCT): coefficients uploaded and transformed by a compute shader
//!    versus uploading the samples the CPU IDCT produced. The CPU IDCT time itself comes from the
//!    JPEG decoder's profile (it is fused into the entropy decode, see the lane report).
//! 3. **BCn**: CPU decode to RGBA (`specular-texture`) + RGBA upload versus uploading the blocks
//!    to a BC texture and converting by sampling it (needs `TEXTURE_COMPRESSION_BC`).
//!
//! Every time is wall clock from the start of the work until the GPU has finished (best of
//! `RUNS`, default 5), on the device `Gpu::new` opens.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use specular_gfx::Gpu;
use specular_gfx::wgpu;
use std::time::Instant;
use wgpu::util::DeviceExt;

fn runs() -> usize {
    std::env::var("RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
}

fn best(mut f: impl FnMut() -> f64) -> f64 {
    f();
    (0..runs()).map(|_| f()).fold(f64::INFINITY, f64::min)
}

fn wait(gpu: &Gpu) {
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Pseudo-random bytes (xorshift), smooth enough to look like image data where it matters.
fn noise(n: usize, seed: u64) -> Vec<u8> {
    let mut s = seed | 1;
    (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s & 31) as usize + (i / 7) % 200) as u8
        })
        .collect()
}

fn tex(
    gpu: &Gpu,
    w: u32,
    h: u32,
    f: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: f,
        usage,
        view_formats: &[],
    })
}

fn write(gpu: &Gpu, t: &wgpu::Texture, data: &[u8], bytes_per_row: u32, rows: u32) {
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: t,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(rows),
        },
        t.size(),
    );
}

const CONVERT: &str = r#"
@group(0) @binding(0) var y: texture_2d<f32>;
@group(0) @binding(1) var cb: texture_2d<f32>;
@group(0) @binding(2) var cr: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
@group(0) @binding(4) var out: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(5) var<uniform> div: vec4<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    let size = textureDimensions(y);
    if g.x >= size.x || g.y >= size.y { return; }
    let p = vec2<f32>(g.xy);
    let l = textureLoad(y, vec2<i32>(g.xy), 0).r;
    let cdim = vec2<f32>(textureDimensions(cb));
    let uv = ((p + 0.5) / div.xy) / cdim;
    let u = textureSampleLevel(cb, samp, uv, 0.0).r - 0.5019608;
    let v = textureSampleLevel(cr, samp, uv, 0.0).r - 0.5019608;
    let rgb = vec3<f32>(l + 1.402 * v, l - 0.344136 * u - 0.714136 * v, l + 1.772 * u);
    textureStore(out, vec2<i32>(g.xy), vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
"#;

const RGBA_COPY: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var out: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    let size = textureDimensions(src);
    if g.x >= size.x || g.y >= size.y { return; }
    textureStore(out, vec2<i32>(g.xy), textureLoad(src, vec2<i32>(g.xy), 0));
}
"#;

/// One 8×8 block per workgroup: dequantise, separable float IDCT, level shift, pack 4 samples
/// per u32 into a plane (`stride` u32 per row).
const IDCT: &str = r#"
struct P { blocks_w: u32, stride: u32, _a: u32, _b: u32 };
@group(0) @binding(0) var<storage, read> coef: array<u32>;
@group(0) @binding(1) var<storage, read> quant: array<u32, 64>;
@group(0) @binding(2) var<storage, read_write> plane: array<u32>;
@group(0) @binding(3) var<uniform> p: P;

var<workgroup> s: array<f32, 64>;
var<workgroup> t: array<f32, 64>;
var<workgroup> o: array<u32, 64>;

fn c(k: u32, n: u32) -> f32 {
    let a = select(1.0, 0.70710678, k == 0u);
    return a * cos(f32((2u * n + 1u) * k) * 0.19634954);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32,
        @builtin(local_invocation_id) l: vec3<u32>) {
    let block = wg.y * p.blocks_w + wg.x;
    let word = coef[block * 32u + li / 2u];
    let v = select(word & 0xffffu, word >> 16u, (li & 1u) == 1u);
    let sv = f32(i32(v << 16u) >> 16u);
    s[li] = sv * f32(quant[li]);
    workgroupBarrier();
    // Rows: t[y][x] = sum_u C(u, x) s[y][u]
    var acc = 0.0;
    for (var u = 0u; u < 8u; u++) { acc += c(u, l.x) * s[l.y * 8u + u]; }
    t[li] = acc * 0.5;
    workgroupBarrier();
    var acc2 = 0.0;
    for (var k = 0u; k < 8u; k++) { acc2 += c(k, l.y) * t[k * 8u + l.x]; }
    o[li] = u32(clamp(round(acc2 * 0.5 + 128.0), 0.0, 255.0));
    workgroupBarrier();
    if li < 16u {
        let y = li / 2u;
        let x0 = (li & 1u) * 4u;
        let b = y * 8u + x0;
        let packed = o[b] | (o[b + 1u] << 8u) | (o[b + 2u] << 16u) | (o[b + 3u] << 24u);
        plane[(wg.y * 8u + y) * p.stride + wg.x * 2u + (li & 1u)] = packed;
    }
}
"#;

struct Colour {
    pipe: wgpu::ComputePipeline,
    copy: wgpu::ComputePipeline,
    sampler: wgpu::Sampler,
}

fn compute(gpu: &Gpu, src: &str) -> wgpu::ComputePipeline {
    let m = specular_gfx::shader(&gpu.device, "bench", src);
    gpu.device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &m,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
}

fn bind(gpu: &Gpu, p: &wgpu::ComputePipeline, res: &[wgpu::BindingResource]) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry> = res
        .iter()
        .enumerate()
        .map(|(i, r)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: r.clone(),
        })
        .collect();
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &p.get_bind_group_layout(0),
        entries: &entries,
    })
}

fn dispatch(gpu: &Gpu, p: &wgpu::ComputePipeline, b: &wgpu::BindGroup, x: u32, y: u32) {
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(p);
        pass.set_bind_group(0, b, &[]);
        pass.dispatch_workgroups(x, y, 1);
    }
    gpu.queue.submit([enc.finish()]);
}

const SAMPLED: wgpu::TextureUsages =
    wgpu::TextureUsages::TEXTURE_BINDING.union(wgpu::TextureUsages::COPY_DST);

/// BT.601 full-range integer conversion with centred bilinear chroma, rows split over threads.
fn cpu_convert(y: &[u8], cb: &[u8], cr: &[u8], w: usize, h: usize, sub: usize, out: &mut [u8]) {
    let cw = w.div_ceil(sub);
    let ch = h.div_ceil(sub);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows_per = h.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(rows_per * w * 4).enumerate() {
            s.spawn(move || {
                let mut u = vec![0u8; w];
                let mut v = vec![0u8; w];
                for (r, row) in chunk.chunks_exact_mut(w * 4).enumerate() {
                    let yy = k * rows_per + r;
                    if sub == 1 {
                        u.copy_from_slice(&cb[yy * w..yy * w + w]);
                        v.copy_from_slice(&cr[yy * w..yy * w + w]);
                    } else {
                        // Centred: chroma row (yy - 0.5) / 2, weights 3:1.
                        let cy0 = (yy.saturating_sub(1) / 2).min(ch - 1);
                        let cy1 = yy.div_ceil(2).min(ch - 1);
                        let (wa, wb) = if yy.is_multiple_of(2) { (1, 3) } else { (3, 1) };
                        for x in 0..w {
                            let cx0 = (x.saturating_sub(1) / 2).min(cw - 1);
                            let cx1 = x.div_ceil(2).min(cw - 1);
                            let (xa, xb) = if x % 2 == 0 { (1, 3) } else { (3, 1) };
                            let f = |p: &[u8]| {
                                let a =
                                    p[cy0 * cw + cx0] as u32 * xa + p[cy0 * cw + cx1] as u32 * xb;
                                let b =
                                    p[cy1 * cw + cx0] as u32 * xa + p[cy1 * cw + cx1] as u32 * xb;
                                ((a * wa + b * wb + 8) >> 4) as u8
                            };
                            u[x] = f(cb);
                            v[x] = f(cr);
                        }
                    }
                    for x in 0..w {
                        let l = (y[yy * w + x] as i32) << 16;
                        let (cu, cv) = (u[x] as i32 - 128, v[x] as i32 - 128);
                        let c = |a: i32| (a + 32768).clamp(0, 255 << 16) >> 16;
                        row[x * 4] = c(l + 91881 * cv) as u8;
                        row[x * 4 + 1] = c(l - 22554 * cu - 46802 * cv) as u8;
                        row[x * 4 + 2] = c(l + 116130 * cu) as u8;
                        row[x * 4 + 3] = 255;
                    }
                }
            });
        }
    });
}

fn bench_colour(gpu: &Gpu, c: &Colour, mp: f64, sub: usize) {
    let h = ((mp * 1e6 / 1.5).sqrt()) as u32 & !15;
    let w = h * 3 / 2;
    let (cw, ch) = (w.div_ceil(sub as u32), h.div_ceil(sub as u32));
    let y = noise((w * h) as usize, 1);
    let cb = noise((cw * ch) as usize, 2);
    let cr = noise((cw * ch) as usize, 3);
    let st = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
    let out = tex(gpu, w, h, wgpu::TextureFormat::Rgba8Unorm, st);
    let ov = out.create_view(&Default::default());
    let (ty, tcb, tcr) = (
        tex(gpu, w, h, wgpu::TextureFormat::R8Unorm, SAMPLED),
        tex(gpu, cw, ch, wgpu::TextureFormat::R8Unorm, SAMPLED),
        tex(gpu, cw, ch, wgpu::TextureFormat::R8Unorm, SAMPLED),
    );
    let div = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[sub as f32, sub as f32, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let (vy, vcb, vcr) = (
        ty.create_view(&Default::default()),
        tcb.create_view(&Default::default()),
        tcr.create_view(&Default::default()),
    );
    let bg = bind(
        gpu,
        &c.pipe,
        &[
            wgpu::BindingResource::TextureView(&vy),
            wgpu::BindingResource::TextureView(&vcb),
            wgpu::BindingResource::TextureView(&vcr),
            wgpu::BindingResource::Sampler(&c.sampler),
            wgpu::BindingResource::TextureView(&ov),
            div.as_entire_binding(),
        ],
    );
    let gpu_path = best(|| {
        let t = Instant::now();
        write(gpu, &ty, &y, w, h);
        write(gpu, &tcb, &cb, cw, ch);
        write(gpu, &tcr, &cr, cw, ch);
        dispatch(gpu, &c.pipe, &bg, w.div_ceil(16), h.div_ceil(16));
        wait(gpu);
        ms(t)
    });
    let gpu_kernel = best(|| {
        let t = Instant::now();
        dispatch(gpu, &c.pipe, &bg, w.div_ceil(16), h.div_ceil(16));
        wait(gpu);
        ms(t)
    });
    let rgba_t = tex(gpu, w, h, wgpu::TextureFormat::Rgba8Unorm, SAMPLED);
    let rv = rgba_t.create_view(&Default::default());
    let bg2 = bind(
        gpu,
        &c.copy,
        &[
            wgpu::BindingResource::TextureView(&rv),
            wgpu::BindingResource::TextureView(&ov),
        ],
    );
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut cpu_only = f64::INFINITY;
    let cpu_path = best(|| {
        let t = Instant::now();
        cpu_convert(&y, &cb, &cr, w as usize, h as usize, sub, &mut rgba);
        cpu_only = cpu_only.min(ms(t));
        write(gpu, &rgba_t, &rgba, w * 4, h);
        dispatch(gpu, &c.copy, &bg2, w.div_ceil(16), h.div_ceil(16));
        wait(gpu);
        ms(t)
    });
    println!(
        "colour {:>5.1} MP 4:{}: GPU (upload planes + convert) {gpu_path:6.1} ms [kernel {gpu_kernel:5.1}] | CPU convert {cpu_only:6.1} + RGBA upload = {cpu_path:6.1} ms",
        (w * h) as f64 / 1e6,
        if sub == 2 { "2:0" } else { "4:4" }
    );
}

fn bench_idct(gpu: &Gpu, idct: &wgpu::ComputePipeline, mp: f64) {
    // One 4:2:0 image = 1.5 samples per pixel; benchmark as a single plane of that many samples.
    let h = (((mp * 1.5e6) / 1.5).sqrt() as u32) & !15;
    let w = (h * 3 / 2) & !15;
    let (bw, bh) = (w / 8, h / 8);
    let blocks = (bw * bh) as usize;
    // Typical q92 photo: DC plus ~10 low-frequency AC coefficients per block.
    let mut coef = vec![0i16; blocks * 64];
    for (b, blk) in coef.chunks_exact_mut(64).enumerate() {
        blk[0] = ((b * 37) % 200) as i16 - 100;
        for (k, c) in blk.iter_mut().enumerate().take(10).skip(1) {
            *c = (((b * 13 + k * 7) % 31) as i16) - 15;
        }
    }
    let quant: Vec<u32> = (0..64).map(|k| 2 + k as u32 / 8).collect();
    let cbuf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (blocks * 128) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let qbuf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&quant),
            usage: wgpu::BufferUsages::STORAGE,
        });
    let pbuf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (w * h) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let ubuf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[bw, w / 4, 0, 0]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let bg = bind(
        gpu,
        idct,
        &[
            cbuf.as_entire_binding(),
            qbuf.as_entire_binding(),
            pbuf.as_entire_binding(),
            ubuf.as_entire_binding(),
        ],
    );
    let bytes: &[u8] = bytemuck::cast_slice(&coef);
    // Dispatch is split in rows of blocks to stay under the 65535 workgroups per dimension.
    let run = |gpu: &Gpu| {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(idct);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(bw, bh, 1);
        }
        gpu.queue.submit([enc.finish()]);
    };
    let with_upload = best(|| {
        let t = Instant::now();
        gpu.queue.write_buffer(&cbuf, 0, bytes);
        run(gpu);
        wait(gpu);
        ms(t)
    });
    let kernel = best(|| {
        let t = Instant::now();
        run(gpu);
        wait(gpu);
        ms(t)
    });
    let samples = noise((w * h) as usize, 9);
    let sample_upload = best(|| {
        let t = Instant::now();
        gpu.queue.write_buffer(&pbuf, 0, &samples);
        wait(gpu);
        ms(t)
    });
    println!(
        "idct   {:>5.1} MP 4:2:0 ({:.0} M samples): GPU coefficients upload + IDCT {with_upload:6.1} ms [kernel {kernel:5.1}] | upload CPU-IDCT samples {sample_upload:6.1} ms",
        mp,
        (w * h) as f64 / 1e6
    );
}

fn dds(w: u32, h: u32, dxgi: u32, blocks: &[u8]) -> Vec<u8> {
    let mut d = b"DDS ".to_vec();
    let mut hdr = [0u32; 31];
    hdr[0] = 124;
    hdr[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000;
    hdr[2] = h;
    hdr[3] = w;
    hdr[4] = blocks.len() as u32;
    hdr[6] = 1;
    hdr[18] = 32; // pixel format size
    hdr[19] = 0x4; // DDPF_FOURCC
    hdr[20] = u32::from_le_bytes(*b"DX10");
    hdr[26] = 0x1000;
    d.extend(hdr.iter().flat_map(|v| v.to_le_bytes()));
    d.extend([dxgi, 3, 0, 1, 0].iter().flat_map(|v| v.to_le_bytes()));
    d.extend_from_slice(blocks);
    d
}

fn bench_bc(gpu: &Gpu, copy: &wgpu::ComputePipeline, side: u32, bc7: bool) {
    let block_bytes = if bc7 { 16 } else { 8 };
    let nb = ((side / 4) * (side / 4)) as usize;
    let mut blocks = noise(nb * block_bytes, 5);
    if bc7 {
        // Mode 6 blocks (one subset, RGBA 7.7.7.7 + p-bits): the common mode for photos.
        for b in blocks.chunks_exact_mut(16) {
            b[0] = 0x40 | (b[0] & 0x80);
        }
    }
    let file = dds(side, side, if bc7 { 98 } else { 71 }, &blocks);
    let t = specular_texture::Texture::open(&file).expect("dds");
    let st = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
    let out = tex(gpu, side, side, wgpu::TextureFormat::Rgba8Unorm, st);
    let ov = out.create_view(&Default::default());
    let rgba_t = tex(gpu, side, side, wgpu::TextureFormat::Rgba8Unorm, SAMPLED);
    let rv = rgba_t.create_view(&Default::default());
    let bg = bind(
        gpu,
        copy,
        &[
            wgpu::BindingResource::TextureView(&rv),
            wgpu::BindingResource::TextureView(&ov),
        ],
    );
    let mut cpu_only = f64::INFINITY;
    let cpu = best(|| {
        let t0 = Instant::now();
        let d = t
            .decode(&file, &specular_texture::DecodeOptions::default())
            .expect("decode");
        cpu_only = cpu_only.min(ms(t0));
        write(gpu, &rgba_t, &d.data, side * 4, side);
        dispatch(gpu, copy, &bg, side.div_ceil(16), side.div_ceil(16));
        wait(gpu);
        ms(t0)
    });
    let name = if bc7 { "BC7" } else { "BC1" };
    if !gpu
        .device
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    {
        println!(
            "bcn    {side}² {name}: CPU decode {cpu_only:6.1} + upload = {cpu:6.1} ms | GPU: no BC support"
        );
        return;
    }
    let fmt = if bc7 {
        wgpu::TextureFormat::Bc7RgbaUnorm
    } else {
        wgpu::TextureFormat::Bc1RgbaUnorm
    };
    let bct = tex(gpu, side, side, fmt, SAMPLED);
    let bv = bct.create_view(&Default::default());
    let bg2 = bind(
        gpu,
        copy,
        &[
            wgpu::BindingResource::TextureView(&bv),
            wgpu::BindingResource::TextureView(&ov),
        ],
    );
    let g = best(|| {
        let t0 = Instant::now();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &bct,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &blocks,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(side / 4 * block_bytes as u32),
                rows_per_image: Some(side / 4),
            },
            bct.size(),
        );
        dispatch(gpu, copy, &bg2, side.div_ceil(16), side.div_ceil(16));
        wait(gpu);
        ms(t0)
    });
    println!(
        "bcn    {side}² {name}: CPU decode {cpu_only:6.1} + RGBA upload = {cpu:6.1} ms | GPU block upload + sample {g:6.1} ms"
    );
}

fn main() {
    let gpu = Gpu::new().expect("gpu");
    println!(
        "{} ({:?}), BC: {}",
        gpu.info.name,
        gpu.info.backend,
        gpu.device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    );
    let colour = Colour {
        pipe: compute(&gpu, CONVERT),
        copy: compute(&gpu, RGBA_COPY),
        sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }),
    };
    let only = std::env::args().nth(1);
    let want = |s: &str| only.as_deref().is_none_or(|o| o == s);
    let max = gpu.max_texture_size() as f64;
    if want("colour") {
        for mp in [2.0f64, 12.0, 24.0, 50.0, 100.0] {
            if (mp * 1e6 * 1.5).sqrt() > max {
                continue;
            }
            bench_colour(&gpu, &colour, mp, 2);
            bench_colour(&gpu, &colour, mp, 1);
        }
    }
    if want("idct") {
        let idct = compute(&gpu, IDCT);
        for mp in [2.0f64, 12.0, 24.0, 50.0, 100.0] {
            bench_idct(&gpu, &idct, mp);
        }
    }
    if want("bc") {
        for side in [1024, 4096, 8192, 16384] {
            if side as f64 > max {
                continue;
            }
            bench_bc(&gpu, &colour.copy, side, false);
            bench_bc(&gpu, &colour.copy, side, true);
        }
    }
}
