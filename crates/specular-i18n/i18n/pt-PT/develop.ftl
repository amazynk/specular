# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Português (Portugal).

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Um módulo desta edição foi atualizado para uma versão mais recente. O resultado pode ficar ligeiramente diferente.
    [many] { $count } módulos desta edição foram atualizados para uma versão mais recente. O resultado pode ficar ligeiramente diferente.
   *[other] { $count } módulos desta edição foram atualizados para uma versão mais recente. O resultado pode ficar ligeiramente diferente.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Um módulo desta edição requer uma versão mais recente do Specular. É mantido, mas não é mostrado.
    [many] { $count } módulos desta edição requerem uma versão mais recente do Specular. São mantidos, mas não são mostrados.
   *[other] { $count } módulos desta edição requerem uma versão mais recente do Specular. São mantidos, mas não são mostrados.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Ponto { $index } de { $count }: entrada { $input }, saída { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = As setas movem o ponto. Page Up e Page Down escolhem outro. Insert adiciona um ponto, Delete remove-o.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutro
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Tom { $hue }°, intensidade { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = As setas movem a cor. Delete repõe-na.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Revelação
# machine-drafted, needs native review
develop-undo = Anular
# machine-drafted, needs native review
develop-redo = Refazer
# machine-drafted, needs native review
develop-before-after = Antes e depois
# machine-drafted, needs native review
develop-close = Sair da Revelação
# machine-drafted, needs native review
develop-history = Histórico
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } ativado
develop-card-menu = Opções de { $module }
develop-card-changed = Alterado
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Tira de filme

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Exposição
develop-module-light = Luz
develop-look-profile = Perfil
develop-look-standard = Padrão
develop-look-film = Película
develop-look-natural = Natural
develop-module-grading = Gradação de cor
develop-add-tool = Adicionar uma ferramenta
# machine-drafted, needs native review
develop-module-white-balance = Equilíbrio de brancos
# machine-drafted, needs native review
develop-module-tone = Tom
# machine-drafted, needs native review
develop-module-contrast = Contraste e gama
# machine-drafted, needs native review
develop-module-colour = Cor
# machine-drafted, needs native review
develop-module-curve = Curva
# machine-drafted, needs native review
develop-module-crop = Recortar e endireitar
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Outro módulo
# machine-drafted, needs native review
develop-exposure = Exposição
# machine-drafted, needs native review
develop-black-level = Nível de preto
# machine-drafted, needs native review
develop-temperature = Temperatura
# machine-drafted, needs native review
develop-tint = Matiz
# machine-drafted, needs native review
develop-highlights = Realces
# machine-drafted, needs native review
develop-shadows = Sombras
# machine-drafted, needs native review
develop-contrast = Contraste
# machine-drafted, needs native review
develop-whites = Brancos
# machine-drafted, needs native review
develop-blacks = Pretos
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = Limite HDR
# machine-drafted, needs native review
develop-hdr-preview = Mostrar
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Vibração
# machine-drafted, needs native review
develop-saturation = Saturação
# machine-drafted, needs native review
develop-colour-wheels = Rodas de cor
# machine-drafted, needs native review
develop-curve = Curva de tons
# machine-drafted, needs native review
develop-module-local-contrast = Contraste local
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Exposição automática
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Suavização
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Textura
# machine-drafted, needs native review
develop-clarity = Claridade
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Ponto preto
# machine-drafted, needs native review
develop-levels-mid = Ponto médio
# machine-drafted, needs native review
develop-levels-white = Ponto branco
# machine-drafted, needs native review
develop-module-detail = Detalhe
# machine-drafted, needs native review
develop-module-denoise = Redução de ruído
# machine-drafted, needs native review
develop-module-sharpen = Nitidez
# machine-drafted, needs native review
develop-noise-luminance = Ruído de luminância
# machine-drafted, needs native review
develop-noise-detail = Detalhe do ruído
# machine-drafted, needs native review
develop-noise-colour = Ruído de cor
# machine-drafted, needs native review
develop-sharpen-amount = Intensidade da nitidez
# machine-drafted, needs native review
develop-sharpen-radius = Raio da nitidez
# machine-drafted, needs native review
develop-sharpen-detail = Detalhe da nitidez
# machine-drafted, needs native review
develop-sharpen-masking = Máscara
# machine-drafted, needs native review
develop-capture-amount = Nitidez de captura
# machine-drafted, needs native review
develop-capture-radius = Raio de captura
# machine-drafted, needs native review
develop-straighten = Endireitar
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Recortar
# machine-drafted, needs native review
develop-crop-turn = Rodar
# machine-drafted, needs native review
develop-crop-flip = Inverter
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Correções da objetiva
develop-module-optics = Ótica
# machine-drafted, needs native review
develop-lens-distortion = Distorção
# machine-drafted, needs native review
develop-lens-vignette = Vinhetagem
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Franja vermelha
# machine-drafted, needs native review
develop-lens-tca-blue = Franja azul
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Perfil da objetiva
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automático
# machine-drafted, needs native review
develop-lensfun-off = Nenhum
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Nenhum perfil da objetiva.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Nenhum perfil Lensfun para { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Nenhum perfil Lensfun para esta objetiva.
# machine-drafted, needs native review
develop-lensfun-no-name = O ficheiro não indica a objetiva. Escolha-a acima.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } objetivas correspondem. Escolha uma acima.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Recortar. Arraste os limites, ou Ctrl+arraste ao longo de uma linha que deva ficar nivelada. Enter mantém, Esc volta atrás.
# machine-drafted, needs native review
develop-crop-kept = Recorte mantido
# machine-drafted, needs native review
develop-crop-cancelled = Recorte anulado
# machine-drafted, needs native review
develop-crop-reset = Recorte e endireitamento repostos
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Endireitada em { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Proporção { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = livre
# machine-drafted, needs native review
develop-crop-aspect-original = original
# machine-drafted, needs native review
develop-crop-turned-right = Rodada para a direita
# machine-drafted, needs native review
develop-crop-turned-left = Rodada para a esquerda
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Invertida da esquerda para a direita
# machine-drafted, needs native review
develop-crop-flipped-vertical = Invertida de cima para baixo
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Proporção { $aspect } · Guia: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A proporção · X trocar · O guia · [ ] rodar · H V inverter · Enter manter · Esc voltar
# machine-drafted, needs native review
develop-crop-guide-thirds = terços
# machine-drafted, needs native review
develop-crop-guide-golden = proporção áurea
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonais
# machine-drafted, needs native review
develop-crop-guide-grid = grelha

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspectiva
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Prumo
# machine-drafted, needs native review
develop-persp-mode-off = Desligado
# machine-drafted, needs native review
develop-persp-mode-auto = Automático
# machine-drafted, needs native review
develop-persp-mode-level = Nível
# machine-drafted, needs native review
develop-persp-mode-vertical = Verticais
# machine-drafted, needs native review
develop-persp-mode-full = Completo
# machine-drafted, needs native review
develop-persp-mode-guided = Guiado
# machine-drafted, needs native review
develop-persp-vertical = Perspectiva vertical
# machine-drafted, needs native review
develop-persp-horizontal = Perspectiva horizontal
# machine-drafted, needs native review
develop-persp-rotate = Rodar
# machine-drafted, needs native review
develop-persp-aspect = Proporção
# machine-drafted, needs native review
develop-persp-scale = Escala
# machine-drafted, needs native review
develop-persp-offset-x = Lateral
# machine-drafted, needs native review
develop-persp-offset-y = Para cima ou para baixo
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Evitar margens vazias
# machine-drafted, needs native review
develop-persp-guides = Guias
# machine-drafted, needs native review
develop-persp-focal = Distância focal
# machine-drafted, needs native review
develop-persp-draw-guides = Desenhar guias
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Desenhar guias ({ $n } de { $max })
# machine-drafted, needs native review
develop-persp-reset = Repor perspectiva
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Prumo: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = A procurar linhas…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Repor a perspectiva? As suas { $n } guias são removidas. Anular repõe-nas.
# machine-drafted, needs native review
develop-persp-reset-ok = Repor
# machine-drafted, needs native review
develop-persp-reset-done = Perspectiva reposta
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Remover a guia { $n }? Anular repõe-na.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Remover
# machine-drafted, needs native review
develop-persp-guide-removed = Guia removida
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Perspectiva vertical { $vertical }, horizontal { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Falha no prumo: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: inclinação { $vertical }°, viragem { $horizontal }°, rotação { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } precisa de pelo menos duas linhas retas distintas. Experimente outro modo ou desenhe guias.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = No máximo { $n } guias. Arraste uma ponta para mover uma.
# machine-drafted, needs native review
develop-persp-guides-on = Guias. Arraste ao longo de uma linha que deva ficar vertical ou nivelada.
# machine-drafted, needs native review
develop-persp-guides-off = Recortar
# machine-drafted, needs native review
develop-persp-guide-moved = Guia movida
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Guia { $n } adicionada
# machine-drafted, needs native review
develop-persp-guides-keys = Arrastar adiciona · Arrastar uma ponta move · Delete remove · Shift+R recortar
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Guias: { $n } de { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Sombras
# machine-drafted, needs native review
develop-wheel-midtones = Tons médios
# machine-drafted, needs native review
develop-wheel-highlights = Realces
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminância das sombras
# machine-drafted, needs native review
develop-luminance-midtones = Luminância dos tons médios
# machine-drafted, needs native review
develop-luminance-highlights = Luminância dos realces
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Global
# machine-drafted, needs native review
develop-luminance-global = Luminância global
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Intensidade da cor
# machine-drafted, needs native review
develop-brilliance = Brilho da cor

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Misturador de cores
# machine-drafted, needs native review
develop-colour-red = Vermelho
# machine-drafted, needs native review
develop-colour-orange = Laranja
# machine-drafted, needs native review
develop-colour-yellow = Amarelo
# machine-drafted, needs native review
develop-colour-green = Verde
# machine-drafted, needs native review
develop-colour-aqua = Água-marinha
# machine-drafted, needs native review
develop-colour-blue = Azul
# machine-drafted, needs native review
develop-colour-purple = Roxo
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Matiz – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Saturação – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminância – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Cor pontual
# machine-drafted, needs native review
develop-point-pick = Escolher uma cor
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Escolher outra cor
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Clique na cor que pretende alterar. Esc cancela.
# machine-drafted, needs native review
develop-point-picked = Cor escolhida
# machine-drafted, needs native review
develop-point-cancelled = Escolha cancelada
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Intervalo
# machine-drafted, needs native review
develop-point-hue-range = Intervalo de matiz
# machine-drafted, needs native review
develop-point-saturation-range = Intervalo de saturação
# machine-drafted, needs native review
develop-point-lightness-range = Intervalo de luminosidade
# machine-drafted, needs native review
develop-point-hue = Deslocamento de matiz
# machine-drafted, needs native review
develop-point-saturation = Deslocamento de saturação
# machine-drafted, needs native review
develop-point-lightness = Deslocamento de luminosidade

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Preto e branco
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminância – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Escolher um ficheiro LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Ficheiro: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Não é possível ler { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Escolher um ficheiro LUT
# machine-drafted, needs native review
develop-lut-kind = Ficheiros LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Feita para
# machine-drafted, needs native review
develop-lut-space-linear = Linear (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES linear (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Fotografia (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Vídeo (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Método
# machine-drafted, needs native review
develop-lut-tetrahedral = Preciso
# machine-drafted, needs native review
develop-lut-trilinear = Simples
# machine-drafted, needs native review
develop-lut-strength = Intensidade

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Original
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } ativado
# machine-drafted, needs native review
develop-history-off = { $module } desativado
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = De volta ao original
# machine-drafted, needs native review
develop-at-step = De volta a: { $step }
# machine-drafted, needs native review
develop-undone = Anulado: { $step }
# machine-drafted, needs native review
develop-redone = Refeito: { $step }
# machine-drafted, needs native review
develop-entered = Revelação. As edições nunca alteram o ficheiro.
# machine-drafted, needs native review
develop-left = Saiu da Revelação
# machine-drafted, needs native review
develop-split-on = Antes e depois lado a lado
# machine-drafted, needs native review
develop-split-off = Só a edição
# machine-drafted, needs native review
develop-showing-before = A mostrar o original
# machine-drafted, needs native review
develop-showing-after = A mostrar a edição
# machine-drafted, needs native review
# Labels over the image.
develop-before = Antes
# machine-drafted, needs native review
develop-after = Depois
# machine-drafted, needs native review
develop-working = A revelar…
# machine-drafted, needs native review
develop-preview-failed = Não é possível mostrar a edição deste ficheiro

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Revelação
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Edição rápida: manter premido e deslocar
# machine-drafted, needs native review
palette-develop = Revelação
# machine-drafted, needs native review
palette-develop-help = Editar esta fotografia sem alterar o ficheiro. De novo para sair.
# machine-drafted, needs native review
palette-develop-before-after = Antes e depois
# machine-drafted, needs native review
palette-develop-before-after-help = Mostrar o original, ou de novo a edição.
# machine-drafted, needs native review
palette-develop-split = Antes e depois divididos
# machine-drafted, needs native review
palette-develop-split-help = Mostrar o original e a edição lado a lado.
# machine-drafted, needs native review
palette-develop-panel = Ir para o painel Revelação
# machine-drafted, needs native review
palette-develop-panel-help = Levar o teclado para o painel. Esc volta.
# machine-drafted, needs native review
palette-develop-speed-exposure = Edição rápida da exposição
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Manter premido e deslocar ou premir as setas para mudar a exposição.
# machine-drafted, needs native review
palette-develop-speed-contrast = Edição rápida do contraste
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Manter premido e deslocar ou premir as setas para mudar o contraste.
# machine-drafted, needs native review
palette-develop-speed-highlights = Edição rápida dos realces
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Manter premido e deslocar ou premir as setas para mudar os realces.
# machine-drafted, needs native review
palette-develop-speed-shadows = Edição rápida das sombras
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Manter premido e deslocar ou premir as setas para mudar as sombras.
# machine-drafted, needs native review
palette-develop-speed-whites = Edição rápida dos brancos
# machine-drafted, needs native review
palette-develop-speed-whites-help = Manter premido e deslocar ou premir as setas para mudar os brancos.
# machine-drafted, needs native review
palette-develop-speed-blacks = Edição rápida dos pretos
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Manter premido e deslocar ou premir as setas para mudar os pretos.
# machine-drafted, needs native review
palette-develop-speed-temperature = Edição rápida da temperatura
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Manter premido e deslocar ou premir as setas para mudar a temperatura do equilíbrio de brancos.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Edição rápida da vibração
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Manter premido e deslocar ou premir as setas para mudar a vibração.
# machine-drafted, needs native review
palette-develop-crop = Recortar e endireitar
# machine-drafted, needs native review
palette-develop-crop-help = Recorte, endireite, rode ou inverta a fotografia. Enter mantém, Esc volta atrás.
# machine-drafted, needs native review
disabled-not-photo = Só para fotografias
# machine-drafted, needs native review
disabled-develop-off = A Revelação está desativada nos perfis Medicina e Ciência
# machine-drafted, needs native review
develop-module-corrections = Correções
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Desligado
# machine-drafted, needs native review
develop-on = Ligado
# machine-drafted, needs native review
develop-highlight-method = Altas luzes queimadas
# machine-drafted, needs native review
develop-highlight-opposed = Reconstruir cor
# machine-drafted, needs native review
develop-highlight-rebuild = Reconstruir áreas grandes
# machine-drafted, needs native review
develop-highlight-clip = Cortar para branco
# machine-drafted, needs native review
develop-highlight-threshold = Nível de corte
# machine-drafted, needs native review
develop-module-hot-pixels = Píxeis quentes
# machine-drafted, needs native review
develop-hot-threshold = Limiar de píxeis quentes
# machine-drafted, needs native review
develop-module-ca = Aberração cromática
# machine-drafted, needs native review
develop-ca-mode = Franjas
# machine-drafted, needs native review
develop-ca-auto = Automática
# machine-drafted, needs native review
develop-ca-manual = Manual
# machine-drafted, needs native review
develop-ca-red = Desvio do vermelho
# machine-drafted, needs native review
develop-ca-blue = Desvio do azul
# machine-drafted, needs native review
develop-module-defringe = Remover franjas
# machine-drafted, needs native review
develop-defringe = Remover franjas
# machine-drafted, needs native review
develop-defringe-purple = Matiz roxo
# machine-drafted, needs native review
develop-defringe-green = Matiz verde
# machine-drafted, needs native review
develop-defringe-range = Intervalo de matiz
# machine-drafted, needs native review
develop-defringe-edge = Contraste de contorno
# machine-drafted, needs native review
develop-module-flat-field = Fotogramas de calibração
# machine-drafted, needs native review
develop-calibration = Calibração
# machine-drafted, needs native review
develop-calibration-auto = Da pasta
# machine-drafted, needs native review
develop-calibration-chosen = Ficheiros escolhidos
# machine-drafted, needs native review
develop-flat-choose = Escolher um flat…
# machine-drafted, needs native review
develop-dark-choose = Escolher um dark…
# machine-drafted, needs native review
develop-flat-file = Fotograma flat: { $name }
# machine-drafted, needs native review
develop-dark-file = Fotograma dark: { $name }
# machine-drafted, needs native review
develop-flat-use = Fotograma flat
# machine-drafted, needs native review
develop-flat-use-full = Vinhetagem e pó
# machine-drafted, needs native review
develop-flat-use-vignetting = Só vinhetagem
# machine-drafted, needs native review
develop-flat-dialog = Escolher um flat
# machine-drafted, needs native review
develop-dark-dialog = Escolher um dark
# machine-drafted, needs native review
develop-raw-kind = Ficheiros raw
# machine-drafted, needs native review
develop-flat-frame = Fotograma flat
# machine-drafted, needs native review
develop-dark-frame = Fotograma dark

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Adicionar máscara
# machine-drafted, needs native review
develop-mask-kind-ellipse = Elipse
# machine-drafted, needs native review
develop-mask-kind-rectangle = Retângulo
# machine-drafted, needs native review
develop-mask-kind-polygon = Polígono
# machine-drafted, needs native review
develop-mask-kind-linear = Gradiente linear
# machine-drafted, needs native review
develop-mask-kind-radial = Gradiente radial
# machine-drafted, needs native review
develop-mask-kind-brush = Pincel
# machine-drafted, needs native review
develop-mask-kind-luminance = Intervalo de luminância
# machine-drafted, needs native review
develop-mask-kind-hue = Intervalo de matiz
# machine-drafted, needs native review
develop-mask-kind-chroma = Intervalo de saturação
# machine-drafted, needs native review
develop-mask-kind-colour = Intervalo de cor
# machine-drafted, needs native review
develop-mask-kind-other = Outra máscara
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Mistura
# machine-drafted, needs native review
develop-blend-normal = Normal
# machine-drafted, needs native review
develop-blend-multiply = Multiplicar
# machine-drafted, needs native review
develop-blend-screen = Ecrã
# machine-drafted, needs native review
develop-blend-overlay = Sobrepor
# machine-drafted, needs native review
develop-blend-soft-light = Luz suave
# machine-drafted, needs native review
develop-blend-lightness = Só luminosidade
# machine-drafted, needs native review
develop-blend-colour = Só cor
# machine-drafted, needs native review
develop-mask-amount = Intensidade da máscara
# machine-drafted, needs native review
develop-mask-invert = Inverter máscara
# machine-drafted, needs native review
develop-mask-off = Desligado
# machine-drafted, needs native review
develop-mask-on = Ligado
# machine-drafted, needs native review
develop-mask-refine = Refinar contornos
# machine-drafted, needs native review
develop-mask-show = Mostrar máscara
# machine-drafted, needs native review
develop-mask-show-red = Vermelho
# machine-drafted, needs native review
develop-mask-show-grey = Cinzento
# machine-drafted, needs native review
develop-mask-remove = Remover máscara
# machine-drafted, needs native review
develop-mask-combine-add = Adicionar
# machine-drafted, needs native review
develop-mask-combine-subtract = Subtrair
# machine-drafted, needs native review
develop-mask-combine-intersect = Interseção
# machine-drafted, needs native review
develop-mask-row-opacity = Opacidade
# machine-drafted, needs native review
develop-mask-row-invert = Inverter
# machine-drafted, needs native review
develop-mask-feather = Esbatimento
# machine-drafted, needs native review
develop-mask-roundness = Arredondamento
# machine-drafted, needs native review
develop-mask-brush-size = Tamanho do pincel
# machine-drafted, needs native review
develop-mask-brush-feather = Esbatimento do pincel
# machine-drafted, needs native review
develop-mask-brush-flow = Fluxo
# machine-drafted, needs native review
develop-mask-brush-mode = Pincel
# machine-drafted, needs native review
develop-mask-brush-paint = Pintar
# machine-drafted, needs native review
develop-mask-brush-erase = Apagar
# machine-drafted, needs native review
develop-mask-range-from = De
# machine-drafted, needs native review
develop-mask-range-full-from = Pleno a partir de
# machine-drafted, needs native review
develop-mask-range-full-to = Pleno até
# machine-drafted, needs native review
develop-mask-range-to = Até
# machine-drafted, needs native review
develop-mask-range-test = Testar
# machine-drafted, needs native review
develop-mask-range-input = Entrada
# machine-drafted, needs native review
develop-mask-range-output = Saída
# machine-drafted, needs native review
develop-mask-colour-pick = Escolher uma cor
# machine-drafted, needs native review
develop-mask-colour-pick-more = Escolher outra cor
# machine-drafted, needs native review
develop-mask-colour-range = Intervalo
# machine-drafted, needs native review
develop-mask-colour-falloff = Atenuação
# machine-drafted, needs native review
develop-mask-edit = Editar na imagem
# machine-drafted, needs native review
develop-mask-row-remove = Remover { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Remover a máscara de { $module }? As suas formas desaparecem. A fotografia no disco fica.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Remover { $name } de { $module }? As outras máscaras e a fotografia no disco ficam.
# machine-drafted, needs native review
develop-mask-remove-ok = Remover
# machine-drafted, needs native review
develop-mask-removed = Máscara removida.
# machine-drafted, needs native review
develop-mask-added = { $name } adicionada.
# machine-drafted, needs native review
develop-mask-full = Esta máscara já tem todas as partes que comporta.
# machine-drafted, needs native review
develop-mask-editing = A editar { $name }. Arraste as pegas, ou use Tab e as setas.
# machine-drafted, needs native review
develop-mask-painting = A pintar { $name }. Arraste sobre a imagem.
# machine-drafted, needs native review
develop-mask-picking = Clique na imagem para escolher uma cor.
# machine-drafted, needs native review
develop-mask-picked = Cor escolhida.
# machine-drafted, needs native review
develop-mask-done = Edição da máscara terminada.
# machine-drafted, needs native review
develop-mask-handles = Pegas da máscara
# machine-drafted, needs native review
develop-handle-centre = Centro
# machine-drafted, needs native review
develop-handle-width = Largura
# machine-drafted, needs native review
develop-handle-height = Altura
# machine-drafted, needs native review
develop-handle-rotate = Rodar
# machine-drafted, needs native review
develop-handle-feather = Esbatimento
# machine-drafted, needs native review
develop-handle-inner = Contorno interior
# machine-drafted, needs native review
develop-handle-start = Início
# machine-drafted, needs native review
develop-handle-end = Fim
# machine-drafted, needs native review
develop-handle-point = Ponto { $n }
# machine-drafted, needs native review
develop-instance = Camada
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Camada { $n }.
# machine-drafted, needs native review
develop-instance-new = Nova camada
# machine-drafted, needs native review
develop-instance-duplicate = Duplicar camada
# machine-drafted, needs native review
develop-instance-delete = Eliminar camada
# machine-drafted, needs native review
develop-instance-earlier = Mover para antes
# machine-drafted, needs native review
develop-instance-later = Mover para depois
# machine-drafted, needs native review
develop-instance-delete-confirm = Eliminar a camada { $n } de { $module }? As suas definições e máscaras desaparecem. A fotografia no disco fica.
# machine-drafted, needs native review
develop-instance-delete-ok = Eliminar
# machine-drafted, needs native review
develop-instance-deleted = Camada eliminada.
# machine-drafted, needs native review
develop-history-mask = Máscara: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Formas da máscara: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Novo: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Eliminado: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Ordem: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Outros programas
# machine-drafted, needs native review
develop-darktable-edited = Editada no darktable:
# machine-drafted, needs native review
develop-darktable-open = Abrir no darktable
# machine-drafted, needs native review
develop-darktable-opening = A abrir no darktable
# machine-drafted, needs native review
develop-darktable-failed = O darktable não arrancou: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importar do Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] 1 definição do Lightroom importada. O resultado é aproximado.
    [many] { $count } definições do Lightroom importadas. O resultado é aproximado.
   *[other] { $count } definições do Lightroom importadas. O resultado é aproximado.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Nada a importar: estas definições do Lightroom não mudam nada aqui.
# machine-drafted, needs native review
develop-history-lightroom = Importado do Lightroom (aproximado)
# machine-drafted, needs native review
develop-conflict-title = A edição de { $file } mudou fora do Specular
# machine-drafted, needs native review
develop-conflict-file = Ficheiro auxiliar: guardado { $time }
# machine-drafted, needs native review
develop-conflict-library = Biblioteca: guardada { $time }
# machine-drafted, needs native review
develop-conflict-writer = O ficheiro auxiliar foi escrito por { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Fique com uma. A outra é substituída.
# machine-drafted, needs native review
develop-conflict-use-file = Usar a do ficheiro
# machine-drafted, needs native review
develop-conflict-keep-library = Manter a da biblioteca
# machine-drafted, needs native review
develop-conflict-used-file = A usar a edição do ficheiro auxiliar
# machine-drafted, needs native review
develop-conflict-kept-library = A manter a edição da biblioteca
# machine-drafted, needs native review
toast-develop-edit-newer-process = Esta edição foi feita num Specular mais recente. Pode parecer diferente aqui.
# machine-drafted, needs native review
info-darktable-edited = Editada no darktable
# machine-drafted, needs native review
info-darktable-no-modules = Nenhum módulo ativo
# machine-drafted, needs native review
info-darktable-group = Ficheiro
# machine-drafted, needs native review
toast-develop-edit-partial = Esta edição foi feita num Specular mais recente. As partes que esta versão não mostra ficam de fora. As alterações aqui não são guardadas.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = O ficheiro de edição ao lado desta foto não pode ser lido. Fica como está. As alterações aqui não são guardadas.
# machine-drafted, needs native review
develop-demosaic = Detalhes raw
develop-demosaic-directional = Padrão
develop-demosaic-dual = Suavizar áreas lisas
develop-demosaic-fast = Rápido
develop-dual-threshold = Contraste das áreas lisas
