// Batched 2-D transpose, src [B, R, C] to dst [B, C, R], through 32×32 tiles in workgroup
// memory: reads and writes both along rows (the strided copy reads one value per cache
// line when the moved axis is short, as Tesseract's channels-last to channels-first).
// P: B, R, C, column tiles, row tiles.
var<workgroup> tile: array<f32, 1056>;

@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>,
        @builtin(local_invocation_index) li: u32) {
    let wg = growid(wid, nwg);
    let rows = pu(1u);
    let cols = pu(2u);
    let tc = pu(3u);
    let per_b = tc * pu(4u);
    let b = wg / per_b;
    let t = wg % per_b;
    let r0 = (t / tc) * 32u;
    let c0 = (t % tc) * 32u;
    let lx = li % 32u;
    let ly = li / 32u;
    let inside = b < pu(0u);
    for (var y = ly; y < 32u; y += 8u) {
        let r = r0 + y;
        let c = c0 + lx;
        var v = 0.0;
        if (inside && r < rows && c < cols) {
            v = ld0((b * rows + r) * cols + c);
        }
        tile[y * 33u + lx] = v;
    }
    workgroupBarrier();
    for (var y = ly; y < 32u; y += 8u) {
        let c = c0 + y;
        let r = r0 + lx;
        if (inside && c < cols && r < rows) {
            dst[(b * cols + c) * rows + r] = tile[lx * 33u + y];
        }
    }
}
