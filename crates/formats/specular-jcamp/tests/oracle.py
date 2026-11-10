"""Writes the oracles' reading of every JCAMP-DX and SPC file under the given folders as
JSON, one file per input, for tests/oracle.rs.

JCAMP-DX: the `jcamp` package (x and y of each block, LINK children too) and nmrglue's
jcampdx reader (ordinates: the real part, and the imaginary one of NTUPLES). SPC: spc_spectra
(x and y of each subfile).

Usage (the oracle venv has all three):
  target/pcb-spectra/venv/bin/python -I crates/formats/specular-jcamp/tests/oracle.py \
      OUT_DIR tools/corpus/pcb-spectra/jcamp crates/formats/specular-jcamp/tests/data
Each output is OUT_DIR/<input path relative to its folder, '/' as '__'>.json.
"""
import contextlib
import io
import json
import math
import os
import sys
import warnings

import numpy as np


def arr(a):
    if a is None:
        return None
    v = np.asarray(a, dtype=float).ravel().tolist()
    return [None if (isinstance(x, float) and not math.isfinite(x)) else x for x in v]


def quiet(f, *a):
    """Calls f without its prints and warnings; the exception text on failure."""
    with warnings.catch_warnings():
        warnings.simplefilter('ignore')
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            try:
                return f(*a), None
            except Exception as e:  # noqa: BLE001 - any oracle failure is recorded
                return None, f'{type(e).__name__}: {e}'


def jcamp_blocks(d):
    out = []
    if d is None:
        return out
    if 'children' in d and d['children']:
        for c in d['children']:
            out.extend(jcamp_blocks(c))
        return out
    x, y = d.get('x'), d.get('y')
    if y is not None and len(y):
        out.append({'x': arr(x), 'y': arr(y), 'xunits': str(d.get('xunits', ''))})
    return out


def do_jcamp(path):
    res = {}
    import jcamp
    d, err = quiet(jcamp.readfile, path)
    res['jcamp'] = {'blocks': jcamp_blocks(d)} if d is not None else {'error': err}
    from nmrglue.fileio import jcampdx
    r, err = quiet(jcampdx.read, path)
    if r is None:
        res['nmrglue'] = {'error': err}
    else:
        dic, data = r
        if data is None:
            res['nmrglue'] = {'error': 'no data'}
        elif isinstance(data, list):
            res['nmrglue'] = {'real': arr(data[0]), 'imag': arr(data[1]) if len(data) > 1 else None}
        else:
            res['nmrglue'] = {'real': arr(data)}
    return res


class _NumpyFixed:
    """numpy for spc_spectra's old-format reader, whose `np.int32(list)` of unsigned 32-bit
    words overflows under NumPy 2: the same words reinterpreted as signed, as NumPy 1 did."""

    def __getattr__(self, name):
        return getattr(np, name)

    @staticmethod
    def int32(v):
        return np.asarray(v, dtype=np.int64).astype(np.uint32).view(np.int32)


def do_spc(path):
    import spc_spectra
    import spc_spectra.sub
    spc_spectra.sub.np = _NumpyFixed()
    f, err = quiet(spc_spectra.File, path)
    if f is None or not hasattr(f, 'sub'):
        return {'spc': {'error': err or 'not read'}}
    subs = []
    for s in f.sub:
        x = getattr(s, 'x', None)
        if x is None:
            x = getattr(f, 'x', None)
        subs.append({'x': arr(x), 'y': arr(s.y)})
    return {'spc': {'subs': subs}}


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    n = 0
    for root in sys.argv[2:]:
        for dirpath, _, files in sorted(os.walk(root)):
            for name in sorted(files):
                p = os.path.join(dirpath, name)
                ext = name.lower().rsplit('.', 1)[-1]
                if ext in ('jdx', 'dx', 'jcamp'):
                    res = do_jcamp(p)
                elif ext == 'spc':
                    res = do_spc(p)
                else:
                    continue
                rel = os.path.relpath(p, root).replace(os.sep, '__')
                res['file'] = os.path.abspath(p)
                with open(os.path.join(out, rel + '.json'), 'w') as fh:
                    json.dump(res, fh)
                n += 1
    print(f'{n} files')


main()
