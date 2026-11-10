# Develop: tone zones, batch conflicts, RAW+JPEG pairs, CMYK sources.
# machine-drafted, needs native review

develop-tone-zones = Tonzoner
develop-tone-zone = { $ev } EV
develop-tone-zones-adjust = Justera zoner i bilden
develop-tone-zones-done = Klar med zoner
develop-tone-zones-adjusting = Dra uppåt eller nedåt i bilden för att höja eller sänka den tonen. Esc avslutar.
develop-tone-zone-picked = Zon { $zone }
develop-batch-conflict-title = { $count ->
    [one] Ett foto ändrades utanför Specular
   *[other] { $count } foton ändrades utanför Specular
}
develop-batch-conflict-detail = Deras sidofiler är nyare än bibliotekets kopior.
develop-batch-conflict-keep = Behåll: inklistringen läggs ovanpå filens ändringar.
develop-batch-conflict-replace = Ersätt: inklistringen läggs på bibliotekets kopia. Filens ändringar försvinner.
develop-batch-conflict-skip-detail = Hoppa över: de fotona förblir som de är.
develop-batch-conflict-file = Behåll filens ändringar
develop-batch-conflict-library = Ersätt dem
develop-batch-conflict-skip = Hoppa över de fotona
develop-pair-raw = Framkallar { $file }, råfilen i paret
develop-source-cmyk = Det här CMYK-fotot redigeras i RGB. Det konverterades med sin profil.
