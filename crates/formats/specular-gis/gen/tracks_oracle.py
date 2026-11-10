"""The tracks oracle: what fitdecode reads from FIT files, and what ElementTree reads from
TCX, GPX (TrackPointExtension) and KML (gx:Track, GroundOverlay) files, written to
CORPUS/expected/tracks.jsonl for crates/formats/specular-gis/tests/tracks_oracle.rs.

Usage: python -I tracks_oracle.py CORPUS_DIR

Also writes two small synthetic files under CORPUS/gen/tracks/ when absent (a GPX with
Garmin TrackPointExtension v2 and GpxExtensions v3 values, a KML with gx:Track,
gx:MultiTrack, a GroundOverlay and a gx:LatLonQuad), so the extensions have a fixture.

Reads the corpus as data only; nothing from it is executed.
"""

import json
import math
import os
import sys
import xml.etree.ElementTree as ET

import fitdecode

SEMI = 180.0 / 2**31


def local(tag):
    return tag.rsplit("}", 1)[-1]


def fit(path):
    records = 0
    pos = []
    sessions = []
    file_type = None
    error = None
    try:
        with fitdecode.FitReader(
            path,
            check_crc=fitdecode.CrcCheck.DISABLED,
            keep_raw_chunks=False,
        ) as r:
            for frame in r:
                if not isinstance(frame, fitdecode.FitDataMessage):
                    continue
                if frame.name == "file_id" and file_type is None:
                    file_type = frame.get_value("type", fallback=None)
                if frame.name == "record":
                    records += 1
                    lat = frame.get_value("position_lat", fallback=None)
                    lon = frame.get_value("position_long", fallback=None)
                    if isinstance(lat, int) and isinstance(lon, int):
                        pos.append([lon * SEMI, lat * SEMI])
                elif frame.name == "session":
                    d = frame.get_value("total_distance", fallback=None)
                    t = frame.get_value("total_elapsed_time", fallback=None)
                    sessions.append(
                        {
                            "sport": str(frame.get_value("sport", fallback=None)),
                            "distance": d if isinstance(d, (int, float)) else None,
                            "elapsed": t if isinstance(t, (int, float)) else None,
                        }
                    )
    except Exception as e:  # noqa: BLE001 - damaged files: what was read so far
        error = str(e)[:200]
    return {
        "kind": "fit",
        "records": records,
        "positioned": len(pos),
        "first": pos[0] if pos else None,
        "last": pos[-1] if pos else None,
        "sample": pos[:: max(1, len(pos) // 50)][:60],
        "sessions": sessions,
        "file_type": str(file_type),
        "error": error,
    }


def tcx(path):
    root = ET.parse(path).getroot()
    tracks = []
    for el in root.iter():
        if local(el.tag) not in ("Activity", "Course"):
            continue
        pts, n = [], 0
        for tp in el.iter():
            if local(tp.tag) != "Trackpoint":
                continue
            n += 1
            lat = lon = None
            for c in tp.iter():
                if local(c.tag) == "LatitudeDegrees":
                    lat = float(c.text)
                elif local(c.tag) == "LongitudeDegrees":
                    lon = float(c.text)
            if lat is not None and lon is not None:
                pts.append([lon, lat])
        tracks.append({"trackpoints": n, "positioned": len(pts), "first": pts[0] if pts else None,
                       "last": pts[-1] if pts else None})
    return {"kind": "tcx", "tracks": tracks}


def gpx(path):
    root = ET.parse(path).getroot()
    tracks = []
    for trk in root:
        if local(trk.tag) != "trk":
            continue
        n, hr, cad, temp = 0, [], [], []
        for p in trk.iter():
            if local(p.tag) != "trkpt":
                continue
            n += 1
            for e in p.iter():
                t = local(e.tag)
                if t == "hr":
                    hr.append(float(e.text))
                elif t == "cad":
                    cad.append(float(e.text))
                elif t in ("atemp", "Temperature"):
                    temp.append(float(e.text))
        tracks.append({
            "points": n,
            "avg_hr": sum(hr) / len(hr) if hr else None,
            "max_hr": max(hr) if hr else None,
            "avg_cadence": sum(cad) / len(cad) if cad else None,
            "min_temp": min(temp) if temp else None,
            "max_temp": max(temp) if temp else None,
        })
    return {"kind": "gpx", "tracks": tracks}


def kml(path):
    root = ET.parse(path).getroot()
    coords = sum(1 for e in root.iter() if local(e.tag) == "coord")
    overlays = []
    for go in root.iter():
        if local(go.tag) != "GroundOverlay":
            continue
        o = {"href": None, "box": None, "quad": None}
        for e in go.iter():
            t = local(e.tag)
            if t == "href":
                o["href"] = e.text.strip()
            elif t == "LatLonBox":
                o["box"] = {local(c.tag): float(c.text) for c in e}
            elif t == "LatLonQuad":
                for c in e:
                    if local(c.tag) == "coordinates":
                        o["quad"] = [[float(v) for v in t.split(",")[:2]] for t in c.text.split()]
        if o["href"] and (o["box"] or o["quad"]):
            overlays.append(o)
    return {"kind": "kml", "track_coords": coords, "overlays": overlays}


GPX = """<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="specular tracks_oracle" xmlns="http://www.topografix.com/GPX/1/1"
 xmlns:gpxtpx="http://www.garmin.com/xmlschemas/TrackPointExtension/v2"
 xmlns:gpxx="http://www.garmin.com/xmlschemas/GpxExtensions/v3">
 <trk><name>Synthetic ride</name><trkseg>
{points}
 </trkseg></trk>
</gpx>
"""

KML = """<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2">
<Document><name>Synthetic</name>
 <Placemark><name>Track</name><gx:Track>
{whens}
{coords}
 </gx:Track></Placemark>
 <Placemark><name>Multi</name><gx:MultiTrack>
  <gx:Track><gx:coord>8.0 47.0 0</gx:coord><gx:coord>8.1 47.0 0</gx:coord></gx:Track>
  <gx:Track><gx:coord>8.2 47.0 0</gx:coord><gx:coord>8.3 47.1 0</gx:coord></gx:Track>
 </gx:MultiTrack></Placemark>
 <GroundOverlay><name>Box</name><Icon><href>overlay.png</href></Icon>
  <LatLonBox><north>47.6</north><south>47.2</south><east>8.9</east><west>8.3</west><rotation>12.5</rotation></LatLonBox>
 </GroundOverlay>
 <GroundOverlay><name>Quad</name><Icon><href>files/quad.jpg</href></Icon>
  <gx:LatLonQuad><coordinates>8.0,46.0 8.5,46.1 8.6,46.5 7.9,46.4</coordinates></gx:LatLonQuad>
 </GroundOverlay>
</Document></kml>
"""


def synthesize(root):
    d = os.path.join(root, "gen", "tracks")
    os.makedirs(d, exist_ok=True)
    g = os.path.join(d, "tpx_ride.gpx")
    if not os.path.exists(g):
        pts = []
        for i in range(120):
            lat = 47.0 + 0.0005 * i
            lon = 8.0 + 0.0007 * i + 0.0002 * math.sin(i / 5)
            t = f"2024-05-01T10:{i // 60:02d}:{i % 60:02d}Z"
            hr, cad, temp = 100 + i % 40, 70 + i % 25, 15 + i % 9
            ext = (f"<gpxtpx:TrackPointExtension><gpxtpx:hr>{hr}</gpxtpx:hr>"
                   f"<gpxtpx:cad>{cad}</gpxtpx:cad></gpxtpx:TrackPointExtension>")
            ext += (f"<gpxx:TrackPointExtension><gpxx:Temperature>{temp}</gpxx:Temperature>"
                    "</gpxx:TrackPointExtension>") if i % 2 else (
                f"<gpxtpx:TrackPointExtension><gpxtpx:atemp>{temp}</gpxtpx:atemp></gpxtpx:TrackPointExtension>")
            pts.append(f'  <trkpt lat="{lat:.7f}" lon="{lon:.7f}"><ele>400</ele><time>{t}</time>'
                       f"<extensions>{ext}</extensions></trkpt>")
        open(g, "w").write(GPX.format(points="\n".join(pts)))
    k = os.path.join(d, "gx_track_overlays.kml")
    if not os.path.exists(k):
        whens = "\n".join(f"  <when>2024-05-01T10:00:{i:02d}Z</when>" for i in range(30))
        coords = "\n".join(f"  <gx:coord>{8.0 + 0.001 * i:.6f} {47.0 + 0.0005 * i:.6f} 400</gx:coord>"
                           for i in range(30))
        open(k, "w").write(KML.format(whens=whens, coords=coords))


def main():
    root = sys.argv[1]
    synthesize(root)
    os.makedirs(os.path.join(root, "expected"), exist_ok=True)
    n = 0
    with open(os.path.join(root, "expected", "tracks.jsonl"), "w") as out:
        for sub in ("fit", "tcx", "gdal/gpx", "gdal/kml", "gen/tracks"):
            base = os.path.join(root, sub)
            if not os.path.isdir(base):
                continue
            for name in sorted(os.listdir(base)):
                path = os.path.join(base, name)
                ext = os.path.splitext(name)[1].lower()
                try:
                    if ext == ".fit":
                        rec = fit(path)
                    elif ext == ".tcx":
                        rec = tcx(path)
                    elif ext == ".gpx":
                        rec = gpx(path)
                    elif ext == ".kml":
                        rec = kml(path)
                    else:
                        continue
                except Exception as e:  # noqa: BLE001 - not parseable: noted
                    rec = {"kind": ext[1:], "error": str(e)[:200]}
                rec["file"] = os.path.relpath(path, root)
                out.write(json.dumps(rec) + "\n")
                n += 1
    print(f"  tracks oracle: {n} files")


if __name__ == "__main__":
    main()
