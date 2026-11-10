# Develop: tone zones, batch conflicts, RAW+JPEG pairs, CMYK sources.
# machine-drafted, needs native review

develop-tone-zones = Strefy tonalne
develop-tone-zone = { $ev } EV
develop-tone-zones-adjust = Dostosuj strefy na obrazie
develop-tone-zones-done = Zakończ dostosowywanie stref
develop-tone-zones-adjusting = Przeciągnij w górę lub w dół na obrazie, aby rozjaśnić lub przyciemnić ten ton. Esc kończy.
develop-tone-zone-picked = Strefa { $zone }
develop-batch-conflict-title = { $count ->
    [one] Jedno zdjęcie zmieniono poza Specular
    [few] { $count } zdjęcia zmieniono poza Specular
    [many] { $count } zdjęć zmieniono poza Specular
   *[other] { $count } zdjęcia zmieniono poza Specular
}
develop-batch-conflict-detail = Ich pliki pomocnicze są nowsze niż kopie w bibliotece.
develop-batch-conflict-keep = Zachowaj: wklejenie trafia na zmiany z pliku.
develop-batch-conflict-replace = Zastąp: wklejenie trafia na kopię z biblioteki. Zmiany z pliku przepadają.
develop-batch-conflict-skip-detail = Pomiń: te zdjęcia zostają bez zmian.
develop-batch-conflict-file = Zachowaj zmiany z pliku
develop-batch-conflict-library = Zastąp je
develop-batch-conflict-skip = Pomiń te zdjęcia
develop-pair-raw = Wywoływanie { $file }, pliku RAW tej pary
develop-source-cmyk = To zdjęcie CMYK jest edytowane w RGB. Przekształcono je za pomocą jego profilu.
