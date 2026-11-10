#!/usr/bin/env python3
"""Synthetic text lines, upright and upside down, for the OCR orientation check
(crates/specular-infer/tests/ocr.rs `real_orientation_turns_upside_down_lines`).

Usage: python -I tools/corpus/ocr-orient.py OUT_DIR [COUNT]

Renders COUNT lines (default 240) of short phrases in Latin, Cyrillic, Greek, Arabic, Chinese
and Korean script, each at a random size and colour on a plain or noisy light or dark plate
with a few pixels of margin, as the detector crops them. Writes OUT_DIR/NNN.ppm (upright),
OUT_DIR/NNN-r.ppm (the same turned 180°) and OUT_DIR/truth.tsv (name, script, text).
Fonts come from the system (fontconfig, Noto or DejaVu); Pillow with libraqm shapes Arabic.
The phrases are our own.
"""

import os
import random
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont

PHRASES = {
    "en": ["Open daily 9 to 5", "Main Street", "No parking on weekdays", "Fresh bread and coffee",
           "Platform 3", "Welcome to the museum", "Sale 50% off", "Bus stop 12", "Library",
           "Keep left", "Tickets and information", "Room 204"],
    "latin": ["Fermé le dimanche", "Bäckerei Müller", "Café de la gare", "Ausgang", "Plaza Mayor",
              "Stazione centrale", "Rua das Flores", "Peligro"],
    "ru": ["Добро пожаловать", "Выход", "Улица Ленина", "Открыто", "Аптека", "Библиотека"],
    "el": ["Καλώς ήρθατε", "Έξοδος", "Οδός Ερμού", "Φαρμακείο", "Βιβλιοθήκη"],
    "ar": ["أهلا وسهلا", "مخرج", "شارع النيل", "مفتوح", "صيدلية", "مكتبة عامة"],
    "zh": ["欢迎光临", "北京东路", "营业时间", "禁止吸烟", "火车站", "新鲜水果"],
    "ko": ["어서 오세요", "서울역", "영업 중", "도서관", "금연 구역"],
}

FONT_FOR = {
    "en": "DejaVu Sans", "latin": "DejaVu Serif", "ru": "DejaVu Sans", "el": "DejaVu Sans",
    "ar": "Noto Sans Arabic", "zh": "Noto Sans CJK SC", "ko": "Noto Sans CJK KR",
}


def font_file(family, lang):
    try:
        out = subprocess.run(["fc-match", "-f", "%{file}", f"{family}:lang={lang}"],
                             capture_output=True, text=True, check=True).stdout.strip()
        return out or None
    except (OSError, subprocess.CalledProcessError):
        return None


def write_ppm(img, path):
    img = img.convert("RGB")
    with open(path, "wb") as f:
        f.write(b"P6\n%d %d\n255\n" % img.size)
        f.write(img.tobytes())


def main():
    out = sys.argv[1]
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 240
    os.makedirs(out, exist_ok=True)
    rng = random.Random(20261101)
    langs = sorted(PHRASES)
    fonts = {}
    for lang in langs:
        f = font_file(FONT_FOR[lang], "ar" if lang == "ar" else lang[:2])
        if f:
            fonts[lang] = f
    with open(os.path.join(out, "truth.tsv"), "w", encoding="utf-8") as truth:
        for i in range(count):
            lang = langs[i % len(langs)]
            if lang not in fonts:
                continue
            text = rng.choice(PHRASES[lang])
            size = rng.randint(18, 56)
            font = ImageFont.truetype(fonts[lang], size)
            x0, y0, x1, y1 = font.getbbox(text)
            m = rng.randint(2, max(3, size // 5))
            w, h = x1 - x0 + 2 * m, y1 - y0 + 2 * m
            dark = rng.random() < 0.25
            bg = tuple(rng.randint(0, 60) if dark else rng.randint(190, 255) for _ in range(3))
            fg = tuple(rng.randint(200, 255) if dark else rng.randint(0, 80) for _ in range(3))
            img = Image.new("RGB", (w, h), bg)
            ImageDraw.Draw(img).text((m - x0, m - y0), text, font=font, fill=fg)
            if rng.random() < 0.5:
                px = img.load()
                for _ in range(w * h // 8):
                    x, y = rng.randrange(w), rng.randrange(h)
                    p = px[x, y]
                    d = rng.randint(-25, 25)
                    px[x, y] = tuple(max(0, min(255, c + d)) for c in p)
            name = f"{i:03d}"
            write_ppm(img, os.path.join(out, name + ".ppm"))
            write_ppm(img.rotate(180), os.path.join(out, name + "-r.ppm"))
            truth.write(f"{name}\t{lang}\t{text}\n")


if __name__ == "__main__":
    main()
