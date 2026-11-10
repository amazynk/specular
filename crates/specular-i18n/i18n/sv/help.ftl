# machine-drafted, needs native review
## Handbokens panel, kraschrapporter, diagnostikpaketet, uppdateringskontroll och Om. Svenska.
## Stil: kort, tydligt, utan utfyllnad; en tanke per mening.

## Palett

palette-help-manual = Hjälp
palette-help-manual-help = Öppna handboken på sidan för det som visas.
palette-save-diagnostics = Spara diagnostikpaket…
palette-save-diagnostics-help = Spara kraschrapporter, loggen och systeminformation i en zip-fil. Inget skickas.
# En palettpost som öppnar ett avsnitt i handboken; { $title } är en sida eller ”Sida › Avsnitt”.
palette-help-topic = Hjälp: { $title }
palette-help-topic-help = Öppna det här avsnittet i handboken.
# Under ett kommando som inte är tillgängligt i paletten.
palette-disabled-help = Inte tillgängligt nu. Tryck på Enter för att läsa varför.

## Hjälppanelen

help-title = Hjälp
help-search-placeholder = Sök i handboken
help-contents = Innehåll
help-back = Bakåt
help-forward = Framåt
help-no-results = Inget hittades för ”{ $query }”.
help-results = { $count ->
    [one] 1 avsnitt hittades
   *[other] { $count } avsnitt hittades
}
help-section-start = Kom igång
help-section-modes = Visning och lägen
help-section-tasks = Hur gör jag…
help-section-reference = Referens
help-keys-unbound = Inte angivet
# Ett kommando i handboken utan tangent; det körs från paletten.
help-key-unbound = { $name } (palett)
help-keys-command = Kommando
help-keys-keys = Tangenter
help-commands = Alla kommandon finns i kommandopaletten ({ $key }). Skriv en del av namnet.
help-commands-no-key = Alla kommandon finns i kommandopaletten. Skriv en del av namnet.
help-link = Gå till { $title }
help-open = Hjälp
# Panelens namn för skärmläsare; { $page } är sidan som visas.
help-label = Hjälp: { $page }
help-disabled-why = Varför?

## Kraschrapport som erbjuds vid nästa start

crash-title = { -app-name } avslutades oväntat
crash-body = En rapport sparades på den här datorn. Inget skickades.
crash-view = Visa rapport
crash-hide = Dölj rapport
crash-copy = Kopiera
crash-copied = Rapporten kopierad
crash-save-bundle = Spara diagnostikpaket…
crash-dismiss = Stäng

## Diagnostikpaket

diag-title = Spara diagnostikpaket
diag-intro = En zip-fil att bifoga till en felrapport. Den sparas på den här datorn. Inget skickas.
diag-contents = Innehåll
diag-include-names = Ta med filnamn
diag-include-names-help = Av: namn ersätts med file-1.jpg, dir-1 och så vidare.
diag-privacy = Din hemmapp visas som ~ och ditt användarnamn som <user>. Lösenord, serveradresser och bilder tas aldrig med.
diag-where = Rapportera ett problem: { $issues } · { $email }
diag-save = Spara…
diag-cancel = Avbryt
diag-saved = Diagnostikpaket sparat: { $path }
diag-failed = Kunde inte spara diagnostikpaketet: { $reason }
diag-entry-summary = Version, system, GPU och drivrutin
diag-entry-crash = Senaste kraschrapport
diag-entry-crashes = Tidigare kraschrapporter ({ $count })
diag-entry-log = Sista { $count } loggraderna
diag-entry-settings = Inställningar, utan lösenord och adresser
diag-entry-gpu = GPU- och skärminformation
diag-entry-readme = Vad paketet är
diag-size = { $size }
# README.txt i paketet.
diag-readme =
    Diagnostikpaket för { -app-name }.

    Det innehåller det som hjälper till att hitta ett problem: version och systeminformation, kraschrapporter, slutet av loggen och inställningarna.

    Din hemmapp visas som ~ och ditt användarnamn som <user>. Om du inte valde att ta med dem ersätts fil- och mappnamn med file-1, dir-1 och så vidare. Lösenord och serveradresser tas bort.

    Inget skickades. Du bestämmer om du vill dela filen.

## Uppdateringar (Inställningar)

settings-section-updates = Uppdateringar
settings-updates-check = Sök efter uppdateringar
settings-updates-check-help = En gång om dagen, fråga om det finns en ny version. Begäran innehåller bara versionsnumret: ingen identifierare, ingen cookie. Av: { -app-name } ansluter aldrig för uppdateringar.
settings-updates-channel = Kanal
settings-updates-channel-help = Stabil: bara utgåvor. Beta: även testversioner.
settings-updates-channel-stable = Stabil
settings-updates-channel-beta = Beta
settings-updates-check-now = Sök nu
settings-updates-check-now-help = Sök en gång nu.
update-available = Version { $version } finns
update-notes = Versionsinformation
update-download = Hämta
update-none = { -app-name } är uppdaterat.
update-checking = Söker efter uppdateringar…
update-failed = Kunde inte söka efter uppdateringar: { $reason }
update-last-checked = Senast kontrollerat { $when }

## Om

about-build = Bygge { $commit } · { $profile } · { $target }
about-build-no-commit = Bygge { $profile } · { $target }
about-help = Hjälp
about-diagnostics = Spara diagnostikpaket…
# machine-drafted, needs native review
about-renderer = Renderarinfo…
about-licences-count = { $count ->
    [one] 1 komponent från tredje part
   *[other] { $count } komponenter från tredje part
}
about-licence-group = { $licence } ({ $count })
about-contact = Ärenden: { $issues } · Support: { $email }
