# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Převést…

## Settings → Convert
settings-section-convert = Převod
settings-convert-format = Formát
settings-convert-format-help = Formát, do kterého zapisuje příkaz „Převést…“. PNG a TIFF jsou bezeztrátové; WebP a JPEG XL mohou být obojí.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = Kvalita JPEG
settings-convert-jpeg-quality-help = Vyšší hodnota zachová víc detailů ve větších souborech. Při 90 je těžké rozeznat rozdíl od originálu.
settings-convert-jpeg-full-chroma = Barva v plném rozlišení v JPEG
settings-convert-jpeg-full-chroma-help = Zachová barvu v plném rozlišení (4:4:4) místo polovičního (4:2:0): ostřejší barevné hrany, větší soubory.
settings-convert-png-effort = Komprese PNG
settings-convert-png-effort-help = Vyšší hodnota vytváří menší soubory, ale pomaleji. 1 je nejrychlejší.
settings-convert-jxl-effort = Úsilí JPEG XL
settings-convert-jxl-effort-help = Vyšší hodnota vytváří menší soubory, ale pomaleji.
settings-convert-tiff-compression = Komprese TIFF
settings-convert-tiff-compression-help = Deflate je bezeztrátová a přečte ji každý program pro TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Žádná
settings-convert-depth = Bitová hloubka
settings-convert-depth-help = Automaticky zapíše 16 bitů na vzorek, když má obrázek víc než 8 a formát je pojme (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Automaticky
settings-convert-depth-8 = 8 bitů
settings-convert-depth-16 = 16 bitů
settings-convert-resize = Změna velikosti
settings-convert-resize-help = Obrázky se zmenšují v lineárním světle kvalitním filtrem.
settings-convert-resize-none = Původní velikost
settings-convert-resize-fit = Vměstnat do
settings-convert-resize-long-edge = Delší strana
settings-convert-resize-percent = Procenta
settings-convert-width = Šířka
settings-convert-width-help = Největší šířka, jakou obrázek může mít při volbě Vměstnat do.
settings-convert-height = Výška
settings-convert-height-help = Největší výška, jakou obrázek může mít při volbě Vměstnat do.
settings-convert-long-edge = Delší strana
settings-convert-long-edge-help = Délka delší strany obrázku při volbě Delší strana.
settings-convert-percent = Procenta
settings-convert-percent-help = Velikost vzhledem k originálu při volbě Procenta.
settings-convert-upscale = Zvětšovat malé obrázky
settings-convert-upscale-help = Když je vypnuto, obrázky menší než požadovaná velikost si svou velikost ponechají.
settings-convert-colour = Barvy
settings-convert-colour-help = Ponechat uloží s obrázkem jeho barevný profil. Převést do sRGB se hodí pro web a zařízení bez správy barev.
settings-convert-colour-keep = Ponechat profil
settings-convert-colour-srgb = Převést do sRGB
settings-convert-hdr = Obrázky HDR
settings-convert-hdr-help = PNG a JPEG XL zachovají HDR tak, jak je; ostatní formáty se vždy tónově mapují do SDR.
settings-convert-hdr-keep = Ponechat, kde to jde
settings-convert-hdr-tone-map = Tónově mapovat do SDR
settings-convert-metadata = Metadata
settings-convert-metadata-help = Exif a XMP, například fotoaparát, datum pořízení a poloha. Barevný profil se ponechá vždy.
settings-convert-metadata-all = Ponechat vše
settings-convert-metadata-no-location = Vše kromě polohy
settings-convert-metadata-strip = Odebrat
settings-convert-folder = Cílová složka
settings-convert-folder-help = Kam se převedené soubory uloží. Když není nastavena, uloží se do podsložky vedle každého obrázku.
settings-convert-folder-unset = Podsložka vedle obrázků
settings-convert-subfolder = Název podsložky
settings-convert-subfolder-help = Podsložka použitá, když není nastavena cílová složka. Prázdné: vlastní složka obrázků.
settings-convert-pattern = Názvy souborů
settings-convert-pattern-help = {"{"}name{"}"} je původní název, {"{"}n:03{"}"} číslo v dávce, {"{"}date:%Y-%m-%d{"}"} datum pořízení, {"{"}w{"}"} a {"{"}h{"}"} nová velikost, {"{"}ext{"}"} přípona formátu.
settings-convert-conflicts = Když je název obsazen
settings-convert-conflicts-help = Existující soubory se nikdy nenahrazují.
settings-convert-conflicts-suffix = Přidat číslo
settings-convert-conflicts-skip = Přeskočit obrázek

## The dialog
convert-title = Převést
convert-images = Obrázky
convert-images-current = Tento obrázek
convert-images-selection = Výběr ({ $count })
convert-images-folder = Složka ({ $count })
convert-images-picked = Vybrané ({ $count })
convert-quality = Kvalita
convert-chroma = Barevné detaily
convert-chroma-half = 4:2:0 (menší)
convert-chroma-full = 4:4:4 (ostřejší)
convert-effort = Úsilí
convert-on = Zapnuto
convert-off = Vypnuto
convert-pixels = { $value } px
convert-effort-fastest = { $value } (nejrychlejší)
convert-folder-subfolder = Podsložka „{ $name }“
convert-folder-same = Stejná složka jako obrázek
convert-choose-folder = Zvolit složku…
convert-name = Název
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Závorka {"{"} není uzavřena.
convert-name-error-stray = Závorka {"}"} nemá svou {"{"}. Pro složenou závorku napište {"}}"}.
convert-name-error-unknown = { $token } není část názvu. Použijte name, n, date, w, h nebo ext.
convert-name-error-format = { $token } má chybný formát.
convert-name-error-empty = Zadejte vzor názvu.
convert-start = Převést
convert-stop = Zastavit
convert-hide = Skrýt
convert-close = Zavřít
convert-progress = Převádí se { $done } z { $total }
convert-progress-eta = Převádí se { $done } z { $total } · zbývá asi { $time }
convert-eta-seconds = { $count ->
    [one] { $count } sekunda
    [few] { $count } sekundy
    [many] { $count } sekundy
   *[other] { $count } sekund
}
convert-eta-minutes = { $count ->
    [one] { $count } minuta
    [few] { $count } minuty
    [many] { $count } minuty
   *[other] { $count } minut
}
convert-done = { $count ->
    [one] Převeden { $count } obrázek
    [few] Převedeny { $count } obrázky
    [many] Převedeno { $count } obrázku
   *[other] Převedeno { $count } obrázků
}
convert-done-skipped = Přeskočeno: { $count } (název byl obsazen)
convert-done-failed = Selhalo: { $count }
convert-stopped = Zastaveno po { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = nelze otevřít ({ $error })
convert-failure-decode = nelze přečíst ({ $error })
convert-failure-encode = nelze zapsat v tomto formátu ({ $error })
convert-failure-write = nelze uložit ({ $error })
convert-failure-same = nový soubor by nahradil originál
convert-report-more = a { $count } dalších
convert-running = Převod už probíhá.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = Bezeztrátový WebP
settings-convert-webp-lossless-help = Zachová přesně každý pixel. Když je vypnuto, jsou soubory WebP při zvolené kvalitě WebP mnohem menší.
settings-convert-webp-quality = Kvalita WebP
settings-convert-webp-quality-help = Vyšší hodnota zachová víc detailů ve větších souborech. Použije se, když WebP není bezeztrátový.
settings-convert-webp-effort = Úsilí WebP
settings-convert-webp-effort-help = Vyšší hodnota vytváří menší soubory, ale pomaleji. Použije se, když WebP není bezeztrátový.
settings-convert-jxl-lossless = Bezeztrátový JPEG XL
settings-convert-jxl-lossless-help = Zachová přesně každý pixel. Když je vypnuto, jsou soubory JPEG XL při zvolené kvalitě JPEG XL mnohem menší.
settings-convert-jxl-quality = Kvalita JPEG XL
settings-convert-jxl-quality-help = Vyšší hodnota zachová víc detailů ve větších souborech. Při 90 je těžké rozeznat rozdíl od originálu. Použije se, když JPEG XL není bezeztrátový.
settings-convert-avif-quality = Kvalita AVIF
settings-convert-avif-quality-help = Vyšší hodnota zachová víc detailů ve větších souborech.
settings-convert-avif-effort = Úsilí AVIF
settings-convert-avif-effort-help = Vyšší hodnota vytváří menší soubory, ale pomaleji. 1 je nejrychlejší.
convert-lossless = Bezeztrátově
