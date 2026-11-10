# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Comparar com…
palette-compare-with-help = Comparar esta imagem com outra à sua escolha.
palette-compare-side = Comparar lado a lado
palette-compare-side-help = Mostrar as imagens comparadas uma ao lado da outra.
palette-compare-split = Comparar com divisão
palette-compare-split-help = Mostrar a segunda imagem sobre a primeira, cortada por um divisor que pode arrastar.
palette-compare-split-direction = Rodar o divisor
palette-compare-split-direction-help = Pôr o divisor na horizontal em vez da vertical, ou voltar.
palette-compare-overlay = Comparar em sobreposição
palette-compare-overlay-help = Mostrar a segunda imagem sobre a primeira, misturada.
palette-compare-difference = Comparar a diferença
palette-compare-difference-help = Mostrar onde as imagens diferem; de novo para o tipo seguinte.
palette-compare-flicker = Alternar entre as imagens comparadas
palette-compare-flicker-help = Mostrar a outra imagem de imediato.
palette-compare-auto-flicker = Alternância automática
palette-compare-auto-flicker-help = Alternar sozinho entre as duas imagens, ou parar.
palette-compare-align = Alinhar as imagens comparadas
palette-compare-align-help = Alinhar imagens de tamanhos diferentes: no canto superior esquerdo, centradas, redimensionadas ou automaticamente.
palette-compare-metric-space = Mudar o espaço de medida
palette-compare-metric-space-help = Medir em luz linear ou em sRGB codificado.
palette-compare-whole-image = Medir a imagem inteira
palette-compare-whole-image-help = Medir a imagem inteira em vez da parte visível, ou voltar.
palette-compare-probe = Sonda de píxeis da comparação
palette-compare-probe-help = Mostrar os valores das duas imagens sob o ponteiro.
palette-compare-link = Ligar ou desligar as imagens comparadas
palette-compare-link-help = Ampliar e deslocar cada uma das duas imagens à parte, ou de novo em conjunto.
palette-compare-close = Fechar a comparação
palette-compare-close-help = Voltar à galeria.
disabled-compare-closed = Abra primeiro a comparação
disabled-compare-not-two = Funciona com duas imagens

## Cheat sheet
cheatsheet-compare-split = Divisão ou lado a lado
cheatsheet-compare-split-direction = Divisor horizontal ou vertical
cheatsheet-compare-auto-flicker = Alternância automática
cheatsheet-compare-align = Alinhamento seguinte
cheatsheet-compare-space = Espaço de medida
cheatsheet-compare-whole = Imagem inteira ou parte visível
cheatsheet-compare-probe = Sonda de píxeis
cheatsheet-compare-link = Ligar ou desligar duas imagens

## Settings
settings-gallery-compare-metric-space = Espaço de medida da comparação
settings-gallery-compare-metric-space-help = Onde a comparação mede MAE, MSE, PSNR e SSIM. A luz linear é o que o ecrã mostra. O sRGB coincide com outras ferramentas em ficheiros de 8-bit.
settings-gallery-compare-metric-space-linear = Luz linear
settings-gallery-compare-metric-space-srgb = sRGB codificado
settings-gallery-compare-flicker-limit = Alternância automática mais rápida
settings-gallery-compare-flicker-limit-help = Mudanças de imagem por segundo, no máximo. Acima de 3, a intermitência pode provocar convulsões.

## Toolbar and captions
compare-split = Divisão
compare-split-down = Divisor vertical
compare-split-across = Divisor horizontal
compare-link = Ligadas
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } mudanças por segundo
compare-gain = Ganho ×{ $gain }
compare-probe = Sonda
compare-kind-signed = Erro com sinal
compare-kind-on-image = Erro sobre a imagem
compare-align-top-left = Canto superior esquerdo
compare-align-centre = Centrado
compare-align-scaled = B redimensionada para A
compare-align-auto = Alinhamento automático
compare-space-linear = Luz linear
compare-space-srgb = sRGB
compare-space-help-linear = Medido em luz linear, tal como apresentado após o mapeamento de tons. Aqui o PSNR não é o habitual PSNR sRGB de 8-bit.
compare-space-help-srgb = Medido nos valores sRGB codificados, como fazem outras ferramentas com ficheiros de 8-bit. O HDR acima do branco SDR conta como branco.
compare-space-help-common = O ΔE2000 é sempre CIELAB (D65), com o branco SDR como L* 100 e o HDR cortado. Imagens com mais de { $size } píxeis são medidas numa cópia de { $size } píxeis.
compare-region-view = Parte visível
compare-region-whole = Imagem inteira
compare-ssim = SSIM { $ssim }
compare-ssim-small = O SSIM precisa de 11 píxeis
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, máx. { $max })
compare-delta-e-hdr = HDR cortado para ΔE
compare-aligned-centre = Centrado, { $width } × { $height } comparados
compare-aligned-scaled = B redimensionada { $scale }×, { $width } × { $height } comparados
compare-aligned-auto = Alinhado { $angle }° { $scale }×, { $width } × { $height } comparados
compare-aligning = A alinhar…
compare-align-failed = Não foi possível alinhar estas imagens. De volta ao canto superior esquerdo.
compare-split-label = { $b } sobre { $a }, divisor a { PERCENT($percent) }
compare-showing-which = A mostrar { $which }: { $name }
compare-auto-flicker-on = A alternar, { $rate } mudanças por segundo
compare-auto-flicker-off = Alternância parada
compare-linked = Imagens ligadas
compare-unlinked = Imagens desligadas: cada uma desloca-se à parte
compare-with-title = Comparar com
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Divisor
compare-a11y-space = Espaço de medida
compare-a11y-align = Alinhamento
