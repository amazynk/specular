# machine-drafted, needs native review
# Specular — Develop: cronologia, istantanee, stili, preimpostazioni, versioni, copia e incolla, corrispondenza. Italiano.

palette-develop-copy = Copia modifiche
palette-develop-copy-help = Copia le modifiche della foto, tranne ritaglio e obiettivo, per incollarle su altre foto.
palette-develop-copy-some = Copia alcuni moduli…
palette-develop-copy-some-help = Scegli quali moduli delle modifiche copiare.
palette-develop-paste = Incolla modifiche
palette-develop-paste-help = Incolla le modifiche copiate sulla foto, o sulle foto selezionate nella galleria.
palette-develop-paste-some = Incolla alcuni moduli…
palette-develop-paste-some-help = Scegli quali moduli copiati incollare, aggiungendo o sostituendo.
palette-develop-paste-picks = Incolla modifiche sulle scelte
palette-develop-paste-picks-help = Incolla le modifiche copiate su ogni foto scelta.
palette-develop-reference = Usa come riferimento
palette-develop-reference-help = Altre foto potranno riprendere l’esposizione e il bilanciamento del bianco di questa.
palette-develop-match = Uniforma al riferimento
palette-develop-match-help = Regola esposizione e bilanciamento del bianco della foto, della selezione o delle scelte sul riferimento.
palette-develop-sync = Sincronizza sulle scelte
palette-develop-sync-help = Metti le modifiche di questa foto, tranne ritaglio e obiettivo, sulle foto scelte.
palette-develop-sync-some = Sincronizza alcuni moduli…
palette-develop-sync-some-help = Scegli quali moduli di questa foto vanno sulle foto scelte.
palette-develop-next-capture = Stile dei prossimi scatti…
palette-develop-next-capture-help = Lo stile che riceve ogni nuovo file che arriva nella cartella.
palette-develop-auto-rules = Regole automatiche…
palette-develop-auto-rules-help = Le regole che applicano uno stile la prima volta che una foto si apre in Develop.
palette-develop-import-style = Importa stile…
palette-develop-import-style-help = Aggiungi uno stile da un file.
palette-develop-snapshot = Scatta istantanea
palette-develop-snapshot-help = Dai un nome alle modifiche come sono ora, per confrontarle o tornarci più tardi.
palette-develop-compress = Compatta cronologia
palette-develop-compress-help = Togli i passaggi sostituiti da altri successivi. Le modifiche restano uguali.
palette-develop-new-version = Nuova versione
palette-develop-new-version-help = Un’altra modifica della stessa foto, con la sua cronologia. Il file non viene copiato.
palette-develop-new-style = Nuovo stile…
palette-develop-new-style-help = Salva i moduli scelti delle modifiche come stile.

develop-styles = Stili
develop-snapshots = Cronologia e istantanee
develop-versions = Versioni
develop-style-new = Nuovo stile da queste modifiche…
develop-style-import = Importa stile…
develop-rules = Regole automatiche…
develop-snapshot-take = Scatta istantanea
develop-history-compress = Compatta cronologia
develop-snapshot-comparing = Confronto: { $name }
develop-snapshot-compare-off = Confronto spento
develop-version-number = Versione { $number }
develop-version-open = { $name } (aperta)
develop-version-new = Nuova versione
develop-version-duplicate = Duplica questa versione

develop-history-paste = Incolla
develop-history-sync = Sincronizza
develop-history-match = Uniforma al riferimento
develop-history-snapshot = Istantanea: { $name }
develop-history-previous = Come la precedente

develop-rename = Rinomina…
develop-delete = Elimina
develop-save = Salva
develop-name = Nome
develop-style-apply = Applica
develop-style-apply-replace = Applica sostituendo questi moduli
develop-style-auto = Applica alle nuove foto di questa fotocamera
develop-style-export = Esporta…
develop-snapshot-restore = Torna a questa istantanea
develop-version-open-it = Apri
develop-preset-save = Salva come preimpostazione…
develop-preset-save-module = Salva { $module } come preimpostazione…
develop-preset-delete = Elimina «{ $name }»…

develop-sheet-modules = Moduli
develop-sheet-included = Incluso
develop-sheet-left-out = Escluso
develop-sheet-more = altri { $count }
develop-copy-title = Copia moduli
develop-copy-ok = Copia
develop-paste-title = Incolla moduli
develop-paste-ok = Incolla
develop-paste-mode = Moduli già presenti nelle modifiche
develop-paste-append = Aggiungi
develop-paste-replace = Sostituisci
develop-style-new-title = Nuovo stile
develop-style-save = Salva stile
develop-style-default-name = Il mio stile { $number }
develop-style-rename-title = Rinomina stile
develop-preset-save-title = Preimpostazione { $module }
develop-preset-default-name = La mia preimpostazione
develop-snapshot-take-title = Scatta istantanea
develop-snapshot-default-name = Istantanea { $number }
develop-snapshot-rename-title = Rinomina istantanea
develop-version-new-title = Nuova versione
develop-version-duplicate-title = Duplica versione
develop-version-create = Crea
develop-version-rename-title = Rinomina versione

develop-style-delete-ask = Eliminare lo stile «{ $name }»?
develop-style-delete-detail = Le foto a cui è stato applicato mantengono le modifiche. Il file dello stile viene rimosso.
develop-preset-delete-ask = Eliminare la preimpostazione «{ $name }»?
develop-preset-delete-detail = Le modifiche a cui è stata applicata restano come sono.
develop-snapshot-delete-ask = Eliminare l’istantanea «{ $name }»?
develop-snapshot-delete-detail = Le modifiche e la loro cronologia restano come sono.
develop-version-delete-ask = Eliminare «{ $name }»?
develop-version-delete-detail = Le modifiche e la cronologia di questa versione vengono rimosse. La foto e le altre versioni restano sul disco.
develop-rule-delete-ask = Eliminare la regola «{ $rule }»?
develop-rule-delete-detail = Lo stile resta, e le foto già aperte mantengono le modifiche.

develop-rule-text = { $style }: { $when }
develop-rule-every-photo = ogni foto
develop-rule-iso-min = ISO { $iso } e oltre
develop-rule-iso-max = ISO fino a { $iso }
develop-rule-iso-range = ISO { $low }–{ $high }
develop-rule-raw = file raw
develop-rule-other = non raw
develop-rule-any-camera = qualsiasi fotocamera
develop-next-placeholder = Stile di ogni nuovo file
develop-next-off-row = Nessuno
develop-next-previous-row = Come la foto precedente

develop-copied = { $count ->
    [one] 1 modulo copiato da { $name }
   *[other] { $count } moduli copiati da { $name }
}
develop-pasted = { $count ->
    [one] Incollato su 1 foto
   *[other] Incollato su { $count } foto
}
develop-nothing-to-copy = Nessuna foto da cui copiare.
develop-nothing-copied = Ancora nulla di copiato. Copia prima delle modifiche (Ctrl+Alt+C).
develop-no-photos = Nessuna foto da cambiare.
develop-no-picks = Nessuna foto scelta.
develop-reference-set = Riferimento: { $name }
develop-no-reference = Ancora nessun riferimento. Apri la foto di riferimento e scegli Usa come riferimento (Ctrl+Alt+R).
develop-reference-unreadable = Impossibile leggere la foto di riferimento.
develop-matching = { $count ->
    [one] Uniformo 1 foto…
   *[other] Uniformo { $count } foto…
}
develop-matched = { $count ->
    [one] 1 foto uniformata
   *[other] { $count } foto uniformate
}
develop-matched-some = { $count } uniformate. { $failed } non si sono potute leggere o misurare.
develop-style-saved = Stile salvato: { $name }
develop-style-applied = Stile applicato: { $name }
develop-style-imported = Stile importato: { $name }
develop-style-not-a-style = Questo file non è uno stile.
develop-style-exported = Stile esportato: { $name }
develop-style-export-failed = Impossibile scrivere lo stile lì.
develop-style-import-dialog = Importa uno stile
develop-style-export-dialog = Esporta lo stile
develop-style-file-kind = Stile
develop-preset-saved = Preimpostazione salvata: { $name }
develop-preset-applied = Preimpostazione applicata: { $name }
develop-snapshot-taken = Istantanea scattata: { $name }
develop-history-compressed = { $count ->
    [0] Niente da compattare
    [one] 1 passaggio tolto
   *[other] { $count } passaggi tolti
}
develop-version-created = Versione creata: { $name }
develop-version-opened = Versione aperta: { $name }
develop-version-deleted = Versione eliminata: { $name }
develop-rule-added = { $name } si applica alle nuove foto di { $camera }.
develop-rule-applied = Regola automatica: { $name }
develop-rules-none = Nessuna regola automatica. Aggiungine una dal menu di uno stile.
develop-next-off = I nuovi file non ricevono stili.
develop-next-previous = I nuovi file ricevono le modifiche della foto precedente.
develop-next-style = I nuovi file ricevono lo stile { $name }.
develop-next-applied = { $count ->
    [one] Stile dei prossimi scatti su 1 nuovo file
   *[other] Stile dei prossimi scatti su { $count } nuovi file
}

develop-style-specular = Specular deciso
develop-style-soft = Morbido
develop-style-vivid = Vivace
develop-style-mono = Bianco e nero
develop-preset-plus-one = Più chiaro (+1 EV)
develop-preset-minus-one = Più scuro (−1 EV)
develop-preset-low-contrast = Contrasto basso
develop-preset-high-contrast = Contrasto alto
develop-preset-open-shadows = Ombre aperte
develop-preset-crisp = Nitido
develop-preset-muted = Tenue
develop-preset-high-iso = ISO alti
develop-preset-screen = Per schermi

gallery-badge-versions = { $count } versioni
# machine-drafted, needs native review
develop-batch-row = Salvataggio delle modifiche
# machine-drafted, needs native review
develop-batch-row-detail = { $done } di { $total } foto
# machine-drafted, needs native review
develop-batch-saving = Salvataggio
# machine-drafted, needs native review
develop-batch-stopping = Arresto in corso
# machine-drafted, needs native review
develop-batch-stopped = Interrotto. Salvate { $done } di { $total } foto; le altre sono invariate.
