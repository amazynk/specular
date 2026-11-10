# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Contrôler
ndt-probe = Sonde : { $name }
ndt-probe-profile = Profil de ligne
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = IQI duplex à fils
ndt-probe-wire-iqi = IQI à fil unique
ndt-probe-crack = Largeur de fissure
ndt-probe-density = Point de densité

ndt-hint-profile = Tracez une ligne pour afficher les niveaux de gris le long de celle-ci. [ et ] changent de sonde.
ndt-hint-snr = Tracez une zone dans une région uniforme. Elle est agrandie à au moins 20 × 55 pixels.
ndt-hint-duplex-iqi = Tracez une ligne sur toutes les paires de fils, le long de l’IQI.
ndt-hint-wire-iqi = Tracez une courte ligne en travers du fil le plus fin visible.
ndt-hint-crack = Cliquez le long de la fissure, puis appuyez sur Entrée. La largeur est lue en 10 points.
ndt-hint-density = Cliquez sur un point pour lire son niveau de gris. Saisissez sa densité connue pour étalonner.

ndt-refused-needs-scale = Définissez d’abord l’échelle : la taille des fils se mesure en millimètres. Utilisez Étalonner sur une longueur connue.
ndt-refused-nothing-found = Rien ne se détache du bruit le long de cette ligne.
ndt-refused-too-small = L’image est plus petite que la zone SNR de 20 × 55 pixels.
ndt-refused-no-noise = Cette zone n’a aucun bruit à mesurer (tous les pixels sont identiques).

ndt-reading = Lecture…
ndt-no-pixels = Impossible de lire les pixels.

ndt-profile-range = Gris { $min } à { $max }
ndt-profile-mean = Gris moyen { $mean }
ndt-profile-density = Densité { $min } à { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = Le SNRn nécessite la SRb : mesurez un IQI duplex ou saisissez-la.
ndt-snr-stats = Moyenne { $mean }, σ { $sd }, zone { $w } × { $h } px

ndt-duplex-result = Première paire non résolue { $pair } : SRb { $srb } µm, flou { $unsharpness }
ndt-duplex-all-resolved = Toutes les paires trouvées sont résolues.
ndt-duplex-dips = Creux : { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolé : iSRb { $isrb } µm, flou { $unsharpness }

ndt-wire-result = Fil { $wire }, { $diameter }
ndt-wire-measured = Largeur mesurée { $width } (FWHM), soit environ { $diameter } de diamètre

ndt-crack-mean = Largeur moyenne { $width }
ndt-crack-range = La plus étroite { $min }, la plus large { $max }
ndt-crack-cuts = Points de mesure : { $count }

ndt-density-value = Gris { $grey }, densité { $density }
ndt-density-grey = Gris { $grey }
ndt-density-uncalibrated = Densité pas encore étalonnée : saisissez des densités connues en deux points.

ndt-ask-density = Densité connue ici : { $value } (Entrée applique, Entrée sans valeur passe)
ndt-ask-srb = Résolution spatiale de base SRb : { $value } µm (Entrée applique, Esc passe)
ndt-bad-density = Saisissez une densité de 0 à 10.
ndt-bad-srb = Saisissez la SRb en micromètres, par exemple 80.
ndt-srb-set = SRb fixée à { $srb } µm pour cette image.
ndt-density-one-more = Point de densité conservé. Saisissez la densité en un point de plus.
ndt-density-calibrated = Densité étalonnée à partir de deux points.
ndt-sidecar-failed = Impossible d’enregistrer les réglages d’inspection à côté du fichier : { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m au-dessus du point de décollage
map-export-kml = Enregistrer les photos en KMZ ou KML…
map-export-kmz-full = Enregistrer les photos en KMZ avec les photos complètes…
map-kml-heading = Cap { $degrees }°
map-kml-height = { $metres } m au-dessus du point de décollage
map-kml-nothing = Aucune photo avec une position à enregistrer.
map-kml-saved = Photos enregistrées dans { $name } : { $count }.
map-kml-failed = Impossible d’enregistrer le fichier de carte : { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Rapport d’inspection
inspection-report-default-title = Rapport d’inspection
inspection-report-folder = Ajouter les autres photos de ce dossier
inspection-report-docx = Enregistrer aussi un document Word (DOCX)
inspection-report-kmz = Enregistrer aussi un KMZ des lieux des photos
inspection-report-caption = Photo avec les mesures
inspection-report-col-value = Valeur
inspection-report-col-scale = Échelle d’après
inspection-report-docx-failed = Impossible d’écrire le document Word.
inspection-report-unreadable = Aucune des photos n’a pu être lue.
