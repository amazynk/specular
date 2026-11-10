# Specular — Develop export. Français.
# Exports are new files; the photo and its edit are never changed.

# machine-drafted, needs native review
palette-develop-export = Exporter…
# machine-drafted, needs native review
palette-develop-export-help = Enregistrer la photo retouchée, ou plusieurs, dans de nouveaux fichiers.

## The dialog

# machine-drafted, needs native review
develop-export-title = Exporter
# machine-drafted, needs native review
develop-export-start = Exporter
# machine-drafted, needs native review
develop-export-preset = Préréglage
# machine-drafted, needs native review
develop-export-preset-custom = Personnalisé
# machine-drafted, needs native review
develop-export-preset-jpeg = JPEG, taille réelle
# machine-drafted, needs native review
develop-export-preset-web = Web, 2048 px
# machine-drafted, needs native review
develop-export-preset-tiff16 = TIFF 16 bits pour l’impression
# machine-drafted, needs native review
develop-export-preset-exr = OpenEXR, linéaire
# machine-drafted, needs native review
develop-export-save-preset = Enregistrer le préréglage
# machine-drafted, needs native review
develop-export-preset-saved = Enregistré comme préréglage « { $name } ».
# machine-drafted, needs native review
develop-export-preset-exists = C’est déjà le préréglage « { $name } ».
# machine-drafted, needs native review
develop-export-delete-preset = Supprimer le préréglage
# machine-drafted, needs native review
develop-export-delete-preset-ask = Supprimer « { $name } » ?
# machine-drafted, needs native review
develop-export-delete-preset-confirm = Supprimer le préréglage « { $name } » ? Appuyez de nouveau sur Suppr pour le supprimer. Les fichiers déjà exportés restent.
# machine-drafted, needs native review
develop-export-preset-deleted = Préréglage « { $name } » supprimé. Les fichiers déjà exportés restent.
# machine-drafted, needs native review
develop-export-preset-name = { $format } · { $space } · { $size }
# machine-drafted, needs native review
develop-export-size-full = taille réelle
# machine-drafted, needs native review
develop-export-format-exr = OpenEXR
# machine-drafted, needs native review
develop-export-depth-half = virgule flottante 16 bits
# machine-drafted, needs native review
develop-export-depth-float = virgule flottante 32 bits
# machine-drafted, needs native review
develop-export-space = Espace colorimétrique
# machine-drafted, needs native review
develop-export-space-srgb = sRGB
# machine-drafted, needs native review
develop-export-space-display-p3 = Display P3
# machine-drafted, needs native review
develop-export-space-rec2020 = Rec. 2020
# machine-drafted, needs native review
develop-export-space-adobe-rgb = Adobe RGB (compatible)
# machine-drafted, needs native review
# HDR output of an export. PQ and HLG are HDR files; a gain map is an SDR photo that also shows in HDR where the screen can.
develop-export-hdr = HDR
# machine-drafted, needs native review
develop-export-hdr-off = Désactivé (SDR)
# machine-drafted, needs native review
develop-export-hdr-pq = PQ
# machine-drafted, needs native review
develop-export-hdr-hlg = HLG
# machine-drafted, needs native review
develop-export-hdr-gain-map = Carte de gain

## Progress and the end

# machine-drafted, needs native review
develop-export-progress = Exportation de { $done } sur { $total }
# machine-drafted, needs native review
develop-export-progress-eta = Exportation de { $done } sur { $total } · il reste environ { $time }
# machine-drafted, needs native review
develop-export-done = { $count ->
    [one] { $count } image exportée
   *[other] { $count } images exportées
}
# machine-drafted, needs native review
develop-export-stopped = Arrêté après { $count }
# machine-drafted, needs native review
develop-export-running = Une exportation ou une conversion est déjà en cours.
# machine-drafted, needs native review
develop-export-row = Exportation
# machine-drafted, needs native review
develop-export-row-detail = { $done } sur { $total }
# machine-drafted, needs native review
develop-export-starting = Démarrage
# machine-drafted, needs native review
develop-export-no-gpu = L’exportation nécessite un périphérique graphique, et aucun n’a pu être ouvert.
