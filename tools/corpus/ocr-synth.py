#!/usr/bin/env python3
"""A synthetic text-in-photo set for the OCR oracle (tools/corpus/ocr-oracle.py).

Usage: python -I tools/corpus/ocr-synth.py KODAK_DIR OUT_DIR [COUNT]

Draws lines of text in eleven scripts (Latin, Chinese, Japanese, Korean, Cyrillic, Greek,
Arabic, Hebrew, Thai, Devanagari, plus English) onto the Kodak photos (enlarged to
1600×1067), at random sizes, tilts, colours and on plain or no plates, as signs and
captions look. Fonts come from the system (fontconfig, Noto where present); Pillow with
libraqm shapes and orders the text (Arabic joining, right-to-left lines).

Writes OUT_DIR/<lang>/NNN.png and OUT_DIR/truth.tsv: image (lang/NNN.png), the line's
tight quad (8 numbers, clockwise from its top left), 1, the line's text (logical order).
Also OUT_DIR/big.png: one 4000×3000 (12 MP) photo with lines, for timing.
The phrases are our own; the photos are the Kodak set (free for any use).
"""

import math
import os
import random
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont, features

PHRASES = {
    "en": ["Open daily 9 to 5", "Main Street", "No parking on weekdays", "Fresh bread and coffee",
           "Platform 3", "Exit", "Welcome to the museum", "Sale 50% off", "Bus stop", "Library"],
    "latin": ["Fermé le dimanche", "Bäckerei Müller", "Café de la gare", "Ausgang", "Plaza Mayor",
              "Peligro", "Stazione centrale", "Łódź Główna", "Åbo hamn", "Rua das Flores"],
    "zh": ["欢迎光临", "出口", "北京东路", "营业时间", "禁止吸烟", "火车站", "图书馆", "新鲜水果"],
    "ja": ["いらっしゃいませ", "東京駅", "出口はこちら", "本日休業", "営業中", "禁煙"],
    "ko": ["어서 오세요", "서울역", "출구", "영업 중", "금연", "도서관"],
    "ru": ["Добро пожаловать", "Выход", "Улица Ленина", "Открыто", "Аптека", "Библиотека"],
    "el": ["Καλώς ήρθατε", "Έξοδος", "Οδός Ερμού", "Φαρμακείο", "Ανοιχτό", "Βιβλιοθήκη"],
    "ar": ["أهلا وسهلا", "مخرج", "شارع النيل", "مفتوح", "صيدلية", "مكتبة عامة"],
    "he": ["ברוכים הבאים", "יציאה", "רחוב הרצל", "פתוח", "בית מרקחת", "ספרייה"],
    "th": ["ยินดีต้อนรับ", "ทางออก", "ถนนสุขุมวิท", "เปิด", "ร้านขายยา", "ห้องสมุด"],
    "hi": ["स्वागत है", "निकास", "मुख्य सड़क", "खुला है", "दवाखाना", "पुस्तकालय"],
}

FONTS = {
    "en": ["Noto Sans", "DejaVu Sans", "Liberation Serif", "Noto Serif"],
    "latin": ["Noto Sans", "DejaVu Sans", "Liberation Serif"],
    "zh": ["Noto Sans CJK SC", "Noto Serif CJK SC"],
    "ja": ["Noto Sans CJK JP", "Noto Serif CJK JP"],
    "ko": ["Noto Sans CJK KR", "NanumSquare", "UnDotum"],
    "ru": ["Noto Sans", "DejaVu Sans", "Liberation Serif"],
    "el": ["Noto Sans", "DejaVu Sans", "Liberation Serif"],
    "ar": ["Noto Sans Arabic", "Noto Naskh Arabic"],
    "he": ["Noto Sans Hebrew", "DejaVu Sans"],
    "th": ["Noto Sans Thai", "Noto Serif Thai"],
    "hi": ["Noto Sans Devanagari", "Noto Serif Devanagari"],
}

W, H = 1600, 1067


def font_file(family, lang):
    try:
        out = subprocess.run(["fc-match", "-f", "%{file}", f"{family}:lang={lang}"],
                             capture_output=True, text=True, check=True).stdout
        return out or None
    except (OSError, subprocess.CalledProcessError):
        return None


def contrast_colours(rng):
    if rng.random() < 0.5:
        return (rng.randint(0, 60),) * 3, (rng.randint(215, 255),) * 3
    return (rng.randint(215, 255),) * 3, (rng.randint(0, 60),) * 3


def draw_line(img, text, font, lang, rng, taken):
    """Draws one line; returns its quad or None when it does not fit free space."""
    direction = "rtl" if lang in ("ar", "he") else "ltr"
    probe = ImageDraw.Draw(Image.new("L", (1, 1)))
    l, t, r, b = probe.textbbox((0, 0), text, font=font, direction=direction, language=lang)
    tw, th = r - l, b - t
    pad = max(4, th // 3)
    canvas = Image.new("L", (tw + 2 * pad, th + 2 * pad), 0)
    ImageDraw.Draw(canvas).text((pad - l, pad - t), text, fill=255, font=font,
                                direction=direction, language=lang)
    angle = rng.uniform(-12, 12) if rng.random() < 0.6 else 0.0
    fg, bg = contrast_colours(rng)
    plate = rng.random() < 0.6
    for _ in range(40):
        cx = rng.uniform(0.1 * W, 0.9 * W)
        cy = rng.uniform(0.08 * H, 0.92 * H)
        a = math.radians(angle)
        ca, sa = math.cos(a), math.sin(a)
        hw, hh = canvas.width / 2, canvas.height / 2

        def rot(x, y):
            return cx + x * ca - y * sa, cy + x * sa + y * ca

        outer = [rot(-hw, -hh), rot(hw, -hh), rot(hw, hh), rot(-hw, hh)]
        xs, ys = [p[0] for p in outer], [p[1] for p in outer]
        box = (min(xs), min(ys), max(xs), max(ys))
        if box[0] < 0 or box[1] < 0 or box[2] >= W or box[3] >= H:
            continue
        if any(not (box[2] < o[0] - 8 or box[0] > o[2] + 8 or box[3] < o[1] - 8 or box[1] > o[3] + 8)
               for o in taken):
            continue
        taken.append(box)
        rotated = canvas.rotate(-angle, resample=Image.BICUBIC, expand=True)
        ox, oy = int(round(cx - rotated.width / 2)), int(round(cy - rotated.height / 2))
        if plate:
            plate_img = Image.new("L", canvas.size, 255).rotate(-angle, resample=Image.BICUBIC, expand=True)
            img.paste(Image.new("RGB", rotated.size, bg), (ox, oy), plate_img)
        img.paste(Image.new("RGB", rotated.size, fg), (ox, oy), rotated)
        # The tight quad: the ink box (canvas minus padding) rotated.
        ihw, ihh = tw / 2, th / 2
        q = [rot(-ihw, -ihh), rot(ihw, -ihh), rot(ihw, ihh), rot(-ihw, ihh)]
        return q
    return None


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    if not features.check("raqm"):
        sys.exit("Pillow without libraqm: cannot shape Arabic or order right-to-left lines")
    kodak, out = sys.argv[1], sys.argv[2]
    count = int(sys.argv[3]) if len(sys.argv) > 3 else 6
    photos = sorted(os.path.join(kodak, f) for f in os.listdir(kodak) if f.endswith(".png"))
    rng = random.Random(20261031)
    os.makedirs(out, exist_ok=True)
    rows = []
    for li, (lang, phrases) in enumerate(PHRASES.items()):
        files = [f for f in (font_file(fam, lang.split("-")[0] if lang not in ("en", "latin") else "en")
                             for fam in FONTS[lang]) if f]
        os.makedirs(os.path.join(out, lang), exist_ok=True)
        for n in range(count):
            img = Image.open(photos[(li * count + n) % len(photos)]).convert("RGB").resize((W, H), Image.BICUBIC)
            taken = []
            for _ in range(rng.randint(3, 6)):
                text = rng.choice(phrases)
                size = rng.randint(24, 72)
                font = ImageFont.truetype(rng.choice(files), size, layout_engine=ImageFont.Layout.RAQM)
                q = draw_line(img, text, font, lang, rng, taken)
                if q:
                    rows.append((f"{lang}/{n:03}.png", q, text))
            img.save(os.path.join(out, lang, f"{n:03}.png"))
    with open(os.path.join(out, "truth.tsv"), "w", encoding="utf-8") as f:
        for name, q, text in rows:
            f.write(name + "\t" + "\t".join(f"{v:.1f}" for p in q for v in p) + f"\t1\t{text}\n")
    # One 12 MP photo for timing.
    big = Image.open(photos[0]).convert("RGB").resize((4000, 3000), Image.BICUBIC)
    d = ImageDraw.Draw(big)
    font = ImageFont.truetype(font_file("Noto Sans", "en"), 90)
    for i, text in enumerate(PHRASES["en"]):
        d.text((200 + (i % 2) * 1800, 200 + i * 260), text, fill=(10, 10, 10), font=font)
    big.save(os.path.join(out, "big.png"))
    print(len(rows), "lines")


if __name__ == "__main__":
    main()
