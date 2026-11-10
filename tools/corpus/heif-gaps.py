#!/usr/bin/env python3
"""Generate the HEIF "format gaps" fixtures and their oracles for
crates/formats/specular-heic/tests/codecs.rs into tools/corpus/out/heif-gaps/ (gitignored).

    gen/<name>.heif   synthetic files muxed here: AVC (libx264) and JPEG (ffmpeg mjpeg) items,
                      single and in grids; `tili` tiled images and `cfen` colour format
                      enhancement images built from heif-enc (x265) items; inter-coded image
                      sequences (heif-enc -S -V).
    ref/<name>.<k>.yuv  bit-exact plane oracles: libavcodec's decode of each coded stream
                      (AVC, JPEG, the sequences' frames in display order) or the lossless
                      source planes (cfen).
    ref/<name>.png    libheif `heif-dec -C nn` output where libheif decodes the file (the
                      `unci` files of the fetched libheif corpus, the grid a tili file was
                      built from).
    manifest.tsv      name, file, oracle kind, oracle files, notes (see codecs.rs).

Needs ffmpeg/ffprobe (libx264, mjpeg), heif-enc/heif-dec (libheif >= 1.19 with x265) and
magick. libheif and libavcodec are oracles only: nothing here is linked into Specular.

--fixtures writes the small committed set of crates/formats/specular-heic/tests/data/ instead
(the always-on tests of tests/fixtures.rs): avc.heif, jpeg.heif, tili.heif (+ its source
grid tili_src.heic), cfen.heif and inter.heics, with .yuv plane oracles (inter.fnv: frame hashes).

Usage: tools/corpus/heif-gaps.py [--clean | --fixtures]
"""
import os
import shutil
import struct
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
OUT = os.path.join(ROOT, 'tools', 'corpus', 'out', 'heif-gaps')
CONF = os.path.join(ROOT, 'tools', 'corpus', 'conformance', 'libheif')
GEN, REF, SRC = (os.path.join(OUT, d) for d in ('gen', 'ref', 'src'))
MANIFEST = []


def run(*args, **kw):
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, **kw)


# ------------------------------------------------------------------ boxes

def box(t, payload):
    return struct.pack('>I4s', 8 + len(payload), t) + payload


def full(t, v, f, payload):
    return box(t, struct.pack('>I', (v << 24) | f) + payload)


def parse_boxes(b, off=0, end=None):
    end = len(b) if end is None else end
    while off + 8 <= end:
        size, t = struct.unpack('>I4s', b[off:off + 8])
        head = 8
        if size == 1:
            size = struct.unpack('>Q', b[off + 8:off + 16])[0]
            head = 16
        elif size == 0:
            size = end - off
        yield t, b[off + head:off + size]
        off += size


def child(b, t):
    return next((p for k, p in parse_boxes(b) if k == t), None)


class Heif:
    """A minimal HEIF reader: items, their data, properties and references."""

    def __init__(self, data):
        self.data = data
        meta = child(data, b'meta')[4:]
        self.primary = struct.unpack('>H', child(meta, b'pitm')[4:6])[0]
        self.idat = child(meta, b'idat') or b''
        self.types, self.props, self.refs, self.locs = {}, {}, [], {}
        iinf = child(meta, b'iinf')
        n_off = 6 if iinf[0] == 0 else 8
        for t, p in parse_boxes(iinf, n_off):
            if t == b'infe':
                v = p[0]
                iid = struct.unpack('>H' if v == 2 else '>I', p[4:6] if v == 2 else p[4:8])[0]
                ty = p[8:12] if v == 2 else p[10:14]
                self.types[iid] = ty
        iprp = child(meta, b'iprp')
        ipco = [(t, p) for t, p in parse_boxes(child(iprp, b'ipco'))]
        self.ipco = ipco
        ipma = child(iprp, b'ipma')
        v, f = ipma[0], struct.unpack('>I', ipma[:4])[0] & 0xffffff
        n = struct.unpack('>I', ipma[4:8])[0]
        o = 8
        for _ in range(n):
            if v < 1:
                iid = struct.unpack('>H', ipma[o:o + 2])[0]; o += 2
            else:
                iid = struct.unpack('>I', ipma[o:o + 4])[0]; o += 4
            k = ipma[o]; o += 1
            lst = []
            for _ in range(k):
                if f & 1:
                    w = struct.unpack('>H', ipma[o:o + 2])[0]; o += 2
                    lst.append((bool(w & 0x8000), w & 0x7fff))
                else:
                    w = ipma[o]; o += 1
                    lst.append((bool(w & 0x80), w & 0x7f))
            self.props[iid] = lst
        iref = child(meta, b'iref')
        if iref is not None:
            large = iref[0] == 1
            for t, p in parse_boxes(iref, 4):
                fmt, w = ('>I', 4) if large else ('>H', 2)
                src = struct.unpack(fmt, p[:w])[0]
                cnt = struct.unpack('>H', p[w:w + 2])[0]
                to = [struct.unpack(fmt, p[w + 2 + i * w:w + 2 + (i + 1) * w])[0] for i in range(cnt)]
                self.refs.append((t, src, to))
        iloc = child(meta, b'iloc')
        v = iloc[0]
        a, b2 = iloc[4], iloc[5]
        osz, lsz, bsz = a >> 4, a & 15, b2 >> 4
        isz = b2 & 15 if v in (1, 2) else 0
        o = 6
        cnt = struct.unpack('>H' if v < 2 else '>I', iloc[o:o + (2 if v < 2 else 4)])[0]
        o += 2 if v < 2 else 4

        def rd(n):
            nonlocal o
            val = int.from_bytes(iloc[o:o + n], 'big') if n else 0
            o += n
            return val
        for _ in range(cnt):
            iid = rd(2 if v < 2 else 4)
            cm = rd(2) & 15 if v in (1, 2) else 0
            rd(2)
            base = rd(bsz)
            ext = []
            for _ in range(rd(2)):
                rd(isz)
                ext.append((base + rd(osz), rd(lsz)))
            self.locs[iid] = (cm, ext)

    def item(self, iid):
        cm, ext = self.locs[iid]
        assert cm in (0, 1)
        src = self.data if cm == 0 else self.idat
        return b''.join(src[o:o + n] for o, n in ext)

    def prop(self, iid, t):
        for _, i in self.props.get(iid, []):
            pt, p = self.ipco[i - 1]
            if pt == t:
                return p
        return None

    def dimg(self, iid):
        return next((to for t, s, to in self.refs if t == b'dimg' and s == iid), [])


def write_heif(path, items, refs, primary, brands=(b'mif1', b'heic'), drefs=None):
    """items: [{id, type, data, props: [(box bytes, essential)], hidden?, dref?}]; refs:
    [(type, from, [to...])]. Properties with equal bytes share one ipco entry."""
    ipco, index = [], {}
    for it in items:
        it['assoc'] = []
        for p, ess in it['props']:
            if p not in index:
                ipco.append(p)
                index[p] = len(ipco)
            it['assoc'].append((ess, index[p]))
    ftyp = box(b'ftyp', brands[0] + b'\0\0\0\0' + b''.join(brands))
    hdlr = full(b'hdlr', 0, 0, b'\0' * 4 + b'pict' + b'\0' * 13)
    pitm = full(b'pitm', 0, 0, struct.pack('>H', primary))
    iinf = full(b'iinf', 0, 0, struct.pack('>H', len(items)) + b''.join(
        full(b'infe', 2, 1 if it.get('hidden') else 0,
             struct.pack('>HH', it['id'], 0) + it['type'] + b'\0') for it in items))
    iref = full(b'iref', 0, 0, b''.join(
        box(t, struct.pack('>HH', s, len(to)) + b''.join(struct.pack('>H', x) for x in to))
        for t, s, to in refs)) if refs else b''
    large = any(i > 127 for it in items for _, i in it['assoc'])
    ipma = full(b'ipma', 0, 1 if large else 0, struct.pack('>I', len(items)) + b''.join(
        struct.pack('>HB', it['id'], len(it['assoc'])) + b''.join(
            struct.pack('>H', (0x8000 if e else 0) | i) if large else bytes([(0x80 if e else 0) | i])
            for e, i in it['assoc']) for it in items))
    iprp = box(b'iprp', box(b'ipco', b''.join(ipco)) + ipma)
    dinf = b''
    if drefs:
        dinf = box(b'dinf', full(b'dref', 0, 0, struct.pack('>I', len(drefs)) + b''.join(drefs)))

    def meta(base):
        # iloc version 1: 4-byte offsets and lengths, construction method 0.
        body = struct.pack('>BBH', 0x44, 0x00, len(items))
        off = base
        for it in items:
            body += struct.pack('>HHHHII', it['id'], 0, it.get('dref', 0), 1, off, len(it['data']))
            off += len(it['data'])
        iloc = full(b'iloc', 1, 0, body)
        return full(b'meta', 0, 0, hdlr + pitm + iloc + iinf + iref + iprp + dinf)
    head = len(ftyp) + len(meta(0)) + 8
    out = ftyp + meta(head) + box(b'mdat', b''.join(it['data'] for it in items))
    with open(path, 'wb') as f:
        f.write(out)


def ispe(w, h):
    return full(b'ispe', 0, 0, struct.pack('>II', w, h))


def nclx(p, t, m, full_range):
    return box(b'colr', b'nclx' + struct.pack('>HHHB', p, t, m, 0x80 if full_range else 0))


def pixi(*bits):
    return full(b'pixi', 0, 0, bytes([len(bits)]) + bytes(bits))


# ------------------------------------------------------------------ codecs

def annexb_nals(b):
    out, i, start = [], 0, None
    while i + 3 <= len(b):
        if b[i:i + 3] == b'\0\0\1':
            if start is not None:
                out.append(b[start:i].rstrip(b'\0'))
            i += 3
            start = i
        else:
            i += 1
    if start is not None:
        out.append(b[start:])
    return [n for n in out if n]


def avc_item(h264):
    """(avcC box, item data) from an Annex B stream holding one picture."""
    nals = annexb_nals(h264)
    sps = [n for n in nals if n[0] & 31 == 7]
    pps = [n for n in nals if n[0] & 31 == 8]
    vcl = [n for n in nals if 1 <= n[0] & 31 <= 5]
    s = sps[0]
    rec = bytes([1, s[1], s[2], s[3], 0xff, 0xe0 | len(sps)])
    for n in sps:
        rec += struct.pack('>H', len(n)) + n
    rec += bytes([len(pps)])
    for n in pps:
        rec += struct.pack('>H', len(n)) + n
    if s[1] in (100, 110, 122, 244):
        chroma, bl, bc = sps_chroma(s)
        rec += bytes([0xfc | chroma, 0xf8 | (bl - 8), 0xf8 | (bc - 8), 0])
    data = b''.join(struct.pack('>I', len(n)) + n for n in vcl)
    return box(b'avcC', rec), data


def sps_chroma(sps):
    """chroma_format_idc and bit depths of a High-profile SPS (Exp-Golomb, no emulation
    prevention in the few bytes read)."""
    rbsp = bytearray()
    zeros = 0
    for byte in sps[1:]:
        if zeros >= 2 and byte == 3:
            zeros = 0
            continue
        rbsp.append(byte)
        zeros = zeros + 1 if byte == 0 else 0
    bits = ''.join(f'{x:08b}' for x in rbsp[3:])
    pos = 0

    def ue():
        nonlocal pos
        z = 0
        while bits[pos] == '0':
            z += 1
            pos += 1
        pos += 1
        v = int(bits[pos:pos + z] or '0', 2) + (1 << z) - 1
        pos += z
        return v
    ue()  # seq_parameter_set_id
    chroma = ue()
    if chroma == 3:
        pos += 1
    return chroma, ue() + 8, ue() + 8


def ffmpeg_frame(src, dst, *opts):
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', src, *opts, '-frames:v', '1', dst)


def decode_raw(stream, fmt, pix, dst):
    """libavcodec's decode of an elementary stream to raw planes (oracle)."""
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-f', fmt, '-i', stream, '-f', 'rawvideo',
        '-pix_fmt', pix, dst)


def source(name, w, h, *extra):
    p = os.path.join(SRC, f'{name}.png')
    if not os.path.exists(p):
        # testsrc2 sizes are even: odd sizes are scaled to.
        run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-f', 'lavfi', '-i',
            f'testsrc2=size={w + w % 2}x{h + h % 2}:rate=1', '-frames:v', '1', '-vf',
            f'scale={w}:{h}', *extra, p)
    return p


def h264(src, pix, extra=()):
    tmp = os.path.join(SRC, 'tmp.h264')
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', src, '-frames:v', '1', '-c:v', 'libx264',
        '-pix_fmt', pix, '-crf', '18', '-x264-params', 'keyint=1', *extra, '-bsf:v',
        'h264_mp4toannexb', '-f', 'h264', tmp)
    with open(tmp, 'rb') as f:
        return f.read(), tmp


def mjpeg(src, pix):
    tmp = os.path.join(SRC, 'tmp.jpg')
    if pix == 'gray':
        # libavcodec writes grey as three components; libjpeg (magick) as one.
        run('magick', src, '-colorspace', 'Gray', '-quality', '90', tmp)
    else:
        run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', src, '-frames:v', '1', '-c:v',
            'mjpeg', '-pix_fmt', pix, '-q:v', '3', '-strict', '-1', tmp)
    with open(tmp, 'rb') as f:
        return f.read(), tmp


def crop(src, x, y, w, h, name):
    p = os.path.join(SRC, f'{name}.png')
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', src, '-vf', f'crop={w}:{h}:{x}:{y}', p)
    return p


def manifest(name, kind, refs, note=''):
    MANIFEST.append((name, f'gen/{name}.heif', kind, ' '.join(refs), note))


# ------------------------------------------------------------------ fixtures

def gen_avc():
    for name, pix, ref_pix in [('avc_420', 'yuv420p', 'yuv420p'),
                               ('avc_422_10bit', 'yuv422p10le', 'yuv422p10le'),
                               ('avc_mono', 'gray', 'gray')]:
        src = source('base', 320, 240)
        stream, tmp = h264(src, pix)
        avcc, data = avc_item(stream)
        decode_raw(tmp, 'h264', ref_pix, os.path.join(REF, f'{name}.0.yuv'))
        write_heif(os.path.join(GEN, f'{name}.heif'),
                   [{'id': 1, 'type': b'avc1', 'data': data,
                     'props': [(avcc, True), (ispe(320, 240), False)]}], [], 1,
                   brands=(b'avci', b'mif1', b'avci'))
        manifest(name, 'planes', [f'ref/{name}.0.yuv'], f'320x240 {ref_pix}')
    # A 2x2 grid of 160x120 AVC tiles; the oracle is each tile's decode.
    src = source('base', 320, 240)
    items, refs = [], []
    for k in range(4):
        x, y = (k % 2) * 160, (k // 2) * 120
        t = crop(src, x, y, 160, 120, f'avc_tile{k}')
        stream, tmp = h264(t, 'yuv420p')
        avcc, data = avc_item(stream)
        decode_raw(tmp, 'h264', 'yuv420p', os.path.join(REF, f'avc_grid.{k}.yuv'))
        items.append({'id': 2 + k, 'type': b'avc1', 'data': data, 'hidden': True,
                      'props': [(avcc, True), (ispe(160, 120), False)]})
    grid = bytes([0, 0, 1, 1]) + struct.pack('>HH', 320, 240)
    items.insert(0, {'id': 1, 'type': b'grid', 'data': grid, 'props': [(ispe(320, 240), False)]})
    refs.append((b'dimg', 1, [2, 3, 4, 5]))
    write_heif(os.path.join(GEN, 'avc_grid.heif'), items, refs, 1,
               brands=(b'avci', b'mif1', b'avci'))
    manifest('avc_grid', 'tiles', [f'ref/avc_grid.{k}.yuv' for k in range(4)], '2x2 160x120 yuv420p')


def gen_jpeg():
    for name, w, h, pix in [('jpeg_420', 320, 240, 'yuvj420p'), ('jpeg_422', 320, 240, 'yuvj422p'),
                            ('jpeg_444', 320, 240, 'yuvj444p'), ('jpeg_gray', 320, 240, 'gray'),
                            ('jpeg_odd', 321, 241, 'yuvj420p')]:
        src = source(f'base{w}', w, h)
        jpg, tmp = mjpeg(src, pix)
        decode_raw(tmp, 'mjpeg', pix, os.path.join(REF, f'{name}.0.yuv'))
        write_heif(os.path.join(GEN, f'{name}.heif'),
                   [{'id': 1, 'type': b'jpeg', 'data': jpg,
                     'props': [(ispe(w, h), False), (nclx(1, 13, 6, True), False)]}], [], 1,
                   brands=(b'mif1', b'jpeg'))
        manifest(name, 'planes', [f'ref/{name}.0.yuv'], f'{w}x{h} {pix}')
    src = source('base', 320, 240)
    items = []
    for k in range(4):
        x, y = (k % 2) * 160, (k // 2) * 120
        t = crop(src, x, y, 160, 120, f'jpeg_tile{k}')
        jpg, tmp = mjpeg(t, 'yuvj420p')
        decode_raw(tmp, 'mjpeg', 'yuvj420p', os.path.join(REF, f'jpeg_grid.{k}.yuv'))
        items.append({'id': 2 + k, 'type': b'jpeg', 'data': jpg, 'hidden': True,
                      'props': [(ispe(160, 120), False)]})
    grid = bytes([0, 0, 1, 1]) + struct.pack('>HH', 320, 240)
    items.insert(0, {'id': 1, 'type': b'grid', 'data': grid, 'props': [(ispe(320, 240), False)]})
    write_heif(os.path.join(GEN, 'jpeg_grid.heif'), items, [(b'dimg', 1, [2, 3, 4, 5])], 1,
               brands=(b'mif1', b'jpeg'))
    manifest('jpeg_grid', 'tiles', [f'ref/jpeg_grid.{k}.yuv' for k in range(4)], '2x2 160x120 yuvj420p')


def deti(n, table_start, table_size):
    # Offsets 32-bit (code 0), sizes 32-bit (code 2), sequential, 32-bit count (code 2).
    flags = (2 << 2) | (1 << 4) | (2 << 5)
    return full(b'deti', 0, flags, struct.pack('>III', n, table_start, table_size))


def gen_tili():
    """tili images re-muxed from heif-enc's 256-pixel grid tiles: every tile in the item's
    data, an offset table after them. The oracle is libheif's decode of the grid file."""
    src = source('tili', 1000, 600)
    grid_file = os.path.join(GEN, 'tili_grid_src.heic')
    run('heif-enc', '-q', '80', '--cut-tiles', '256', '-o', grid_file, src)
    run('heif-dec', '--quiet', '-C', 'nn', grid_file, os.path.join(REF, 'tili_grid_src.png'))
    with open(grid_file, 'rb') as f:
        g = Heif(f.read())
    tiles = g.dimg(g.primary)
    hvcc = g.prop(tiles[0], b'hvcC')
    assert hvcc is not None
    gp = g.item(g.primary)
    rows, cols = gp[2] + 1, gp[3] + 1
    for name, empty in [('tili_hevc', None), ('tili_hevc_empty', 5)]:
        body, table = b'', b''
        for k, t in enumerate(tiles):
            if k == empty:
                table += struct.pack('>II', 0xffffffff, 0)
                continue
            d = g.item(t)
            table += struct.pack('>II', len(body), len(d))
            body += d
        start = len(body)
        data = body + table
        # tilC: tile size, no extra dimensions, tile_item_type, tipa (hvcC essential).
        tilc_props = [box(b'hvcC', hvcc)]
        tipa = full(b'tipa', 0, 0, bytes([1, 0x80 | 2]))
        tilc = full(b'tilC', 0, 0, struct.pack('>IIB', 256, 256, 0) + b'hvc1' + tipa)
        url = full(b'url ', 0, 1, b'')
        items = [{'id': 1, 'type': b'tili', 'data': data, 'dref': 2,
                  'props': [(tilc, True), (tilc_props[0], False), (ispe(1000, 600), False)]}]
        write_heif(os.path.join(GEN, f'{name}.heif'), items, [], 1,
                   drefs=[url, deti(len(tiles), start, len(table))])
        manifest(name, 'png', ['ref/tili_grid_src.png'],
                 f'{cols}x{rows} tiles of 256' + (f', tile {empty} empty' if empty is not None else ''))


def gen_cfen():
    """cfen images from monochrome lossless HEVC items (one per channel); the oracle is the
    source planes themselves."""
    src = source('cfen', 320, 240)
    for name, pix, chans, matrix in [('cfen_ycbcr444', 'yuv444p', [2, 3, 4], 6),
                                     ('cfen_ycbcr420', 'yuv420p', [2, 3, 4], 6),
                                     ('cfen_rgb', 'gbrp', [3, 4, 2], 0)]:
        raw = os.path.join(SRC, f'{name}.yuv')
        run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', src, '-f', 'rawvideo', '-pix_fmt', pix,
            '-color_range', 'pc', raw)
        with open(raw, 'rb') as f:
            planes = f.read()
        cw, ch = (160, 120) if pix == 'yuv420p' else (320, 240)
        sizes = [(320, 240), (cw, ch), (cw, ch)]
        items, off = [], 0
        ids = []
        for k, (w, h) in enumerate(sizes):
            plane = planes[off:off + w * h]
            off += w * h
            pgm = os.path.join(SRC, f'{name}.{k}.pgm')
            with open(pgm, 'wb') as f:
                f.write(b'P5\n%d %d\n255\n' % (w, h) + plane)
            png = os.path.join(SRC, f'{name}.{k}.png')
            run('magick', pgm, '-colorspace', 'Gray', png)
            heic = os.path.join(SRC, f'{name}.{k}.heic')
            run('heif-enc', '-L', '-o', heic, png)
            with open(heic, 'rb') as f:
                one = Heif(f.read())
            hvcc = one.prop(one.primary, b'hvcC')
            items.append({'id': 2 + k, 'type': b'hvc1', 'data': one.item(one.primary),
                          'hidden': True,
                          'props': [(box(b'hvcC', hvcc), True), (ispe(w, h), False),
                                    (pixi(8), False)]})
            ids.append(2 + k)
        with open(os.path.join(REF, f'{name}.0.yuv'), 'wb') as f:
            f.write(planes)
        body = bytes([0]) + b''.join(bytes([0, c]) for c in chans)
        items.insert(0, {'id': 1, 'type': b'cfen', 'data': body,
                         'props': [(ispe(320, 240), False), (pixi(8, 8, 8), False),
                                   (nclx(1, 13, matrix, True), False)]})
        write_heif(os.path.join(GEN, f'{name}.heif'), items, [(b'dimg', 1, ids)], 1)
        manifest(name, 'planes', [f'ref/{name}.0.yuv'], f'320x240 {pix}')


def gen_sequences():
    frames = os.path.join(SRC, 'seq')
    os.makedirs(frames, exist_ok=True)
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-f', 'lavfi', '-i',
        'testsrc2=size=192x128:rate=10', '-frames:v', '12', os.path.join(frames, 'f%02d.png'))
    for name, gop in [('seq_lowdelay', 'low-delay'), ('seq_unrestricted', 'unrestricted')]:
        out = os.path.join(GEN, f'{name}.heif')
        run('heif-enc', '-S', '-V', '--fps', '10', '--gop-structure', gop, '-q', '60',
            '--max-keyframe-distance', '8', '-o', out, os.path.join(frames, 'f01.png'))
        ref = os.path.join(REF, f'{name}.all.yuv')
        run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', out, '-f', 'rawvideo', '-pix_fmt',
            'yuv420p', '-fps_mode', 'passthrough', ref)
        manifest(name, 'frames', [f'ref/{name}.all.yuv'], '192x128 yuv420p 12 frames at 10/s')


def gen_unci_refs():
    """libheif's decodes of the fetched libheif `unci` files."""
    d = os.path.join(CONF, 'tests', 'data')
    if not os.path.isdir(d):
        print('heif-gaps: no libheif corpus (fetch-conformance.sh src_heif); unci refs skipped')
        return
    for f in sorted(os.listdir(d)):
        p = os.path.join(d, f)
        with open(p, 'rb') as fh:
            if b'uncC' not in fh.read():
                continue
        name = 'unci_' + os.path.splitext(f)[0]
        png = os.path.join(REF, f'{name}.png')
        try:
            run('heif-dec', '--quiet', '-C', 'nn', p, png)
        except subprocess.CalledProcessError:
            MANIFEST.append((name, os.path.relpath(p, OUT), 'refused', '', 'libheif fails'))
            continue
        MANIFEST.append((name, os.path.relpath(p, OUT), 'png', f'ref/{name}.png', ''))


def gen_corpus_sequences():
    """The inter-coded sequences of the fetched Nokia and MPEG corpora: every frame of the
    first video track decoded by libavcodec, in display order."""
    conf = os.path.dirname(CONF)
    files = ['nokia-heif/content/image_sequences/' + f for f in (
        'bird_burst.heic', 'candle_animation.heic', 'rally_burst.heic', 'sea1_animation.heic',
        'starfield_animation.heic')]
    files += ['mpeg-ffc/data/file_features/published/heif/' + f
              for f in ('C027.heic', 'C028.heic', 'C041.heic')]
    for rel in files:
        p = os.path.join(conf, rel)
        if not os.path.exists(p):
            continue
        name = 'seq_' + os.path.splitext(os.path.basename(rel))[0]
        # libavformat lists the image items as streams too: the track is the first stream
        # with more than one frame.
        streams = subprocess.run(['ffprobe', '-v', 'error', '-show_entries',
                                  'stream=index,nb_frames,pix_fmt', '-of', 'csv=p=0', p],
                                 capture_output=True, text=True).stdout.split()
        track = next((s.split(',') for s in streams
                      if s.split(',')[-1].isdigit() and int(s.split(',')[-1]) > 1), None)
        if track is None:
            continue
        index, pix = track[0], track[1]
        ref = os.path.join(REF, f'{name}.all.yuv')
        try:
            run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', p, '-map', f'0:{index}',
                '-fps_mode', 'passthrough', '-f', 'rawvideo', '-pix_fmt', pix, ref)
        except subprocess.CalledProcessError:
            continue
        MANIFEST.append((name, os.path.relpath(p, OUT), 'frames', f'ref/{name}.all.yuv', pix))


def gen_corpus_items():
    """libheif's avc32.heif / jpeg32.heif: the item streams decoded by libavcodec."""
    d = os.path.join(CONF, 'fuzzing', 'data', 'corpus')
    for f, kind in [('avc32.heif', 'avc'), ('jpeg32.heif', 'jpeg')]:
        p = os.path.join(d, f)
        if not os.path.exists(p):
            continue
        with open(p, 'rb') as fh:
            g = Heif(fh.read())
        name = 'corpus_' + kind
        data = g.item(g.primary)
        if kind == 'avc':
            rec = g.prop(g.primary, b'avcC')
            o, nals = 6, []
            for _ in range(rec[5] & 31):
                n = struct.unpack('>H', rec[o:o + 2])[0]
                nals.append(rec[o + 2:o + 2 + n])
                o += 2 + n
            n_pps = rec[o]
            o += 1
            for _ in range(n_pps):
                n = struct.unpack('>H', rec[o:o + 2])[0]
                nals.append(rec[o + 2:o + 2 + n])
                o += 2 + n
            ls = (rec[4] & 3) + 1
            i = 0
            while i < len(data):
                n = int.from_bytes(data[i:i + ls], 'big')
                nals.append(data[i + ls:i + ls + n])
                i += ls + n
            stream = b''.join(b'\0\0\0\1' + n for n in nals)
            fmt, pix = 'h264', 'yuv420p'
        else:
            stream, fmt, pix = data, 'mjpeg', 'yuvj420p'
        tmp = os.path.join(SRC, f'{name}.bin')
        with open(tmp, 'wb') as fh:
            fh.write(stream)
        try:
            decode_raw(tmp, fmt, pix, os.path.join(REF, f'{name}.0.yuv'))
        except subprocess.CalledProcessError:
            continue
        MANIFEST.append((name, os.path.relpath(p, OUT), 'item', f'ref/{name}.0.yuv', pix))


def gen_fixtures():
    """The committed fixtures (see the module docs), kept to a few kilobytes each."""
    global GEN, REF, SRC
    dest = os.path.join(ROOT, 'crates', 'formats', 'specular-heic', 'tests', 'data')
    GEN = REF = dest
    SRC = os.path.join(OUT, 'fixtures-src')
    os.makedirs(dest, exist_ok=True)
    os.makedirs(SRC, exist_ok=True)
    src = source('fx', 64, 48)
    stream, tmp = h264(src, 'yuv420p', ('-crf', '30'))
    avcc, data = avc_item(stream)
    decode_raw(tmp, 'h264', 'yuv420p', os.path.join(dest, 'avc.yuv'))
    write_heif(os.path.join(dest, 'avc.heif'),
               [{'id': 1, 'type': b'avc1', 'data': data,
                 'props': [(avcc, True), (ispe(64, 48), False)]}], [], 1,
               brands=(b'avci', b'mif1', b'avci'))
    jpg, tmp = mjpeg(src, 'yuvj420p')
    decode_raw(tmp, 'mjpeg', 'yuvj420p', os.path.join(dest, 'jpeg.yuv'))
    write_heif(os.path.join(dest, 'jpeg.heif'),
               [{'id': 1, 'type': b'jpeg', 'data': jpg,
                 'props': [(ispe(64, 48), False), (nclx(1, 13, 6, True), False)]}], [], 1,
               brands=(b'mif1', b'jpeg'))
    # tili: the 2x2 grid of 64-pixel tiles heif-enc cuts from a 112x80 image, re-muxed.
    big = source('fxbig', 112, 80)
    grid_file = os.path.join(dest, 'tili_src.heic')
    run('heif-enc', '-q', '40', '--cut-tiles', '64', '-o', grid_file, big)
    with open(grid_file, 'rb') as f:
        g = Heif(f.read())
    tiles = g.dimg(g.primary)
    hvcc = g.prop(tiles[0], b'hvcC')
    body, table = b'', b''
    for t in tiles:
        d = g.item(t)
        table += struct.pack('>II', len(body), len(d))
        body += d
    tipa = full(b'tipa', 0, 0, bytes([1, 0x80 | 2]))
    tilc = full(b'tilC', 0, 0, struct.pack('>IIB', 64, 64, 0) + b'hvc1' + tipa)
    write_heif(os.path.join(dest, 'tili.heif'),
               [{'id': 1, 'type': b'tili', 'data': body + table, 'dref': 2,
                 'props': [(tilc, True), (box(b'hvcC', hvcc), False), (ispe(112, 80), False)]}],
               [], 1, drefs=[full(b'url ', 0, 1, b''), deti(len(tiles), len(body), len(table))])
    # cfen: R, G and B as three lossless monochrome HEVC items.
    raw = os.path.join(SRC, 'fx.gbr')
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', src, '-f', 'rawvideo', '-pix_fmt', 'gbrp',
        raw)
    with open(raw, 'rb') as f:
        planes = f.read()
    items = []
    for k in range(3):
        pgm = os.path.join(SRC, f'fx{k}.pgm')
        with open(pgm, 'wb') as f:
            f.write(b'P5\n64 48\n255\n' + planes[k * 3072:(k + 1) * 3072])
        png = os.path.join(SRC, f'fx{k}.png')
        run('magick', pgm, '-colorspace', 'Gray', png)
        heic = os.path.join(SRC, f'fx{k}.heic')
        run('heif-enc', '-L', '-o', heic, png)
        with open(heic, 'rb') as f:
            one = Heif(f.read())
        items.append({'id': 2 + k, 'type': b'hvc1', 'data': one.item(one.primary), 'hidden': True,
                      'props': [(box(b'hvcC', one.prop(one.primary, b'hvcC')), True),
                                (ispe(64, 48), False)]})
    with open(os.path.join(dest, 'cfen.yuv'), 'wb') as f:
        f.write(planes)
    items.insert(0, {'id': 1, 'type': b'cfen', 'data': bytes([0, 0, 3, 0, 4, 0, 2]),
                     'props': [(ispe(64, 48), False), (pixi(8, 8, 8), False),
                               (nclx(1, 13, 0, True), False)]})
    write_heif(os.path.join(dest, 'cfen.heif'), items, [(b'dimg', 1, [2, 3, 4])], 1)
    # An inter-coded sequence with B pictures: 8 frames of 64x64 at 10 per second (x265's
    # smallest picture). The oracle: a 64-bit FNV-1a hash of each frame's planes.
    frames = os.path.join(SRC, 'fxseq')
    os.makedirs(frames, exist_ok=True)
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-f', 'lavfi', '-i',
        'testsrc2=size=64x64:rate=10', '-frames:v', '8', os.path.join(frames, 'f%02d.png'))
    out = os.path.join(dest, 'inter.heics')
    run('heif-enc', '-S', '-V', '--fps', '10', '--gop-structure', 'unrestricted', '-q', '30',
        '--max-keyframe-distance', '8', '-o', out, os.path.join(frames, 'f01.png'))
    yuv = os.path.join(SRC, 'inter.yuv')
    run('ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', out, '-fps_mode', 'passthrough', '-f',
        'rawvideo', '-pix_fmt', 'yuv420p', yuv)
    with open(yuv, 'rb') as f:
        raw = f.read()
    with open(os.path.join(dest, 'inter.fnv'), 'w') as f:
        for k in range(0, len(raw), 6144):
            h = 0xcbf29ce484222325
            for byte in raw[k:k + 6144]:
                h = ((h ^ byte) * 0x100000001b3) & 0xffffffffffffffff
            f.write(f'{h:016x}\n')
    shutil.rmtree(SRC, ignore_errors=True)
    print('heif-gaps: fixtures ->', dest)


def main():
    if '--fixtures' in sys.argv:
        gen_fixtures()
        return
    if '--clean' in sys.argv:
        shutil.rmtree(OUT, ignore_errors=True)
        print('removed', OUT)
        return
    for t in ('ffmpeg', 'heif-enc', 'heif-dec', 'magick'):
        if shutil.which(t) is None:
            sys.exit(f'heif-gaps: {t} not found')
    if shutil.disk_usage(ROOT).free < 8 << 30:
        sys.exit('heif-gaps: less than 8 GB free')
    for d in (GEN, REF, SRC):
        os.makedirs(d, exist_ok=True)
    gen_avc()
    gen_jpeg()
    gen_tili()
    gen_cfen()
    gen_sequences()
    gen_unci_refs()
    gen_corpus_items()
    gen_corpus_sequences()
    with open(os.path.join(OUT, 'manifest.tsv'), 'w') as f:
        for row in MANIFEST:
            f.write('\t'.join(row) + '\n')
    shutil.rmtree(SRC, ignore_errors=True)
    print(f'heif-gaps: {len(MANIFEST)} entries -> {OUT}/manifest.tsv')


if __name__ == '__main__':
    main()
