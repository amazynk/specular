# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Português (Portugal). machine-drafted, needs native review

palette-tether = Captura ligada…
palette-tether-capture = Tirar uma fotografia (ligada)

tether-title = Captura ligada
tether-close = Fechar a captura ligada
tether-default-folder = Captura ligada

## Cameras
tether-none = Nenhuma câmara encontrada.
tether-none-hint = Ligue a câmara pelo cabo USB e ligue-a. Se tiver modo USB, escolha PTP ou ligação ao PC.
tether-look-again = Procurar novamente
tether-connect = Ligar
tether-next-camera = Câmara seguinte
tether-connecting = A ligar…
tether-connected = { $camera } ligada
tether-disconnect = Desligar
tether-try-again = Tentar novamente
tether-gone = A câmara foi desligada.
tether-not-connected = Nenhuma câmara ligada. Abra primeiro a captura ligada.

## Why a camera can't be opened
tether-permission = Este utilizador não pode abrir o dispositivo USB da câmara { $node }.
tether-permission-rule = Uma regra do udev permite-o. Como administrador, coloque esta linha em { $file }:
tether-permission-reload = Depois execute "sudo udevadm control --reload" e volte a ligar a câmara.
tether-copy-rule = Copiar regra
tether-busy-other = Outro programa está a usar a câmara.
tether-busy-named = A câmara está a ser usada por { $programs }.
tether-busy-hint = Feche esse programa ou desmonte a câmara no gestor de ficheiros e tente novamente.
tether-driver = O controlador do sistema desta câmara não deixa outros programas usá-la.

## The camera
tether-battery = Bateria { PERCENT($percent) }
tether-card = { $card }: { $free } livres
tether-card-shots = { $card }: { $free } livres, { $shots } fotografias
tether-no-card = Sem cartão de memória.
tether-no-settings = A câmara não indica as definições de exposição.
tether-no-remote = Esta câmara não pode ser disparada daqui. As fotografias tiradas com o botão do obturador são transferidas.
tether-capture = Disparar
tether-step-down = { $setting }: diminuir ({ $value })
tether-step-up = { $setting }: aumentar ({ $value })

## Exposure settings
tether-prop-mode = Modo
tether-prop-shutter = Velocidade
tether-prop-aperture = Abertura
tether-prop-iso = ISO
tether-prop-bias = Compensação da exposição
tether-prop-white-balance = Equilíbrio de brancos
tether-auto = Automático
tether-bulb = Pose B
tether-camera-value = Definição da câmara { $value }
tether-wb-manual = Manual
tether-wb-one-push = Medição única
tether-wb-daylight = Luz do dia
tether-wb-fluorescent = Fluorescente
tether-wb-tungsten = Tungsténio
tether-wb-flash = Flash
tether-mode-manual = Manual
tether-mode-program = Programa
tether-mode-aperture = Prioridade à abertura
tether-mode-shutter = Prioridade à velocidade
tether-mode-creative = Criativo
tether-mode-action = Desporto
tether-mode-portrait = Retrato

## Where shots go
tether-folder = Pasta
tether-names = Nomes
tether-names-camera = Os nomes da câmara
tether-arrival = Estilo do Develop
tether-choose-folder = Pasta…
tether-choose-style = Estilo…
tether-downloading = A transferir… { PERCENT($percent) }
tether-saved = { $name } guardada
tether-saved-count = { $count ->
    [one] { $count } fotografia guardada
   *[other] { $count } fotografias guardadas
}

## Failures
tether-busy = A câmara está ocupada. Tente novamente.
tether-card-full = O cartão de memória está cheio.
tether-set-refused = A câmara não aceitou essa definição. Verifique o seletor de modos.
tether-disk = Não foi possível guardar a fotografia: { $error }
tether-error = Erro da câmara: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Pasta da captura ligada
settings-cull-tether-folder-help = Onde são guardadas as fotografias de uma câmara ligada por cabo.
settings-cull-tether-folder-unset = Imagens › Captura ligada
settings-cull-tether-rename = Nomes dos ficheiros da captura ligada
settings-cull-tether-rename-help = Nomes novos, com os mesmos códigos de Mudar o nome em Importar. Vazio mantém os nomes da câmara.
