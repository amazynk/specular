# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Comparar com…
palette-compare-with-help = Comparar esta imagem com outra que você escolher.
palette-compare-side = Comparar lado a lado
palette-compare-side-help = Mostrar as imagens comparadas uma ao lado da outra.
palette-compare-split = Comparar com divisão
palette-compare-split-help = Mostrar a segunda imagem sobre a primeira, cortada por um divisor que você pode arrastar.
palette-compare-split-direction = Girar o divisor
palette-compare-split-direction-help = Colocar o divisor na horizontal em vez da vertical, ou voltar.
palette-compare-overlay = Comparar em sobreposição
palette-compare-overlay-help = Mostrar a segunda imagem sobre a primeira, mesclada.
palette-compare-difference = Comparar a diferença
palette-compare-difference-help = Mostrar onde as imagens diferem; de novo para o próximo tipo.
palette-compare-flicker = Alternar entre as imagens comparadas
palette-compare-flicker-help = Mostrar a outra imagem na hora.
palette-compare-auto-flicker = Alternância automática
palette-compare-auto-flicker-help = Alternar sozinho entre as duas imagens, ou parar.
palette-compare-align = Alinhar as imagens comparadas
palette-compare-align-help = Alinhar imagens de tamanhos diferentes: no canto superior esquerdo, centralizadas, redimensionadas ou automaticamente.
palette-compare-metric-space = Trocar o espaço de medida
palette-compare-metric-space-help = Medir em luz linear ou em sRGB codificado.
palette-compare-whole-image = Medir a imagem inteira
palette-compare-whole-image-help = Medir a imagem inteira em vez da parte visível, ou voltar.
palette-compare-probe = Sonda de pixel da comparação
palette-compare-probe-help = Mostrar os valores das duas imagens sob o ponteiro.
palette-compare-link = Vincular ou desvincular as imagens comparadas
palette-compare-link-help = Ampliar e mover cada uma das duas imagens separadamente, ou juntas de novo.
palette-compare-close = Fechar a comparação
palette-compare-close-help = Voltar para a galeria.
disabled-compare-closed = Abra primeiro a comparação
disabled-compare-not-two = Funciona com duas imagens

## Cheat sheet
cheatsheet-compare-split = Divisão ou lado a lado
cheatsheet-compare-split-direction = Divisor horizontal ou vertical
cheatsheet-compare-auto-flicker = Alternância automática
cheatsheet-compare-align = Próximo alinhamento
cheatsheet-compare-space = Espaço de medida
cheatsheet-compare-whole = Imagem inteira ou parte visível
cheatsheet-compare-probe = Sonda de pixel
cheatsheet-compare-link = Vincular ou desvincular duas imagens

## Settings
settings-gallery-compare-metric-space = Espaço de medida da comparação
settings-gallery-compare-metric-space-help = Onde a comparação mede MAE, MSE, PSNR e SSIM. Luz linear é o que a tela mostra. sRGB coincide com outras ferramentas em arquivos de 8-bit.
settings-gallery-compare-metric-space-linear = Luz linear
settings-gallery-compare-metric-space-srgb = sRGB codificado
settings-gallery-compare-flicker-limit = Alternância automática mais rápida
settings-gallery-compare-flicker-limit-help = Trocas de imagem por segundo, no máximo. Acima de 3, o piscar pode provocar convulsões.

## Toolbar and captions
compare-split = Divisão
compare-split-down = Divisor vertical
compare-split-across = Divisor horizontal
compare-link = Vinculadas
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } trocas por segundo
compare-gain = Ganho ×{ $gain }
compare-probe = Sonda
compare-kind-signed = Erro com sinal
compare-kind-on-image = Erro sobre a imagem
compare-align-top-left = Canto superior esquerdo
compare-align-centre = Centralizado
compare-align-scaled = B redimensionada para A
compare-align-auto = Alinhamento automático
compare-space-linear = Luz linear
compare-space-srgb = sRGB
compare-space-help-linear = Medido em luz linear, como exibido após o mapeamento de tons. Aqui o PSNR não é o PSNR usual de sRGB 8-bit.
compare-space-help-srgb = Medido nos valores sRGB codificados, como outras ferramentas fazem com arquivos de 8-bit. HDR acima do branco SDR conta como branco.
compare-space-help-common = ΔE2000 é sempre CIELAB (D65), com o branco SDR como L* 100 e o HDR cortado. Imagens com mais de { $size } pixels são medidas em uma cópia de { $size } pixels.
compare-region-view = Parte visível
compare-region-whole = Imagem inteira
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM precisa de 11 pixels
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, máx. { $max })
compare-delta-e-hdr = HDR cortado para ΔE
compare-aligned-centre = Centralizado, { $width } × { $height } comparados
compare-aligned-scaled = B redimensionada { $scale }×, { $width } × { $height } comparados
compare-aligned-auto = Alinhado { $angle }° { $scale }×, { $width } × { $height } comparados
compare-aligning = Alinhando…
compare-align-failed = Não foi possível alinhar estas imagens. De volta ao canto superior esquerdo.
compare-split-label = { $b } sobre { $a }, divisor a { PERCENT($percent) }
compare-showing-which = Mostrando { $which }: { $name }
compare-auto-flicker-on = Alternando, { $rate } trocas por segundo
compare-auto-flicker-off = Alternância parada
compare-linked = Imagens vinculadas
compare-unlinked = Imagens desvinculadas: cada uma se move sozinha
compare-with-title = Comparar com
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Divisor
compare-a11y-space = Espaço de medida
compare-a11y-align = Alinhamento
