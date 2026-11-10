# Specular — Develop's AI denoise: the Detail card's AI mode, and the Denoise
# feature in Settings › Local AI.

# The Detail card: how noise is removed.
develop-noise-mode = Denoise
develop-noise-mode-classic = Classic
develop-noise-mode-ai = AI
# How much of the AI result is mixed with the classic one.
develop-noise-ai-blend = AI share
develop-noise-ai-draft =
    The AI result shows at { PERCENT(100) } and in exports.
    The classic result until then.
# $percent: formatted ("45%").
develop-noise-ai-running = Denoising… { $percent }
develop-noise-ai-ready = AI result shown.
develop-noise-ai-stopped = Stopped. The classic result shows.
develop-noise-ai-failed = AI denoise failed: { $error }
# $model is the model's name ("FFDNet").
develop-noise-ai-needs =
    Needs the { $model } add-on.
    The classic result until then.
# $needed: whole gigabytes.
develop-noise-ai-memory = Needs about { $needed } GB of free RAM.
develop-noise-ai-no-gpu = No graphics device for AI denoise.
develop-noise-ai-stop = Stop
develop-noise-ai-retry = Try again

## Settings › Local AI: the Denoise feature
settings-ai-denoise = AI denoise
settings-ai-denoise-help = Denoise in Develop with a network, mixed with the classic result. Runs at { PERCENT(100) } view and in exports.
settings-ai-denoise-size = Model
settings-ai-denoise-size-help = FFDNet is told the photo's noise level.
settings-ai-denoise-size-recommended = Default
settings-ai-denoise-size-small = FFDNet
settings-ai-denoise-size-standard = FFDNet
settings-ai-denoise-size-large = Large
