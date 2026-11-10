#!/usr/bin/env python3
"""Rendered text lines for the line-recogniser oracle (tools/corpus/ocr-oracle.py `tess`).

Usage: python -I tools/corpus/ocr-lines.py LANG OUT_DIR [COUNT]

LANG is he (Hebrew) or ar (Arabic). Writes OUT_DIR/NNN.png (for Tesseract), NNN.pgm (for
`cargo run --example ocr -- tess`) and OUT_DIR/truth.tsv (name, text in logical order).
Lines are our own phrases in system fonts (fontconfig), black on white or light grey, with a
margin of about a third of the text height (as the detector's boxes give), some slightly
blurred. Pillow with libraqm orders the right-to-left text.
"""

import os
import random
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont

WORDS = {
    "he": ("שלום עולם ברוכים הבאים לעיר יציאה רחוב הרצל פתוח היום בית מרקחת ספרייה ציבורית "
           "תחנה מרכזית קפה ומאפה סגור בשבת מכירה כל הפריטים חדש בית ספר גן ילדים חניה "
           "אסורה בימי חול מוזיאון ישראל ירושלים תל אביב חיפה באר שבע אוניברסיטה ספר "
           "מחברת עיתון בוקר טוב לילה טוב תודה רבה בבקשה").split(),
    "ar": ("أهلا وسهلا مخرج شارع النيل مفتوح صيدلية مكتبة عامة محطة القطار مقهى مغلق يوم "
           "الجمعة تخفيضات جديد مدرسة حديقة موقف سيارات متحف القاهرة بيروت عمان جامعة كتاب "
           "صباح الخير شكرا من فضلك").split(),
}
FONTS = {"he": ["Noto Sans Hebrew", "DejaVu Sans", "Noto Serif Hebrew"],
         "ar": ["Noto Sans Arabic", "Noto Naskh Arabic"]}


def font_file(family, lang):
    out = subprocess.run(["fc-match", "-f", "%{file}", f"{family}:lang={lang}"],
                         capture_output=True, text=True, check=True).stdout
    return out


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    lang, out = sys.argv[1], sys.argv[2]
    count = int(sys.argv[3]) if len(sys.argv) > 3 else 60
    rng = random.Random(36)
    os.makedirs(out, exist_ok=True)
    files = sorted({font_file(f, lang) for f in FONTS[lang]})
    with open(os.path.join(out, "truth.tsv"), "w", encoding="utf-8") as tsv:
        for n in range(count):
            words = [rng.choice(WORDS[lang]) for _ in range(rng.randint(1, 4))]
            if rng.random() < 0.2:
                words.insert(rng.randint(0, len(words)), str(rng.randint(1, 2026)))
            text = " ".join(words)
            size = rng.randint(22, 56)
            font = ImageFont.truetype(rng.choice(files), size, layout_engine=ImageFont.Layout.RAQM)
            probe = ImageDraw.Draw(Image.new("L", (1, 1)))
            l, t, r, b = probe.textbbox((0, 0), text, font=font, direction="rtl", language=lang)
            pad = max(3, (b - t) // 3)
            bg = rng.choice([255, 255, 235])
            img = Image.new("L", (r - l + 2 * pad, b - t + 2 * pad), bg)
            ImageDraw.Draw(img).text((pad - l, pad - t), text, fill=rng.randint(0, 50), font=font,
                                     direction="rtl", language=lang)
            if rng.random() < 0.3:
                img = img.filter(ImageFilter.GaussianBlur(0.8))
            name = f"{n:03}"
            img.save(os.path.join(out, name + ".png"))
            with open(os.path.join(out, name + ".pgm"), "wb") as f:
                f.write(f"P5\n{img.width} {img.height}\n255\n".encode() + img.tobytes())
            tsv.write(f"{name}\t{text}\n")
    print(count, "lines")


if __name__ == "__main__":
    main()
