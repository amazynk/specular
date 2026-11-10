# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Português (Brasil). machine-drafted, needs native review

palette-tether = Captura conectada…
palette-tether-capture = Tirar uma foto (conectada)

tether-title = Captura conectada
tether-close = Fechar a captura conectada
tether-default-folder = Captura conectada

## Cameras
tether-none = Nenhuma câmera encontrada.
tether-none-hint = Conecte a câmera pelo cabo USB e ligue-a. Se ela tiver modo USB, escolha PTP ou conexão com PC.
tether-look-again = Procurar de novo
tether-connect = Conectar
tether-next-camera = Próxima câmera
tether-connecting = Conectando…
tether-connected = { $camera } conectada
tether-disconnect = Desconectar
tether-try-again = Tentar de novo
tether-gone = A câmera foi desconectada.
tether-not-connected = Nenhuma câmera conectada. Abra a captura conectada primeiro.

## Why a camera can't be opened
tether-permission = Este usuário não pode abrir o dispositivo USB da câmera { $node }.
tether-permission-rule = Uma regra do udev permite isso. Como administrador, coloque esta linha em { $file }:
tether-permission-reload = Depois execute "sudo udevadm control --reload" e conecte a câmera de novo.
tether-copy-rule = Copiar regra
tether-busy-other = Outro programa está usando a câmera.
tether-busy-named = A câmera está em uso por { $programs }.
tether-busy-hint = Feche esse programa ou desmonte a câmera no gerenciador de arquivos e tente de novo.
tether-driver = O driver do sistema para esta câmera não deixa outros programas usá-la.

## The camera
tether-battery = Bateria { PERCENT($percent) }
tether-card = { $card }: { $free } livres
tether-card-shots = { $card }: { $free } livres, { $shots } fotos
tether-no-card = Sem cartão de memória.
tether-no-settings = A câmera não informa as configurações de exposição.
tether-no-remote = Esta câmera não pode ser disparada daqui. As fotos tiradas com o botão do obturador dela são baixadas.
tether-capture = Disparar
tether-step-down = { $setting }: diminuir ({ $value })
tether-step-up = { $setting }: aumentar ({ $value })

## Exposure settings
tether-prop-mode = Modo
tether-prop-shutter = Velocidade
tether-prop-aperture = Abertura
tether-prop-iso = ISO
tether-prop-bias = Compensação de exposição
tether-prop-white-balance = Balanço de branco
tether-auto = Automático
tether-bulb = Bulbo (B)
tether-camera-value = Configuração da câmera { $value }
tether-wb-manual = Manual
tether-wb-one-push = Medição única
tether-wb-daylight = Luz do dia
tether-wb-fluorescent = Fluorescente
tether-wb-tungsten = Tungstênio
tether-wb-flash = Flash
tether-mode-manual = Manual
tether-mode-program = Programa
tether-mode-aperture = Prioridade de abertura
tether-mode-shutter = Prioridade de velocidade
tether-mode-creative = Criativo
tether-mode-action = Esportes
tether-mode-portrait = Retrato

## Where shots go
tether-folder = Pasta
tether-names = Nomes
tether-names-camera = Os nomes da câmera
tether-arrival = Estilo do Develop
tether-choose-folder = Pasta…
tether-choose-style = Estilo…
tether-downloading = Baixando… { PERCENT($percent) }
tether-saved = { $name } salva
tether-saved-count = { $count ->
    [one] { $count } foto salva
   *[other] { $count } fotos salvas
}

## Failures
tether-busy = A câmera está ocupada. Tente de novo.
tether-card-full = O cartão de memória está cheio.
tether-set-refused = A câmera não aceitou essa configuração. Verifique o seletor de modos.
tether-disk = Não foi possível salvar a foto: { $error }
tether-error = Erro da câmera: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Pasta da captura conectada
settings-cull-tether-folder-help = Onde as fotos de uma câmera conectada por cabo são salvas.
settings-cull-tether-folder-unset = Imagens › Captura conectada
settings-cull-tether-rename = Nomes de arquivo da captura conectada
settings-cull-tether-rename-help = Nomes novos, com os mesmos códigos de Renomear em Importar. Vazio mantém os nomes da câmera.
