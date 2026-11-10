# Specular — File kinds: which format a shared extension is, Open as…, remembered choices. Italiano.

## Which kind of file? Files whose extension several formats share.

# machine-drafted, needs native review
file-kind-title = Che tipo di file è { $file }?
# machine-drafted, needs native review
file-kind-why-content = Il suo contenuto è compatibile con più di un formato.
# machine-drafted, needs native review
file-kind-why-name = Il suo contenuto non lo dice. Questi formati usano file .{ $ext }.
# machine-drafted, needs native review
file-kind-why-none = Nessun formato riconosce il suo contenuto.
# machine-drafted, needs native review
file-kind-certain = Certo
# machine-drafted, needs native review
file-kind-likely = Probabile
# machine-drafted, needs native review
file-kind-possible = Possibile
# machine-drafted, needs native review
file-kind-guess = Solo dal nome
# machine-drafted, needs native review
file-kind-hint-same = come altri file .{ $ext } qui
# machine-drafted, needs native review
file-kind-hint-companions = i suoi file abituali sono accanto
# machine-drafted, needs native review
file-kind-scope = Usalo per
# machine-drafted, needs native review
file-kind-scope-file = Questo file
# machine-drafted, needs native review
file-kind-scope-folder = File .{ $ext } in questa cartella
# machine-drafted, needs native review
file-kind-scope-everywhere = Tutti i file .{ $ext }
# machine-drafted, needs native review
file-kind-later = Puoi cambiarlo in seguito con Apri come… nel menu del file.
# machine-drafted, needs native review
file-kind-open = Apri
# machine-drafted, needs native review
file-kind-raw = Campioni grezzi
# machine-drafted, needs native review
file-kind-open-as-none = Nient’altro legge { $file }.
# machine-drafted, needs native review
error-reason-ambiguous = Potrebbe essere uno di più formati. Scegline uno per aprirlo.

## Open as… (the palette and the gallery's file menu)

# machine-drafted, needs native review
palette-open-as = Apri come…
# machine-drafted, needs native review
palette-open-as-help = Scegli in quale formato leggere il file.

## Settings › Optional decoders › File kinds

# machine-drafted, needs native review
settings-decoders-file-kinds = Dimentica tutti i tipi di file…
# machine-drafted, needs native review
settings-decoders-file-kinds-help = Formati scelti per estensioni condivise da più formati. Verrà chiesto di nuovo la prossima volta.
# machine-drafted, needs native review
file-kind-rules-intro = Ogni regola apre i file di un’estensione in un formato.
# machine-drafted, needs native review
file-kind-rules-none = Ancora nessuna regola. Scegli «File .ext in questa cartella» o «Tutti i file .ext» quando richiesto.
# machine-drafted, needs native review
file-kind-rule = File .{ $ext }: { $format }
# machine-drafted, needs native review
file-kind-rule-folder = In { $folder }
# machine-drafted, needs native review
file-kind-rule-everywhere = Ovunque
# machine-drafted, needs native review
file-kind-forget = Dimentica…
# machine-drafted, needs native review
file-kind-forget-ask = Dimenticare «File .{ $ext }: { $format }»?
# machine-drafted, needs native review
file-kind-forget-detail = { -app-name } chiederà di nuovo la prossima volta. Nessun file viene modificato o rimosso.
# machine-drafted, needs native review
file-kind-forget-ok = Dimentica
# machine-drafted, needs native review
file-kind-forgotten = Dimenticato: file .{ $ext }: { $format }.
# machine-drafted, needs native review
file-kind-forget-all-ask = { $count ->
    [one] Dimenticare la regola del tipo di file?
   *[other] Dimenticare tutte le { $count } regole dei tipi di file?
}
# machine-drafted, needs native review
file-kind-forgotten-all = { $count ->
    [one] Regola del tipo di file dimenticata.
   *[other] { $count } regole dei tipi di file dimenticate.
}
# machine-drafted, needs native review
file-kind-keywords = estensione, formato, apri come, che tipo, tipo di file

## One line on what each format is, under its name in the prompt.

# machine-drafted, needs native review
file-kind-about-other = Un altro formato che usa file .{ $ext }.
# machine-drafted, needs native review
file-kind-about-raw-volume = Campioni senza intestazione. Indichi larghezza, altezza, sezioni e tipo di campione.
# machine-drafted, needs native review
file-kind-about-raw = Foto raw di fotocamera.
# machine-drafted, needs native review
file-kind-about-flir-fff = Immagine termica FLIR con temperature.
# machine-drafted, needs native review
file-kind-about-fits = Immagine astronomica (FITS).
# machine-drafted, needs native review
file-kind-about-fit = Traccia di attività Garmin: punti GPS e dati dei sensori.
# machine-drafted, needs native review
file-kind-about-mif = Volume medico MRtrix (RM).
# machine-drafted, needs native review
file-kind-about-map-info-mif = Mappa vettoriale MapInfo, con i suoi dati .mid.
# machine-drafted, needs native review
file-kind-about-net-cdf = Dati scientifici su griglia (clima, oceano).
# machine-drafted, needs native review
file-kind-about-hdf5 = Dati scientifici HDF5.
# machine-drafted, needs native review
file-kind-about-playlist = Playlist audio (PLS).
# machine-drafted, needs native review
file-kind-about-gerber = Strato di circuito stampato (Gerber), come la pasta saldante.
# machine-drafted, needs native review
file-kind-about-ser = Video astronomico da una camera planetaria (SER).
# machine-drafted, needs native review
file-kind-about-tia = Serie di microscopio elettronico (FEI TIA).
file-kind-about-pcd = Nuvola di punti 3D (PCD).
file-kind-about-photo-cd = Scansione Kodak Photo CD.
file-kind-about-zx-screen = Schermata di ZX Spectrum.
file-kind-about-cpc-screen = Schermata di Amstrad CPC.
file-kind-about-msx-screen = Schermata di MSX.
file-kind-about-scc = Sottotitoli nascosti Scenarist.
# machine-drafted, needs native review
file-kind-about-pict = Immagine Mac classica (PICT).
# machine-drafted, needs native review
file-kind-about-hdr = Immagine HDR Radiance.
# machine-drafted, needs native review
file-kind-about-analyze = Volume medico Analyze 7.5, con il suo .hdr.
# machine-drafted, needs native review
file-kind-about-envi = Raster di telerilevamento (ENVI), con il suo .hdr.
# machine-drafted, needs native review
file-kind-about-pds3 = Immagine di missione planetaria (PDS3), con la sua etichetta.
# machine-drafted, needs native review
file-kind-about-mpeg-ps = Video da videocamera (program stream MPEG, MOD/TOD).
# machine-drafted, needs native review
file-kind-about-tracker = Modulo musicale tracker (MOD, S3M, XM, IT, 669, MTM, STM, OKT, MED, ULT, FAR).
# machine-drafted, needs native review
file-kind-about-softimage-pic = Render 3D di Softimage (PIC).
# machine-drafted, needs native review
file-kind-about-dr-halo = Immagine Dr. Halo, con la sua tavolozza .pal.
# machine-drafted, needs native review
file-kind-about-wal = Texture di Quake 2, colorata con la tavolozza del gioco.
# machine-drafted, needs native review
file-kind-about-quick-time = Filmato QuickTime.
file-kind-about-pro-res-raw = Video Apple ProRes RAW: fotogrammi dal sensore della videocamera.
file-kind-about-cinema-raw-light = Video Canon Cinema RAW Light: fotogrammi dal sensore della videocamera.
# machine-drafted, needs native review
file-kind-about-gimp-brush = Pennello di GIMP.
# machine-drafted, needs native review
file-kind-about-swatch-exchange = Tavolozza Adobe Swatch Exchange.
# machine-drafted, needs native review
file-kind-about-aseprite = Sprite pixel art di Aseprite.
