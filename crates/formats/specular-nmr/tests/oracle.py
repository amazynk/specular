"""nmrglue's reading of Bruker experiments, the oracle for tests/oracle.rs.

Usage: python -I oracle.py CORPUS_DIR EXPECT_DIR

For every experiment folder under CORPUS_DIR (an `acqus` beside `fid` or `ser`, or a
`pdata/<n>` with `1r`/`2rr`), writes EXPECT_DIR/<relative path>/ with:
  meta.json    sizes, group delay, the processing settings used, ppm limits from nmrglue's
               unit conversion
  fid.f64      the first FID, interleaved real and imaginary (little-endian f64)
  mag.f64      |spectrum| of that FID processed by nmrglue: em, zf_size, fft, rm_dig_filter
               (post_proc, untruncated delay), in Bruker order (highest ppm first)
  <n>.1r.f64   pdata/<n>/1r scaled by 2^NC_proc (read_pdata scale_data=True)
  <n>.2rr.f64  pdata/<n>/2rr, de-tiled, scaled, row-major (F1 rows, F2 columns)

The processing settings follow specular-nmr's rules (lib.rs `settings`): size SI from procs
(else TD), at least TD/2, to a power of two; LB from procs when WDW is 1, 0 when WDW is 0, else 0.3 Hz for
1H/19F/3H and 1 Hz otherwise.
"""
import json
import os
import sys

import numpy as np
import nmrglue as ng


def pow2(n):
    p = 1
    while p < n:
        p *= 2
    return p


def first_procs(exp):
    pd = os.path.join(exp, 'pdata')
    if not os.path.isdir(pd):
        return None
    for n in sorted((int(x) for x in os.listdir(pd) if x.isdigit())):
        p = os.path.join(pd, str(n), 'procs')
        if os.path.isfile(p):
            return ng.bruker.read_jcamp(p)
    return None


def write(path, arr):
    np.asarray(arr, dtype='<f8').tofile(path)


def one(exp, out):
    os.makedirs(out, exist_ok=True)
    meta = {}
    acqus = ng.bruker.read_jcamp(os.path.join(exp, 'acqus'))
    procs = first_procs(exp)
    for name in ('fid', 'ser'):
        f = os.path.join(exp, name)
        if not os.path.isfile(f):
            continue
        dic, data = ng.bruker.read(exp, read_pulseprogram=False)
        fid = data if data.ndim == 1 else data.reshape(-1, data.shape[-1])[0]
        td = int(acqus['TD']) // 2
        fid = fid[:td]
        inter = np.empty(2 * len(fid))
        inter[0::2] = fid.real
        inter[1::2] = fid.imag
        write(os.path.join(out, 'fid.f64'), inter)
        nuc = acqus.get('NUC1', '')
        default_lb = 0.3 if nuc in ('1H', '19F', '3H') else 1.0
        si = int(procs['SI']) if procs and int(procs.get('SI', 0)) >= 8 else int(acqus['TD'])
        size = min(max(pow2(max(si, int(acqus['TD']) // 2)), 8), 1 << 20)
        wdw = int(procs['WDW']) if procs else -1
        lb = float(procs['LB']) if wdw == 1 else (0.0 if wdw == 0 else default_lb)
        sw = float(acqus['SW_h'])
        grpdly = float(acqus.get('GRPDLY', -1))
        digmod = int(acqus.get('DIGMOD', 1))
        dspfvs = int(acqus.get('DSPFVS', 0))
        decim = float(acqus.get('DECIM', 1))
        if digmod == 0:
            gd = 0.0
        elif grpdly > 0:
            gd = grpdly
        elif dspfvs >= 14 or decim <= 1:
            gd = 0.0
        else:
            try:
                gd = ng.bruker.bruker_dsp_table[dspfvs][int(decim)]
            except KeyError:
                gd = 0.0
        s = ng.proc_base.em(fid.astype(complex), max(lb, 0) / sw)
        s = ng.proc_base.zf_size(s, size)
        s = ng.proc_base.fft(s)
        if gd:
            s = ng.bruker.rm_dig_filter(s, decim, dspfvs, gd, truncate_grpdly=False, post_proc=True)
        n = len(s)
        s = s[(n - np.arange(n)) % n]
        write(os.path.join(out, 'mag.f64'), np.abs(s))
        meta['fid'] = {'points': len(fid), 'size': size, 'lb': lb, 'group_delay': gd}
        break
    pdata = os.path.join(exp, 'pdata')
    meta['procnos'] = {}
    if os.path.isdir(pdata):
        for n in sorted((int(x) for x in os.listdir(pdata) if x.isdigit())):
            d = os.path.join(pdata, str(n))
            if os.path.isfile(os.path.join(d, '2rr')):
                files = ['2rr']
            elif os.path.isfile(os.path.join(d, '1r')):
                files = ['1r']
            else:
                continue
            pdic, pd = ng.bruker.read_pdata(d, bin_files=files, scale_data=True)
            udic = ng.bruker.guess_udic(pdic, pd)
            limits = []
            for ax in range(pd.ndim):
                uc = ng.fileiobase.uc_from_udic(udic, dim=ax)
                sc = uc.ppm_scale()
                limits.append([float(sc[0]), float(sc[-1])])
            write(os.path.join(out, f'{n}.{files[0]}.f64'), pd.reshape(-1))
            meta['procnos'][str(n)] = {'file': files[0], 'shape': list(pd.shape), 'ppm': limits}
    with open(os.path.join(out, 'meta.json'), 'w') as f:
        json.dump(meta, f, indent=1)


def experiments(root):
    for d, subdirs, files in os.walk(root):
        subdirs.sort()
        if 'acqus' in files and ('fid' in files or 'ser' in files or 'pdata' in subdirs):
            yield d


def main():
    root, expect = sys.argv[1], sys.argv[2]
    n = bad = 0
    for exp in experiments(root):
        rel = os.path.relpath(exp, root)
        try:
            one(exp, os.path.join(expect, rel))
            n += 1
        except Exception as e:
            bad += 1
            print(f'{rel}: nmrglue failed: {e}', file=sys.stderr)
    print(f'{n} experiments written, {bad} nmrglue could not read')


if __name__ == '__main__':
    main()
