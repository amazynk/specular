# machine-drafted, needs native review
# Specular — Develop: historik, ögonblicksbilder, stilar, förinställningar, versioner, kopiera och klistra in, matcha. Svenska.

palette-develop-copy = Kopiera redigering
palette-develop-copy-help = Kopiera fotots redigering, utom beskärning och objektiv, för att klistra in på andra foton.
palette-develop-copy-some = Kopiera vissa moduler…
palette-develop-copy-some-help = Välj vilka moduler i redigeringen som kopieras.
palette-develop-paste = Klistra in redigering
palette-develop-paste-help = Klistra in den kopierade redigeringen på fotot, eller på fotona som är markerade i galleriet.
palette-develop-paste-some = Klistra in vissa moduler…
palette-develop-paste-some-help = Välj vilka kopierade moduler som klistras in, som tillägg eller ersättning.
palette-develop-paste-picks = Klistra in på valda
palette-develop-paste-picks-help = Klistra in den kopierade redigeringen på varje valt foto.
palette-develop-reference = Använd som referens
palette-develop-reference-help = Andra foton kan sedan matcha detta fotos exponering och vitbalans.
palette-develop-match = Matcha mot referens
palette-develop-match-help = Ställ in exponering och vitbalans för fotot, markeringen eller de valda efter referensen.
palette-develop-sync = Synka till valda
palette-develop-sync-help = Lägg detta fotos redigering, utom beskärning och objektiv, på de valda fotona.
palette-develop-sync-some = Synka vissa moduler…
palette-develop-sync-some-help = Välj vilka moduler från detta foto som går till de valda fotona.
palette-develop-next-capture = Stil för nästa bild…
palette-develop-next-capture-help = Stilen som varje ny fil i mappen får.
palette-develop-auto-rules = Automatiska regler…
palette-develop-auto-rules-help = Reglerna som lägger på en stil första gången ett foto öppnas i Develop.
palette-develop-import-style = Importera stil…
palette-develop-import-style-help = Lägg till en stil från en fil.
palette-develop-snapshot = Ta ögonblicksbild
palette-develop-snapshot-help = Namnge redigeringen som den är nu, för att jämföra med eller gå tillbaka till senare.
palette-develop-compress = Komprimera historik
palette-develop-compress-help = Ta bort steg som senare steg ersatt. Redigeringen blir densamma.
palette-develop-new-version = Ny version
palette-develop-new-version-help = En ny redigering av samma foto, med egen historik. Filen kopieras inte.
palette-develop-new-style = Ny stil…
palette-develop-new-style-help = Spara valda moduler i redigeringen som en stil.

develop-styles = Stilar
develop-snapshots = Historik och ögonblicksbilder
develop-versions = Versioner
develop-style-new = Ny stil från denna redigering…
develop-style-import = Importera stil…
develop-rules = Automatiska regler…
develop-snapshot-take = Ta ögonblicksbild
develop-history-compress = Komprimera historik
develop-snapshot-comparing = Jämför: { $name }
develop-snapshot-compare-off = Jämförelse av
develop-version-number = Version { $number }
develop-version-open = { $name } (öppen)
develop-version-new = Ny version
develop-version-duplicate = Duplicera denna version

develop-history-paste = Klistra in
develop-history-sync = Synka
develop-history-match = Matcha mot referens
develop-history-snapshot = Ögonblicksbild: { $name }
develop-history-previous = Som föregående

develop-rename = Byt namn…
develop-delete = Ta bort
develop-save = Spara
develop-name = Namn
develop-style-apply = Använd
develop-style-apply-replace = Använd och ersätt dessa moduler
develop-style-auto = Använd på nya foton från denna kamera
develop-style-export = Exportera…
develop-snapshot-restore = Gå tillbaka till denna ögonblicksbild
develop-version-open-it = Öppna
develop-preset-save = Spara som förinställning…
develop-preset-save-module = Spara { $module } som förinställning…
develop-preset-delete = Ta bort ”{ $name }”…

develop-sheet-modules = Moduler
develop-sheet-included = Med
develop-sheet-left-out = Utelämnad
develop-sheet-more = { $count } till
develop-copy-title = Kopiera moduler
develop-copy-ok = Kopiera
develop-paste-title = Klistra in moduler
develop-paste-ok = Klistra in
develop-paste-mode = Moduler som redan finns i redigeringen
develop-paste-append = Lägg till
develop-paste-replace = Ersätt
develop-style-new-title = Ny stil
develop-style-save = Spara stil
develop-style-default-name = Min stil { $number }
develop-style-rename-title = Byt namn på stil
develop-preset-save-title = Förinställning för { $module }
develop-preset-default-name = Min förinställning
develop-snapshot-take-title = Ta ögonblicksbild
develop-snapshot-default-name = Ögonblicksbild { $number }
develop-snapshot-rename-title = Byt namn på ögonblicksbild
develop-version-new-title = Ny version
develop-version-duplicate-title = Duplicera version
develop-version-create = Skapa
develop-version-rename-title = Byt namn på version

develop-style-delete-ask = Ta bort stilen ”{ $name }”?
develop-style-delete-detail = Foton den använts på behåller sin redigering. Stilfilen tas bort.
develop-preset-delete-ask = Ta bort förinställningen ”{ $name }”?
develop-preset-delete-detail = Redigeringar den använts på förblir som de är.
develop-snapshot-delete-ask = Ta bort ögonblicksbilden ”{ $name }”?
develop-snapshot-delete-detail = Redigeringen och dess historik förblir som de är.
develop-version-delete-ask = Ta bort ”{ $name }”?
develop-version-delete-detail = Denna versions redigering och historik tas bort. Fotot och dess andra versioner finns kvar på disken.
develop-rule-delete-ask = Ta bort regeln ”{ $rule }”?
develop-rule-delete-detail = Stilen finns kvar, och foton som redan öppnats behåller sin redigering.

develop-rule-text = { $style }: { $when }
develop-rule-every-photo = varje foto
develop-rule-iso-min = ISO { $iso } och uppåt
develop-rule-iso-max = ISO upp till { $iso }
develop-rule-iso-range = ISO { $low }–{ $high }
develop-rule-raw = raw-filer
develop-rule-other = inte raw
develop-rule-any-camera = vilken kamera som helst
develop-next-placeholder = Stil för varje ny fil
develop-next-off-row = Ingen
develop-next-previous-row = Som föregående foto

develop-copied = { $count ->
    [one] 1 modul kopierad från { $name }
   *[other] { $count } moduler kopierade från { $name }
}
develop-pasted = { $count ->
    [one] Inklistrat på 1 foto
   *[other] Inklistrat på { $count } foton
}
develop-nothing-to-copy = Inget foto att kopiera från.
develop-nothing-copied = Inget kopierat än. Kopiera en redigering först (Ctrl+Alt+C).
develop-no-photos = Inga foton att ändra.
develop-no-picks = Inga valda foton.
develop-reference-set = Referens: { $name }
develop-no-reference = Ingen referens än. Öppna referensfotot och välj Använd som referens (Ctrl+Alt+R).
develop-reference-unreadable = Referensfotot kunde inte läsas.
develop-matching = { $count ->
    [one] Matchar 1 foto…
   *[other] Matchar { $count } foton…
}
develop-matched = { $count ->
    [one] 1 foto matchat
   *[other] { $count } foton matchade
}
develop-matched-some = { $count } matchade. { $failed } kunde inte läsas eller mätas.
develop-style-saved = Stil sparad: { $name }
develop-style-applied = Stil använd: { $name }
develop-style-imported = Stil importerad: { $name }
develop-style-not-a-style = Den här filen är ingen stil.
develop-style-exported = Stil exporterad: { $name }
develop-style-export-failed = Stilen kunde inte skrivas där.
develop-style-import-dialog = Importera en stil
develop-style-export-dialog = Exportera stilen
develop-style-file-kind = Stil
develop-preset-saved = Förinställning sparad: { $name }
develop-preset-applied = Förinställning använd: { $name }
develop-snapshot-taken = Ögonblicksbild tagen: { $name }
develop-history-compressed = { $count ->
    [0] Inget att komprimera
    [one] 1 steg borttaget
   *[other] { $count } steg borttagna
}
develop-version-created = Version skapad: { $name }
develop-version-opened = Version öppen: { $name }
develop-version-deleted = Version borttagen: { $name }
develop-rule-added = { $name } gäller nya foton från { $camera }.
develop-rule-applied = Automatisk regel: { $name }
develop-rules-none = Inga automatiska regler. Lägg till en från en stils meny.
develop-next-off = Nya filer får ingen stil.
develop-next-previous = Nya filer får föregående fotos redigering.
develop-next-style = Nya filer får stilen { $name }.
develop-next-applied = { $count ->
    [one] Stil för nästa bild på 1 ny fil
   *[other] Stil för nästa bild på { $count } nya filer
}

develop-style-specular = Specular kraftig
develop-style-soft = Mjuk
develop-style-vivid = Livfull
develop-style-mono = Svartvit
develop-preset-plus-one = Ljusare (+1 EV)
develop-preset-minus-one = Mörkare (−1 EV)
develop-preset-low-contrast = Låg kontrast
develop-preset-high-contrast = Hög kontrast
develop-preset-open-shadows = Lätta upp skuggor
develop-preset-crisp = Skarp
develop-preset-muted = Dämpad
develop-preset-high-iso = Högt ISO
develop-preset-screen = För skärmar

gallery-badge-versions = { $count } versioner
# machine-drafted, needs native review
develop-batch-row = Sparar redigeringar
# machine-drafted, needs native review
develop-batch-row-detail = { $done } av { $total } foton
# machine-drafted, needs native review
develop-batch-saving = Sparar
# machine-drafted, needs native review
develop-batch-stopping = Stoppar
# machine-drafted, needs native review
develop-batch-stopped = Stoppat. { $done } av { $total } foton sparade; resten är oförändrade.
