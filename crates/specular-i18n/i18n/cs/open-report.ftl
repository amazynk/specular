# machine-drafted, needs native review
# Specular — Zpráva o otevření: proč otevření obrázku trvalo tak dlouho (informační panel,
# oddíl „Výkon“; DESIGN.md §12.7).

open-report-title = Výkon

## Headlines
open-report-fast = Otevřeno za { $ms } ms
open-report-display-cache = Zobrazeno z mezipaměti zobrazení
open-report-slow-read = Pomalé úložiště: { $percent } času zabralo čtení souboru ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Pomalé úložiště: { $percent } času zabralo čtení souboru (malý soubor: většinou čekání na odezvu úložiště)
open-report-archive-read = { $percent } času zabralo rozbalování archivu
open-report-startup = { $percent } času zabralo spouštění aplikace { -app-name }
open-report-checked = Dekódováno dvakrát: první výsledek hardwarového dekodéru byl porovnán s výsledkem CPU ({ $percent } času)
open-report-hardware = Dekódováno hardwarovým dekodérem, který zabral { $percent } času
open-report-decode = Velký obrázek: dekódování { $mpx } megapixelů zabralo { $percent } času
open-report-upload = { $percent } času zabralo odeslání obrázku do GPU
open-report-waiting = { $percent } času zabralo čekání na jinou práci
open-report-stage = { $stage } zabralo { $percent } času
open-report-memory-short = Nedostávalo se paměti, systém možná musel odkládat na disk

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Dekódováno softwarově: hardwarový dekodér se teprve zjišťoval
open-report-sw-no-device = Dekódováno softwarově: { $api } není na tomto počítači k dispozici
open-report-sw-no-codec = Dekódováno softwarově: { $api } nemá na tomto GPU dekodér { $codec }
open-report-sw-blocked-mismatch = Dekódováno softwarově: hardwarový dekodér byl vypnut, protože jeho výsledek neodpovídal výsledku CPU
open-report-sw-blocked-error = Dekódováno softwarově: hardwarový dekodér byl vypnut po chybě
open-report-sw-blocked-lost = Dekódováno softwarově: hardwarový dekodér byl vypnut, protože přestal odpovídat
open-report-sw-failed = Dekódováno softwarově: hardwarový dekodér u tohoto obrázku selhal
open-report-sw-busy = Dekódováno softwarově: hardwarový dekodér byl zaneprázdněn
open-report-sw-unsupported = Dekódováno softwarově: hardwarový dekodér tento obrázek nezvládne
open-report-sw-unavailable = Dekódováno softwarově: hardwarové dekódování je vypnuto do restartu aplikace { -app-name }
open-report-sw-cpu-only = Dekódováno softwarově: zpracování je nastaveno na „Pouze CPU“
open-report-sw-setting-off = Dekódováno softwarově: hardwarové dekódování je vypnuto v nastavení
open-report-sw-unchecked = Dekódováno softwarově: hardwarový dekodér ještě není ověřen (nejprve se zkouší na obrázku připravovaném na pozadí)
open-report-sw-lower-power = Dekódováno softwarově: na baterii na to CPU spotřebuje méně energie
open-report-sw-faster = Dekódováno softwarově: na tomto počítači je to rychlejší (asi { $cpu } ms oproti { $hw } ms na hardwarovém dekodéru)
open-report-sw-scaled = Dekódováno softwarově: hardwarový dekodér dekóduje obrázky jen v plné velikosti

## Hints
open-report-hint-copy-local = Zkopírujte soubor na místní disk, aby se otevíral rychleji.
open-report-hint-extract = Rozbalte archiv do složky, aby se jeho stránky otevíraly rychleji.
open-report-hint-try-again = Chcete-li hardwarový dekodér zkusit znovu: Nastavení → Výkon → Zkusit znovu.
open-report-hint-automatic = Nastavte „Kde se obrázky zpracovávají“ na Automaticky v Nastavení → Výkon.
open-report-hint-turn-on = Nastavte „Hardwarové dekódování“ na Automaticky v Nastavení → Výkon.
open-report-hint-free-memory = Zavřete jiné programy a uvolněte tak paměť.
open-report-hint-once-only = Takto se kontroluje jen první obrázek.

## Rows
open-report-total = Do zaostření
open-report-first-pixels = První pixely
# machine-drafted, needs native review
open-report-whole = Zobrazen celý obrázek
open-report-stage-startup = Spouštění
open-report-stage-read = Čtení souboru
open-report-stage-parse = Čtení hlavičky
open-report-stage-colour = Příprava správy barev
open-report-stage-decode = Dekódování
open-report-stage-upload = Odeslání do GPU
open-report-stage-waiting = Čekání na jinou práci
open-report-decoder = Dekódoval
open-report-decoder-checked = { $engine }, ověřeno proti CPU
open-report-decoder-cache = Mezipaměť zobrazení
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, v 1/{ $scale } velikosti
open-report-decoder-scaled-full = { $engine }, v plné velikosti, poté zmenšeno na 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, v 1/{ $scale } velikosti, bez indexu pro rychlý start
open-report-colour-conversion = Barvy převedl

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Opakuje se
open-report-advice-change = Pomohlo by
open-report-advice-gain = Odhadovaný přínos
open-report-advice-software = U { $count } z posledních { $total } pomalých obrázků { $format } proběhlo dekódování z tohoto důvodu softwarově
open-report-advice-slow-storage = U { $count } z posledních { $total } pomalých obrázků zdrželo otevírání čtení souboru (asi { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = U { $count } z posledních { $total } pomalých obrázků zdrželo otevírání čtení souboru (malé soubory: většinou čekání na odezvu úložiště)
open-report-advice-archive = U { $count } z posledních { $total } pomalých obrázků zdrželo otevírání rozbalování archivu
open-report-advice-hardware-decoder = GPU s hardwarovým dekodérem { $codec }
open-report-advice-gain-range = Asi { $low }krát až { $high }krát rychleji
open-report-advice-gain-one = Asi { $low }krát rychleji

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Tipy k výkonu
open-report-insights-indicator = Proč je to pomalé?
open-report-insights-announce = Tip k výkonu: obrázky se opakovaně otevírají pomalu ze stejného důvodu. Otevřete „Proč je to pomalé?“ a uvidíte, co by pomohlo.
open-report-insights-hide = Skrýt
open-report-insights-last = Poslední pomalý obrázek: { $total } do zaostření
open-report-insights-now = Co můžete udělat hned
open-report-insights-hardware = Hardware, který by pomohl
open-report-insights-none = Žádné opakované zpomalení
open-report-insights-none-detail = Tipy se objeví, když se několik obrázků otevře pomalu ze stejného důvodu. Aby je našel, uchovává { -app-name } na tomto počítači pomalá otevření za poslední týden; nic se nikam neodesílá.
open-report-insights-software = Obrázky { $format } se dekódují na CPU
open-report-insights-storage = Čtení souborů zdržuje obrázky
open-report-insights-archive = Rozbalování archivů zdržuje obrázky
open-report-insights-nothing-now = Žádné nastavení aplikace { -app-name } na tom nic nezmění.
open-report-insights-display-cache = Zapněte mezipaměť zobrazení: obrázky, které jste už otevřeli, se pak otevřou z rychlé kopie na tomto počítači.
open-report-insights-prefetch = Mějte připraveno více sousedních obrázků, aby se další připravovaly, zatímco se díváte.
open-report-insights-hw-ssd = Rychlý interní disk (NVMe SSD) pro tyto soubory
open-report-insights-action-turn-on = Zapnout
open-report-insights-action-try-again = Zkusit znovu
open-report-insights-action-automatic = Nastavit na Automaticky
open-report-insights-dismiss-storage = U úložiště už nezobrazovat
open-report-insights-dismiss-decoder = U dekódování už nezobrazovat
open-report-insights-off = Vypnout tipy
open-report-insights-on = Zapnout tipy
open-report-insights-restore = Znovu zobrazit skryté tipy
open-report-insights-power = Tento počítač šetří energii (běží na baterii nebo v úsporném režimu), což zpomaluje dekódování na CPU. Po připojení k napájení se obrázky mohou otevírat rychleji.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Tip k výkonu: miniatury se opakovaně zobrazují pomalu ze stejného důvodu. Otevřete „Proč je to pomalé?“ a uvidíte, co by pomohlo.
open-report-insights-last-thumbnail = Poslední pomalá miniatura: { $total } na vytvoření
open-report-insights-thumbnails-software = Miniatury obrázků { $format } se dekódují na CPU
open-report-insights-thumbnails-storage = Čtení souborů zdržuje miniatury
open-report-insights-thumbnails-archive = Rozbalování archivů zdržuje miniatury
open-report-insights-thumbnail-cache = Miniatury nelze na tomto počítači uchovat (mezipaměť miniatur není dostupná), proto se vytvářejí znovu při každém zobrazení složky.
open-report-advice-thumbnails-software = U { $count } z posledních { $total } pomalých miniatur { $format } proběhlo dekódování z tohoto důvodu softwarově
open-report-advice-thumbnails-slow-storage = U { $count } z posledních { $total } pomalých miniatur zdrželo vytváření čtení souboru (asi { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = U { $count } z posledních { $total } pomalých miniatur zdrželo vytváření čtení souboru (malé soubory: většinou čekání na odezvu úložiště)
open-report-advice-thumbnails-archive = U { $count } z posledních { $total } pomalých miniatur zdrželo vytváření rozbalování archivu
