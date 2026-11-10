# machine-drafted, needs native review
# Specular — Develop: historik, øjebliksbilleder, stilarter, forudindstillinger, versioner, kopiér og indsæt, match. Dansk.

palette-develop-copy = Kopiér redigering
palette-develop-copy-help = Kopiér fotoets redigering, undtagen beskæring og objektiv, for at indsætte den på andre fotos.
palette-develop-copy-some = Kopiér nogle moduler…
palette-develop-copy-some-help = Vælg hvilke moduler i redigeringen der kopieres.
palette-develop-paste = Indsæt redigering
palette-develop-paste-help = Indsæt den kopierede redigering på fotoet eller på de fotos, der er markeret i galleriet.
palette-develop-paste-some = Indsæt nogle moduler…
palette-develop-paste-some-help = Vælg hvilke kopierede moduler der indsættes, som tilføjelse eller erstatning.
palette-develop-paste-picks = Indsæt på valgte
palette-develop-paste-picks-help = Indsæt den kopierede redigering på hvert valgt foto.
palette-develop-reference = Brug som reference
palette-develop-reference-help = Andre fotos kan så matche dette fotos eksponering og hvidbalance.
palette-develop-match = Match med reference
palette-develop-match-help = Indstil eksponering og hvidbalance for fotoet, markeringen eller de valgte efter referencen.
palette-develop-sync = Synkronisér til valgte
palette-develop-sync-help = Læg dette fotos redigering, undtagen beskæring og objektiv, på de valgte fotos.
palette-develop-sync-some = Synkronisér nogle moduler…
palette-develop-sync-some-help = Vælg hvilke moduler fra dette foto der går til de valgte fotos.
palette-develop-next-capture = Stil til næste billede…
palette-develop-next-capture-help = Den stil, hver ny fil i mappen får.
palette-develop-auto-rules = Automatiske regler…
palette-develop-auto-rules-help = Reglerne, der lægger en stil på, første gang et foto åbnes i Develop.
palette-develop-import-style = Importér stil…
palette-develop-import-style-help = Tilføj en stil fra en fil.
palette-develop-snapshot = Tag øjebliksbillede
palette-develop-snapshot-help = Navngiv redigeringen, som den er nu, for at sammenligne med eller gå tilbage til den senere.
palette-develop-compress = Komprimér historik
palette-develop-compress-help = Fjern trin, som senere trin har erstattet. Redigeringen forbliver den samme.
palette-develop-new-version = Ny version
palette-develop-new-version-help = En ny redigering af samme foto med egen historik. Filen kopieres ikke.
palette-develop-new-style = Ny stil…
palette-develop-new-style-help = Gem valgte moduler i redigeringen som en stil.

develop-styles = Stilarter
develop-snapshots = Historik og øjebliksbilleder
develop-versions = Versioner
develop-style-new = Ny stil fra denne redigering…
develop-style-import = Importér stil…
develop-rules = Automatiske regler…
develop-snapshot-take = Tag øjebliksbillede
develop-history-compress = Komprimér historik
develop-snapshot-comparing = Sammenligner: { $name }
develop-snapshot-compare-off = Sammenligning slået fra
develop-version-number = Version { $number }
develop-version-open = { $name } (åben)
develop-version-new = Ny version
develop-version-duplicate = Duplikér denne version

develop-history-paste = Indsæt
develop-history-sync = Synkronisér
develop-history-match = Match med reference
develop-history-snapshot = Øjebliksbillede: { $name }
develop-history-previous = Som forrige

develop-rename = Omdøb…
develop-delete = Slet
develop-save = Gem
develop-name = Navn
develop-style-apply = Anvend
develop-style-apply-replace = Anvend og erstat disse moduler
develop-style-auto = Anvend på nye fotos fra dette kamera
develop-style-export = Eksportér…
develop-snapshot-restore = Gå tilbage til dette øjebliksbillede
develop-version-open-it = Åbn
develop-preset-save = Gem som forudindstilling…
develop-preset-save-module = Gem { $module } som forudindstilling…
develop-preset-delete = Slet “{ $name }”…

develop-sheet-modules = Moduler
develop-sheet-included = Med
develop-sheet-left-out = Udeladt
develop-sheet-more = { $count } mere
develop-copy-title = Kopiér moduler
develop-copy-ok = Kopiér
develop-paste-title = Indsæt moduler
develop-paste-ok = Indsæt
develop-paste-mode = Moduler, der allerede er i redigeringen
develop-paste-append = Tilføj
develop-paste-replace = Erstat
develop-style-new-title = Ny stil
develop-style-save = Gem stil
develop-style-default-name = Min stil { $number }
develop-style-rename-title = Omdøb stil
develop-preset-save-title = Forudindstilling til { $module }
develop-preset-default-name = Min forudindstilling
develop-snapshot-take-title = Tag øjebliksbillede
develop-snapshot-default-name = Øjebliksbillede { $number }
develop-snapshot-rename-title = Omdøb øjebliksbillede
develop-version-new-title = Ny version
develop-version-duplicate-title = Duplikér version
develop-version-create = Opret
develop-version-rename-title = Omdøb version

develop-style-delete-ask = Slet stilen “{ $name }”?
develop-style-delete-detail = Fotos, den er anvendt på, beholder deres redigering. Stilfilen fjernes.
develop-preset-delete-ask = Slet forudindstillingen “{ $name }”?
develop-preset-delete-detail = Redigeringer, den er anvendt på, forbliver som de er.
develop-snapshot-delete-ask = Slet øjebliksbilledet “{ $name }”?
develop-snapshot-delete-detail = Redigeringen og dens historik forbliver som de er.
develop-version-delete-ask = Slet “{ $name }”?
develop-version-delete-detail = Denne versions redigering og historik fjernes. Fotoet og dets andre versioner bliver på disken.
develop-rule-delete-ask = Slet reglen “{ $rule }”?
develop-rule-delete-detail = Stilen bliver, og fotos, der allerede er åbnet, beholder deres redigering.

develop-rule-text = { $style }: { $when }
develop-rule-every-photo = alle fotos
develop-rule-iso-min = ISO { $iso } og op
develop-rule-iso-max = ISO op til { $iso }
develop-rule-iso-range = ISO { $low }–{ $high }
develop-rule-raw = raw-filer
develop-rule-other = ikke raw
develop-rule-any-camera = ethvert kamera
develop-next-placeholder = Stil til hver ny fil
develop-next-off-row = Ingen
develop-next-previous-row = Som forrige foto

develop-copied = { $count ->
    [one] 1 modul kopieret fra { $name }
   *[other] { $count } moduler kopieret fra { $name }
}
develop-pasted = { $count ->
    [one] Indsat på 1 foto
   *[other] Indsat på { $count } fotos
}
develop-nothing-to-copy = Intet foto at kopiere fra.
develop-nothing-copied = Intet kopieret endnu. Kopiér først en redigering (Ctrl+Alt+C).
develop-no-photos = Ingen fotos at ændre.
develop-no-picks = Ingen valgte fotos.
develop-reference-set = Reference: { $name }
develop-no-reference = Ingen reference endnu. Åbn referencefotoet, og vælg Brug som reference (Ctrl+Alt+R).
develop-reference-unreadable = Referencefotoet kunne ikke læses.
develop-matching = { $count ->
    [one] Matcher 1 foto…
   *[other] Matcher { $count } fotos…
}
develop-matched = { $count ->
    [one] 1 foto matchet
   *[other] { $count } fotos matchet
}
develop-matched-some = { $count } matchet. { $failed } kunne ikke læses eller måles.
develop-style-saved = Stil gemt: { $name }
develop-style-applied = Stil anvendt: { $name }
develop-style-imported = Stil importeret: { $name }
develop-style-not-a-style = Denne fil er ikke en stil.
develop-style-exported = Stil eksporteret: { $name }
develop-style-export-failed = Stilen kunne ikke skrives der.
develop-style-import-dialog = Importér en stil
develop-style-export-dialog = Eksportér stilen
develop-style-file-kind = Stil
develop-preset-saved = Forudindstilling gemt: { $name }
develop-preset-applied = Forudindstilling anvendt: { $name }
develop-snapshot-taken = Øjebliksbillede taget: { $name }
develop-history-compressed = { $count ->
    [0] Intet at komprimere
    [one] 1 trin fjernet
   *[other] { $count } trin fjernet
}
develop-version-created = Version oprettet: { $name }
develop-version-opened = Version åben: { $name }
develop-version-deleted = Version slettet: { $name }
develop-rule-added = { $name } gælder for nye fotos fra { $camera }.
develop-rule-applied = Automatisk regel: { $name }
develop-rules-none = Ingen automatiske regler. Tilføj en fra en stils menu.
develop-next-off = Nye filer får ingen stil.
develop-next-previous = Nye filer får det forrige fotos redigering.
develop-next-style = Nye filer får stilen { $name }.
develop-next-applied = { $count ->
    [one] Stil til næste billede på 1 ny fil
   *[other] Stil til næste billede på { $count } nye filer
}

develop-style-specular = Specular kraftig
develop-style-soft = Blød
develop-style-vivid = Livlig
develop-style-mono = Sort-hvid
develop-preset-plus-one = Lysere (+1 EV)
develop-preset-minus-one = Mørkere (−1 EV)
develop-preset-low-contrast = Lav kontrast
develop-preset-high-contrast = Høj kontrast
develop-preset-open-shadows = Lys skygger op
develop-preset-crisp = Skarp
develop-preset-muted = Dæmpet
develop-preset-high-iso = Høj ISO
develop-preset-screen = Til skærme

gallery-badge-versions = { $count } versioner
# machine-drafted, needs native review
develop-batch-row = Gemmer redigeringer
# machine-drafted, needs native review
develop-batch-row-detail = { $done } af { $total } fotos
# machine-drafted, needs native review
develop-batch-saving = Gemmer
# machine-drafted, needs native review
develop-batch-stopping = Stopper
# machine-drafted, needs native review
develop-batch-stopped = Stoppet. { $done } af { $total } fotos er gemt; resten er uændrede.
