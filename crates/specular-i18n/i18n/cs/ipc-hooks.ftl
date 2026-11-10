# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Dálkové ovládání a události
settings-integration-ipc-help = Jiné programy na tomto počítači spuštěné pod vaším účtem mohou přes soukromý socket (specular ipc) ovládat { -app-name } a dozvídat se, co se v něm děje. Na síti nic nenaslouchá.

## The hooks panel under "Remote control and events".
hooks-panel-title = Háčky
hooks-panel-intro = Programy, které settings.toml spouští, když se něco stane. Každý se jednou zeptá před prvním spuštěním.
hooks-panel-none = Žádné háčky. Přidejte je jako [[hooks]] do settings.toml.
hooks-panel-keywords = háčky hooky události automatizace skripty programy
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Když { $events }
hooks-panel-untrusted = Před prvním spuštěním se zeptá
hooks-panel-allowed = Povoleno
hooks-panel-remove = Odebrat…
hooks-panel-unused = Nepoužívá se

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = Spustit „{ $name }“ ({ $program }), když { $events }?
hook-consent-detail = Spustí tento program s názvy fotek: { $args }
hook-consent-trust = Povolujte jen programy, kterým důvěřujete. Když se háček změní, zeptá se znovu.
hook-consent-allow = Povolit
hook-consent-not-now = Teď ne
hook-or = { " " }nebo{ " " }
hook-event-opened = se otevře fotka
hook-event-marked = se označí fotky
hook-event-keywords = se změní klíčová slova
hook-event-exported = se exportují fotky
hook-event-imported = se importují fotky
hook-event-captured = fotoaparát pošle fotku
hook-event-develop-saved = se uloží úprava ve Vyvolání
hook-event-any = se cokoli stane

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = Odebrat háček „{ $name }“?
hook-remove-detail = Odstraní se ze settings.toml. Program a vaše soubory zůstanou na disku.
hook-remove-ok = Odebrat
hook-removed = Háček „{ $name }“ odebrán.
hook-settings-unwritable = settings.toml nelze změnit: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = Háček „{ $name }“ nefungoval: { $reason }
hook-failed-exit = skončil s kódem { $code }
hook-failed-timeout = byl zastaven po { $seconds } s
hook-failed-start = nepodařilo se ho spustit ({ $reason })
hook-dropped = Háček „{ $name }“ je zaneprázdněn: { $count ->
    [one] jedna událost byla přeskočena.
    [few] { $count } události byly přeskočeny.
    [many] { $count } události bylo přeskočeno.
   *[other] { $count } událostí bylo přeskočeno.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] soubor, který by se nahradil, existuje: nic nebylo exportováno
    [few] { $count } soubory, které by se nahradily, existují: nic nebylo exportováno
    [many] { $count } souboru, který by se nahradil, existuje: nic nebylo exportováno
   *[other] { $count } souborů, které by se nahradily, existuje: nic nebylo exportováno
}
export-recipes-remote-skipped = { $file } existuje
