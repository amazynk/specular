"""ALF fixed filter tables of ITU-T H.266 (V4, 01/2026) as Rust (equations 90 and 91).

Usage: python3 -I tools/corpus/vvc-alf-tables.py h266.txt > crates/formats/specular-vvc/src/filter/alf_tables.rs
where h266.txt is `pdftotext -layout` of T-REC-H.266-202601-I!!PDF-E (free from itu.int).

The Recommendation prints both arrays transposed: AlfFixFiltCoeff[ i ][ j ] (i = 0..63, j = 0..11)
as 12 rows of 64 values (row = j), AlfClassToFiltMap[ m ][ n ] (m = 0..15, n = 0..24) as 25 rows
of 16 values (row = n). The output is indexed as the equations are: [i][j] and [m][n].
"""
import re
import sys

L = open(sys.argv[1], encoding='utf-8').read().replace('−', '-').split('\n')
a = next(i for i, l in enumerate(L) if 'AlfFixFiltCoeff[ ][ ] =' in l)
b = next(i for i in range(a, len(L)) if 'AlfClassToFiltMap[ ][ ] =' in L[i])
c = next(i for i in range(b, len(L)) if 'It is a requirement of bitstream conformance' in L[i])


def groups(lo, hi):
    txt = '\n'.join(l for l in L[lo:hi] if 'Rec. ITU-T' not in l)
    # Drop the equation numbers such as "(90)" before collecting the brace groups.
    txt = re.sub(r'\(\d+\)', '', txt)
    return [[int(x) for x in g.replace(',', ' ').split()] for g in re.findall(r'\{([-\d,\s]+)\}', txt)]


coef = groups(a, b)
cmap = groups(b, c)
assert len(coef) == 12 and all(len(r) == 64 for r in coef), [len(r) for r in coef]
assert len(cmap) == 25 and all(len(r) == 16 for r in cmap), [len(r) for r in cmap]
print('//! ALF fixed filters (equations 90 and 91 of ITU-T H.266 V4), extracted from the')
print("//! Recommendation's text by tools/corpus/vvc-alf-tables.py; data only.\n")
print('/// AlfFixFiltCoeff[ i ][ j ].')
print('pub static FIX_FILT_COEFF: [[i8; 12]; 64] = [')
for i in range(64):
    print('    [' + ', '.join(str(coef[j][i]) for j in range(12)) + '],')
print('];')
print('/// AlfClassToFiltMap[ m ][ n ].')
print('pub static CLASS_TO_FILT_MAP: [[u8; 25]; 16] = [')
for m in range(16):
    print('    [' + ', '.join(str(cmap[n][m]) for n in range(25)) + '],')
print('];')
