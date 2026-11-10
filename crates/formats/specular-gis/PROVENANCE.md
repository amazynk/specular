# Provenance of specular-gis's data

The readers are this crate's own. One table is data taken from elsewhere.

## `src/s57_catalogue.rs`

- **What:** the S-57 object class and attribute codes, acronyms and names (IHO S-57 Edition 3.1 Appendix A, with the Inland ENC and AML additions). The codes themselves are the IHO standard's.
- **Taken from:** the catalogue files GDAL ships as data, `s57objectclasses.csv` and `s57attributes.csv`, as bundled in the `gdal_data` folder of the pyogrio 0.13.0 wheel (GDAL 3.x). GDAL's data files are under the MIT licence.
- **How:** `gen/s57_catalogue.py` writes `src/s57_catalogue.rs` from that folder:
  `python3 -I gen/s57_catalogue.py <venv>/…/pyogrio/gdal_data src/s57_catalogue.rs`.
- **Status:** data only, no GDAL code. GDAL's MIT notice is in `packaging/THIRD-PARTY.md`.
