# Provenance of the IQI wire tables in `inspect.rs`

`WIRES` (ISO 19232-1, single-wire IQI, W1–W19) and `DUPLEX` (ISO 19232-5, duplex-wire IQI,
D1–D15) hold nominal wire diameters only. They are facts, not text of the standards.

- Single wires: 3.20, 2.50, 2.00, 1.60, 1.25, 1.00, 0.80, 0.63, 0.50, 0.40, 0.32, 0.25, 0.20,
  0.16, 0.125, 0.100, 0.080, 0.063, 0.050 mm: the R10 preferred-number series (ISO 3),
  as printed in the public summary of ISO 19232-1 and in IQI makers' data sheets.
- Duplex pairs: 0.80, 0.63, 0.50, 0.40, 0.32, 0.25, 0.20, 0.16, 0.13, 0.10, 0.080, 0.063,
  0.050 mm (D1–D13), with 0.040 and 0.032 mm (D14, D15) added in ISO 19232-5:2018. The gap
  in a pair equals the wire diameter; the unsharpness of a pair is twice it.
- Entered by hand on 2026-10-25 from those public listings; no table was copied from code.
