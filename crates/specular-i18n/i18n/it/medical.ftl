# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Immagini mediche
palette-window-level = Strumento finestra/livello (trascina con il sinistro)
palette-window-default = Finestra: predefinita del file
palette-window-lung = Finestra: polmone (W 1500, L −600)
palette-window-mediastinum = Finestra: mediastino (W 350, L 50)
palette-window-bone = Finestra: osso (W 1800, L 400)
palette-window-brain = Finestra: cervello (W 80, L 40)
palette-window-stroke = Finestra: ictus (W 40, L 40)
palette-window-soft-tissue = Finestra: tessuti molli (W 400, L 50)
palette-window-liver = Finestra: fegato (W 150, L 30)
palette-window-subdural = Finestra: subdurale (W 200, L 75)
palette-window-full-range = Finestra: intervallo completo
palette-invert-grey = Inverti grigi
palette-next-slice = Fotogramma successivo
palette-prev-slice = Fotogramma precedente
disabled-not-medical = Solo per immagini mediche in scala di grigi
disabled-one-frame = L’immagine ha un solo fotogramma
settings-section-medical = Immagini mediche
settings-medical-keep-window = Mantieni finestra/livello per la serie successiva
settings-medical-keep-window-help = Mantiene la finestra impostata passando a un’altra serie, invece di partire dal suo valore predefinito. I fotogrammi di un file e le sezioni di una serie la condividono sempre.
medical-window = C { $center }  W { $width }
medical-inverted = invertito
medical-frame = Fotogramma { $frame } di { $count }
medical-tool = Strumento finestra/livello: trascina
medical-tool-on = Strumento finestra/livello attivo: trascina per regolare
medical-tool-off = Strumento finestra/livello disattivato
medical-preset-not-ct = I preset TC richiedono valori in unità Hounsfield
medical-file-window-none = Nessuna finestra { $index } in questo file
medical-file-window-unnamed = Finestra del file { $index } di { $count }
medical-file-window = Finestra del file { $index } di { $count }: { $name }
measure-templates-need-medical = I modelli geometrici sono inclusi nel profilo Medicina
medical-off = Nessuno strumento medico per questa immagine: { $reason }
medical-readout = Valore e finestra

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Riproduci fotogrammi (cine)
palette-measure-length = Misura lunghezza
palette-measure-angle = Misura angolo
palette-measure-ellipse = Misura regione ellittica
palette-measure-rectangle = Misura regione rettangolare
palette-clear-measurements = Cancella misure
palette-measure-templates = Modelli geometrici…
palette-overlays = Mostra o nascondi sovrapposizioni
medical-cine = Riproduzione a { $rate } fotogrammi/s
medical-overlays-shown = Sovrapposizioni visibili
medical-overlays-hidden = Sovrapposizioni nascoste
measure-hint-length = Lunghezza: trascina o fai clic su entrambe le estremità
measure-hint-angle = Angolo: fai clic su tre punti, il vertice per secondo
measure-hint-ellipse = Ellisse: trascina sulla regione
measure-hint-rectangle = Rettangolo: trascina sulla regione
measure-off = Strumento di misura disattivato
measure-cleared = Misure cancellate
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Area { $value } mm²
measure-area-px = Area { $value } px²
measure-angle = { $value }°
measure-mean = Media { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Max { $max }  ({ $count } px)
measure-caveat-detector = Al rivelatore: non corretto per l’ingrandimento
measure-caveat-magnification = Correzione dell’ingrandimento stimata
measure-caveat-uncalibrated = Nessuna spaziatura dei pixel: in pixel
measure-caveat-outside-region = Fuori da una regione ecografica calibrata: in pixel

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Mostra etichette di orientamento
settings-medical-orientation-labels-help = Lettere ai bordi dell’immagine con le direzioni del paziente (R/L destra/sinistra, A/P anteriore/posteriore, S/I testa/piedi), lette dal file e che seguono rotazioni e capovolgimenti. L le mostra o nasconde per la sessione.
settings-medical-wheel-steps-frames = La rotella scorre i fotogrammi
settings-medical-wheel-steps-frames-help = In un file o una serie con più fotogrammi la rotella passa da un fotogramma all’altro invece di eseguire lo zoom o cambiare immagine. Ctrl+rotella esegue comunque lo zoom.
# machine-drafted, needs native review
settings-medical-display-function = Visualizzazione dei grigi
settings-medical-display-function-help = Come vengono mostrati i grigi delle immagini mediche. DICOM GSDF (funzione standard di visualizzazione della scala di grigi) li distribuisce in passi di uguale visibilità tra la luminanza del nero e quella del bianco dello schermo (DICOM PS3.14), come su un monitor diagnostico calibrato. Il profilo colore dello schermo continua ad applicarsi.
settings-medical-display-function-srgb = Standard (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Luminanza del nero dello schermo
settings-medical-display-black-help = La luminanza del nero dello schermo, luce ambientale inclusa, misurata o ricavata dalla scheda tecnica dello schermo. Usata da DICOM GSDF.
settings-medical-display-white = Luminanza del bianco dello schermo
settings-medical-display-white-help = La luminanza del bianco dello schermo, misurata o ricavata dalla scheda tecnica dello schermo. Usata da DICOM GSDF.
palette-orientation-labels = Mostra o nascondi etichette di orientamento
palette-presentation-state = Stato di presentazione successivo
disabled-no-presentation-state = Nessuno stato di presentazione fa riferimento a questa immagine
medical-labels-shown = Etichette di orientamento visibili
medical-labels-hidden = Etichette di orientamento nascoste
medical-presentation = Stato di presentazione: { $label } ({ $index } di { $count })
medical-presentation-none = Stato di presentazione disattivato
medical-presentation-readout = Presentazione: { $label }
medical-preset-presentation = La tabella di lookup dello stato di presentazione determina i livelli di grigio

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientamento: sinistra { $left }, destra { $right }, alto { $top }, basso { $bottom }
