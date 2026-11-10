# Develop: tone zones, batch conflicts, RAW+JPEG pairs, CMYK sources.
# machine-drafted, needs native review

develop-tone-zones = Tonezoner
develop-tone-zone = { $ev } EV
develop-tone-zones-adjust = Juster zoner på billedet
develop-tone-zones-done = Færdig med zoner
develop-tone-zones-adjusting = Træk op eller ned på billedet for at hæve eller sænke den tone. Esc afslutter.
develop-tone-zone-picked = Zone { $zone }
develop-batch-conflict-title = { $count ->
    [one] Ét foto blev ændret uden for Specular
   *[other] { $count } fotos blev ændret uden for Specular
}
develop-batch-conflict-detail = Deres sidecar-filer er nyere end bibliotekets kopier.
develop-batch-conflict-keep = Behold: indsætningen lægges oven på filens ændringer.
develop-batch-conflict-replace = Erstat: indsætningen lægges på bibliotekets kopi. Filens ændringer går tabt.
develop-batch-conflict-skip-detail = Spring over: de fotos forbliver som de er.
develop-batch-conflict-file = Behold filens ændringer
develop-batch-conflict-library = Erstat dem
develop-batch-conflict-skip = Spring de fotos over
develop-pair-raw = Fremkalder { $file }, RAW-filen i parret
develop-source-cmyk = Dette CMYK-foto redigeres i RGB. Det blev konverteret med sin profil.
