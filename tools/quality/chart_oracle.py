"""Oracle for the chart profile solver (specular-develop `chart`): colour-science's
Cheung 2004 matrix fit (3 terms), Finlayson 2015 root-polynomial fit (degree 2) and
CIEDE2000, on the patches the Rust test writes.

    python chart_oracle.py <in.json>   ->  JSON on stdout

Input: {"rgb": [[r,g,b],...], "xyz": [[x,y,z],...], "lab_pairs": [[[L,a,b],[L,a,b]],...]}
Output: {"cheung": 3x3, "root2": 3x6, "de2000": [...], "root2_terms": [[...6]]}
"""

import json
import sys
import warnings

import numpy as np

warnings.filterwarnings("ignore")
import colour  # noqa: E402
from colour.characterisation import (  # noqa: E402
    matrix_colour_correction_Cheung2004,
    matrix_colour_correction_Finlayson2015,
    polynomial_expansion_Finlayson2015,
)


def main():
    with open(sys.argv[1], encoding="utf-8") as f:
        d = json.load(f)
    rgb = np.array(d["rgb"], dtype=float)
    xyz = np.array(d["xyz"], dtype=float)
    cheung = matrix_colour_correction_Cheung2004(rgb, xyz, terms=3)
    root2 = matrix_colour_correction_Finlayson2015(
        rgb, xyz, degree=2, root_polynomial_expansion=True
    )
    terms = polynomial_expansion_Finlayson2015(rgb[:1], degree=2, root_polynomial_expansion=True)
    pairs = np.array(d["lab_pairs"], dtype=float)
    de = colour.difference.delta_E_CIE2000(pairs[:, 0], pairs[:, 1])
    json.dump(
        {
            "cheung": cheung.tolist(),
            "root2": root2.tolist(),
            "root2_terms": terms.tolist(),
            "de2000": np.atleast_1d(de).tolist(),
        },
        sys.stdout,
    )


if __name__ == "__main__":
    main()
