#!/usr/bin/env python3
"""Independent 3D Tiles traversal: the oracle for specular-tiles3d's tile counts and
level-of-detail selections.

Written from the 3D Tiles 1.0/1.1 specifications (not from the Rust code). For every tileset
listed in views.json (written by `cargo test -p specular-tiles3d --test corpus`), it reads
the tileset with its external tilesets and implicit subtrees, counts tiles and drawable
contents, and selects tiles for each listed view by screen-space error with the same rules
the crate documents (frustum test on the bounding sphere, viewer request volumes, pass
through external and implicit roots, ADD/REPLACE). Writes oracle.json beside views.json and
prints the mismatch counts.

Usage: tiles3d-oracle.py CORPUS_DIR OUT_DIR
"""
import json
import math
import struct
import sys
import urllib.parse
from pathlib import Path

A = 6378137.0
E2 = 6.694379990141316e-3
GRID = 9


def v_add(a, b): return [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
def v_sub(a, b): return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
def v_mul(a, s): return [a[0] * s, a[1] * s, a[2] * s]
def v_dot(a, b): return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
def v_len(a): return math.sqrt(v_dot(a, a))


def v_cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def v_unit(a):
    n = v_len(a)
    return v_mul(a, 1.0 / n) if n > 0 else [0.0, 0.0, 0.0]


def m_from_cols(a):
    return [[a[c * 4 + r] for c in range(4)] for r in range(4)]


def m_mul(a, b):
    return [[sum(a[r][k] * b[k][c] for k in range(4)) for c in range(4)] for r in range(4)]


IDENT = [[1.0 if r == c else 0.0 for c in range(4)] for r in range(4)]


def m_pt(m, p):
    return [m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3] for r in range(3)]


def m_vec(m, p):
    return [m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] for r in range(3)]


def cartographic(lon, lat, h):
    s, c = math.sin(lat), math.cos(lat)
    n = A / math.sqrt(1 - E2 * s * s)
    return [(n + h) * c * math.cos(lon), (n + h) * c * math.sin(lon), (n * (1 - E2) + h) * s]


def enu_axes(p):
    up = v_unit([p[0], p[1], p[2] / (1 - E2)])
    east = [-p[1], p[0], 0.0]
    if v_len(east) < 1e-9:
        east = [1.0, 0.0, 0.0]
    east = v_unit(east)
    north = v_cross(up, east)
    return east, north, up


class Vol:
    """An oriented box (centre, three half-axes) or a sphere."""

    def __init__(self, c, axes=None, r=None):
        self.c, self.axes, self.r = c, axes, r

    def radius(self):
        if self.axes is None:
            return self.r
        u, v, w = self.axes
        return max(v_len(v_add(v_add(v_mul(u, a), v_mul(v, b)), w))
                   for a, b in ((1, 1), (1, -1), (-1, 1), (-1, -1)))

    def dist(self, p):
        d = v_sub(p, self.c)
        if self.axes is None:
            return max(v_len(d) - self.r, 0.0)
        tot = 0.0
        for k in range(3):
            ax = self.axes[k]
            h = v_len(ax)
            if h > 1e-12:
                n = v_mul(ax, 1 / h)
            else:
                n, h = v_unit(v_cross(self.axes[(k + 1) % 3], self.axes[(k + 2) % 3])), 0.0
            e = max(abs(v_dot(d, n)) - h, 0.0)
            tot += e * e
        return math.sqrt(tot)


def region_vol(r):
    w, s, e, n, lo, hi = r
    if e < w:
        e += 2 * math.pi
    hi = max(hi, lo)
    origin = cartographic((w + e) / 2, (s + n) / 2, lo)
    ax = enu_axes(origin)
    mn, mx = [math.inf] * 3, [-math.inf] * 3
    for i in range(GRID):
        lon = w + (e - w) * i / (GRID - 1)
        for j in range(GRID):
            lat = s + (n - s) * j / (GRID - 1)
            for h in (lo, hi):
                d = v_sub(cartographic(lon, lat, h), origin)
                q = [v_dot(d, a) for a in ax]
                mn = [min(a, b) for a, b in zip(mn, q)]
                mx = [max(a, b) for a, b in zip(mx, q)]
    mid = [(a + b) / 2 for a, b in zip(mn, mx)]
    half = [(b - a) / 2 for a, b in zip(mn, mx)]
    c = origin
    for k in range(3):
        c = v_add(c, v_mul(ax[k], mid[k]))
    return Vol(c, [v_mul(ax[k], half[k]) for k in range(3)])


def to_vol(bv, m):
    """(given, world volume) of a boundingVolume object under transform m."""
    if not isinstance(bv, dict):
        return None
    def ok(k, n):
        v = bv.get(k)
        return isinstance(v, list) and len(v) == n and all(isinstance(x, (int, float)) for x in v)
    if ok('box', 12):
        b = [float(x) for x in bv['box']]
        return ('box', b), Vol(m_pt(m, b[0:3]), [m_vec(m, b[3:6]), m_vec(m, b[6:9]), m_vec(m, b[9:12])])
    if ok('region', 6):
        r = [float(x) for x in bv['region']]
        return ('region', r), region_vol(r)
    if ok('sphere', 4) and bv['sphere'][3] >= 0:
        s = [float(x) for x in bv['sphere']]
        sc = max(math.sqrt(sum(m[r][c] ** 2 for r in range(3))) for c in range(3))
        return ('sphere', s), Vol(m_pt(m, s[0:3]), r=s[3] * sc)
    return None


def sub_given(given, level, x, y, z, octree):
    kind, g = given
    n = 2.0 ** level
    if kind == 'box':
        out = list(g)
        for k, idx in enumerate((x, y, z)):
            if k == 2 and not octree:
                continue
            f = -1 + 2 * (idx + 0.5) / n
            for d in range(3):
                out[d] += g[3 + 3 * k + d] * f
                out[3 + 3 * k + d] = g[3 + 3 * k + d] / n
        return ('box', out)
    if kind == 'region':
        w, s, e, nn, lo, hi = g
        if e < w:
            e += 2 * math.pi
        def part(a, b, i):
            return a + (b - a) * i / n, a + (b - a) * (i + 1) / n
        w2, e2 = part(w, e, x)
        s2, n2 = part(s, nn, y)
        lo2, hi2 = part(lo, hi, z) if octree else (lo, hi)
        wrap = lambda v: v - 2 * math.pi if v > math.pi else v
        return ('region', [wrap(w2), s2, wrap(e2), n2, lo2, hi2])
    return None


def given_world(given, m):
    kind, g = given
    return to_vol({kind: g}, m)[1]


def resolve(base, uri):
    uri = uri.strip()
    if uri.startswith('data:'):
        return uri
    if '://' in uri or uri.startswith('/') or '\\' in uri:
        return None
    uri = urllib.parse.unquote(uri)
    for sep in '?#':
        uri = uri.split(sep)[0]
    parts = [p for p in base.split('/')[:-1] if p]
    for p in uri.split('/'):
        if p in ('', '.'):
            continue
        if p == '..':
            if not parts:
                return None
            parts.pop()
        else:
            parts.append(p)
    return '/'.join(parts)


class Reader:
    def __init__(self, root):
        self.root = Path(root)

    def read(self, key):
        if key.startswith('data:'):
            import base64
            meta, data = key[5:].split(',', 1)
            return base64.b64decode(data) if meta.endswith(';base64') else urllib.parse.unquote(data).encode()
        p = (self.root / key).resolve()
        if not str(p).startswith(str(self.root.resolve())) or not p.is_file():
            return None
        return p.read_bytes()


def uris_of(t):
    out = []
    def one(c):
        if isinstance(c, dict):
            u = c.get('uri', c.get('url'))
            if isinstance(u, str):
                out.append(u)
    one(t.get('content'))
    for c in t.get('contents', []) or []:
        one(c)
    ext = (t.get('extensions') or {}).get('3DTILES_multiple_contents') or {}
    for c in ext.get('contents', ext.get('content', [])) or []:
        one(c)
    return out


def read_subtree(data, key, reader):
    if data[:4] == b'subt':
        jl, bl = struct.unpack('<QQ', data[8:24])
        js = json.loads(data[24:24 + jl])
        binary = data[24 + jl:24 + jl + bl]
    else:
        js, binary = json.loads(data), b''
    bufs = []
    for i, b in enumerate(js.get('buffers', [])):
        if 'uri' in b:
            k = resolve(key, b['uri'])
            bufs.append(reader.read(k) if k else None)
        else:
            bufs.append(binary if i == 0 else None)
    views = js.get('bufferViews', [])
    def bits(a):
        if a is None:
            return None
        if 'constant' in a:
            return a['constant'] == 1
        v = views[a.get('bitstream', a.get('bufferView'))]
        buf = bufs[v.get('buffer', 0)]
        off = v.get('byteOffset', 0)
        return buf[off:off + v['byteLength']]
    ca = js.get('contentAvailability')
    if ca is None:
        ca = ((js.get('extensions') or {}).get('3DTILES_multiple_contents') or {}).get('contentAvailability')
    if isinstance(ca, dict):
        ca = [ca]
    return {'tiles': bits(js['tileAvailability']),
            'contents': [bits(c) for c in (ca or [])],
            'children': bits(js['childSubtreeAvailability'])}


def bit(b, i):
    if b is True or b is False:
        return b
    return i // 8 < len(b) and (b[i // 8] >> (i % 8)) & 1 == 1


def morton(x, y, z, octree):
    out = 0
    for i in range(21 if octree else 32):
        if octree:
            out |= ((x >> i) & 1) << (3 * i) | ((y >> i) & 1) << (3 * i + 1) | ((z >> i) & 1) << (3 * i + 2)
        else:
            out |= ((x >> i) & 1) << (2 * i) | ((y >> i) & 1) << (2 * i + 1)
    return out


def fill(t, level, x, y, z):
    return t.replace('{level}', str(level)).replace('{x}', str(x)).replace('{y}', str(y)).replace('{z}', str(z))


class Node:
    def __init__(self, vol, err, refine, contents, request=None):
        self.vol, self.err, self.refine, self.contents = vol, err, refine, contents
        self.request = request
        self.kids = []
        self.through = False


def build(doc, key, parent_m, inherit, reader):
    """Explicit tiles (recursively), external tilesets and implicit trees, fully read."""
    def tile(t, pm, inh, depth):
        if depth > 256:
            return None
        m = pm
        if isinstance(t.get('transform'), list) and len(t['transform']) == 16:
            m = m_mul(pm, m_from_cols([float(v) for v in t['transform']]))
        gv = to_vol(t.get('boundingVolume'), m)
        if gv is None:
            return None
        given, vol = gv
        refine = (t.get('refine') or inh).upper()
        if refine not in ('ADD', 'REPLACE'):
            refine = inh
        err = t.get('geometricError', 0.0)
        req = to_vol(t.get('viewerRequestVolume'), m)
        imp = t.get('implicitTiling') or (t.get('extensions') or {}).get('3DTILES_implicit_tiling')
        node = Node(vol, err, refine, [], req[1] if req else None)
        if imp:
            node.through = True
            implicit(node, imp, t, key, given, m, err, refine)
        else:
            ext = None
            for u in uris_of(t):
                k = resolve(key, u)
                if k is None:
                    continue
                if k.lower().endswith('.json'):
                    ext = ext or k
                else:
                    node.contents.append(k)
            if ext:
                node.through = True
                data = reader.read(ext)
                if data is not None:
                    sub = json.loads(data)
                    child = build(sub, ext, m, refine, reader)
                    if child:
                        node.kids.append(child)
        for c in t.get('children', []) or []:
            k = tile(c, m, refine, depth + 1)
            if k:
                node.kids.append(k)
        return node

    def implicit(node, imp, t, base, given, m, err, refine):
        octree = imp['subdivisionScheme'] == 'OCTREE'
        levels = imp['subtreeLevels']
        avail = imp.get('availableLevels', imp.get('maximumLevel', -1) + 1)
        st_tpl = imp['subtrees'].get('uri', imp['subtrees'].get('url'))
        c_tpls = uris_of(t)
        n = 8 if octree else 4

        def load_sub(level, x, y, z):
            k = resolve(base, fill(st_tpl, level, x, y, z))
            data = reader.read(k) if k else None
            return read_subtree(data, k, reader) if data is not None else None

        def make(level, x, y, z, sub, root):
            rl, rx, ry, rz = root
            sh = level - rl
            local = sh
            idx = (n ** local - 1) // (n - 1) + morton(x - (rx << sh), y - (ry << sh), z - (rz << sh), octree)
            cs = []
            for ci, tpl in enumerate(c_tpls):
                if ci < len(sub['contents']) and bit(sub['contents'][ci], idx):
                    k = resolve(base, fill(tpl, level, x, y, z))
                    if k:
                        cs.append(k)
            g = sub_given(given, level, x, y, z, octree)
            nd = Node(given_world(g, m), err / 2 ** level, refine, cs)
            if level + 1 < avail:
                for c in range(n):
                    cx, cy = 2 * x + (c & 1), 2 * y + ((c >> 1) & 1)
                    cz = 2 * z + ((c >> 2) & 1) if octree else 0
                    if local + 1 < levels:
                        j = (n ** (local + 1) - 1) // (n - 1) + morton(cx - (rx << (sh + 1)), cy - (ry << (sh + 1)), cz - (rz << (sh + 1)), octree)
                        if bit(sub['tiles'], j):
                            nd.kids.append(make(level + 1, cx, cy, cz, sub, root))
                    else:
                        j = morton(cx - (rx << (sh + 1)), cy - (ry << (sh + 1)), cz - (rz << (sh + 1)), octree)
                        if bit(sub['children'], j):
                            s2 = load_sub(level + 1, cx, cy, cz)
                            if s2 and bit(s2['tiles'], 0):
                                nd.kids.append(make(level + 1, cx, cy, cz, s2, (level + 1, cx, cy, cz)))
            return nd

        s0 = load_sub(0, 0, 0, 0)
        if s0 and bit(s0['tiles'], 0):
            node.kids.append(make(0, 0, 0, 0, s0, (0, 0, 0, 0)))

    return tile(doc['root'], parent_m, inherit, 0)


def count(n):
    t, c = 1, len(n.contents)
    for k in n.kids:
        a, b = count(k)
        t, c = t + a, c + b
    return t, c


def sees(v, c, r):
    f, up = v_unit(v['dir']), v_unit(v['up'])
    right = v_unit(v_cross(f, up))
    d = v_sub(c, v['eye'])
    z = v_dot(d, f)
    if z < -r:
        return False
    aspect = v['width'] / max(v['height'], 1.0)
    x, y = v_dot(d, right), v_dot(d, up)
    if v['ortho_height'] is not None:
        hh = v['ortho_height'] / 2
        return abs(x) <= hh * aspect + r and abs(y) <= hh + r
    ty = math.tan(v['fov_y'] / 2)
    tx = ty * aspect
    return (abs(x) - tx * z) / math.sqrt(1 + tx * tx) <= r and (abs(y) - ty * z) / math.sqrt(1 + ty * ty) <= r


def sse(v, err, d):
    if v['ortho_height'] is not None:
        return err * v['height'] / max(v['ortho_height'], 1e-12)
    return err * v['height'] / (2 * max(d, 1e-7) * math.tan(v['fov_y'] / 2))


def select(n, v, out):
    if not sees(v, n.vol.c, n.vol.radius()):
        return
    if n.request is not None and n.request.dist(v['eye']) > 0:
        return
    refine = (n.through or sse(v, n.err, n.vol.dist(v['eye'])) > v['max_sse']) and n.kids
    if (not refine or n.refine == 'ADD') and n.contents:
        out.extend(n.contents)
    if refine:
        for k in n.kids:
            select(k, v, out)


def key_name(k):
    return 'data:%d' % len(k) if k.startswith('data:') else k


def main():
    corpus, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    views = json.loads((out_dir / 'views.json').read_text())
    results = []
    bad_counts = bad_sel = total_sel = 0
    for entry in views:
        path = corpus / entry['path']
        reader = Reader(path.parent)
        doc = json.loads(path.read_text())
        try:
            root = build(doc, path.name, IDENT, 'REPLACE', reader)
        except Exception as e:  # a tileset the oracle cannot read either
            results.append({'path': entry['path'], 'refused': str(e)})
            continue
        if root is None:
            results.append({'path': entry['path'], 'refused': 'no root bounding volume'})
            continue
        tiles, contents = count(root)
        sels = []
        for v in entry.get('views', []):
            out = []
            select(root, v, out)
            sels.append(sorted(key_name(k) for k in out))
        results.append({'path': entry['path'], 'tiles': tiles, 'contents': contents, 'selections': sels})
        if 'refused' in entry:
            continue
        if tiles != entry['tiles'] or contents != entry['contents']:
            bad_counts += 1
            print('%s: tiles %s/%s contents %s/%s' % (entry['path'], entry['tiles'], tiles, entry['contents'], contents))
        for i, (a, b) in enumerate(zip(entry['selections'], sels)):
            total_sel += 1
            if a != b:
                bad_sel += 1
                print('%s view %d: ours %d, oracle %d' % (entry['path'], i, len(a), len(b)))
    (out_dir / 'oracle.json').write_text(json.dumps(results, indent=1))
    n_sel_tiles = sum(len(s) for r in results for s in r.get('selections', []))
    print('%d tilesets; tile/content count mismatches %d; selections %d (%d contents chosen), mismatches %d'
          % (len(results), bad_counts, total_sel, n_sel_tiles, bad_sel))


if __name__ == '__main__':
    main()
