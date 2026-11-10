// SHX oracle: renders every shape of the given fonts with @mlightcad/shx-parser (MIT, an
// independent reader, used only here as a test oracle) and prints JSON: per font and code,
// the pen's end and the bounding box of the strokes, in shape units.
//
//   tools/corpus/fetch-shx-oracle.sh            # once: the package under $CARGO_TARGET_DIR
//   node tools/corpus/shx-oracle.mjs FONT.shx... > oracle.json
//   SHX_ORACLE=oracle.json cargo test -p specular-vector --test shx_fonts -- --ignored

import { readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

const target = process.env.CARGO_TARGET_DIR ?? "target";
const mod = join(target, "ref", "shxp", "package", "dist", "index.es.js");
const { ShxFont } = await import(pathToFileURL(mod).href);

const out = {};
for (const path of process.argv.slice(2)) {
  const b = readFileSync(path);
  const font = new ShxFont(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
  const unit = font.fontData.content.height;
  const glyphs = {};
  for (const code of Object.keys(font.fontData.content.data).map(Number)) {
    if (code === 0) continue;
    // The parser itself, not ShxFont.getCharShape, which moves Big Font shapes.
    const s = font.shapeParser.getCharShape(code, unit);
    if (!s) continue;
    const bb = s.polylines.length ? s.bbox : null;
    glyphs[code] = {
      end: s.lastPoint ? [s.lastPoint.x, s.lastPoint.y] : null,
      bbox: bb ? [bb.minX, bb.minY, bb.maxX, bb.maxY] : null,
      lines: s.polylines.length,
    };
  }
  out[basename(path)] = glyphs;
}
console.log(JSON.stringify(out, null, 1));
