# machine-drafted, needs native review
# Specular — Develop's AI denoise: the Detail card's AI mode, and the Denoise
# feature in Settings › Local AI.

develop-noise-mode = ノイズ除去
develop-noise-mode-classic = 従来
develop-noise-mode-ai = AI
develop-noise-ai-blend = AI の割合
develop-noise-ai-draft =
    AI の結果は { PERCENT(100) } 表示と書き出しで表示されます。
    それまでは従来の結果です。
develop-noise-ai-running = ノイズ除去中… { $percent }
develop-noise-ai-ready = AI の結果を表示中。
develop-noise-ai-stopped = 停止しました。従来の結果を表示中。
develop-noise-ai-failed = AI ノイズ除去に失敗しました: { $error }
develop-noise-ai-needs =
    { $model } アドオンが必要です。
    それまでは従来の結果です。
develop-noise-ai-memory = 約 { $needed } GB の空きメモリが必要です。
develop-noise-ai-no-gpu = AI ノイズ除去用のグラフィックデバイスがありません。
develop-noise-ai-stop = 停止
develop-noise-ai-retry = 再試行

## Settings › Local AI: the Denoise feature
settings-ai-denoise = AI ノイズ除去
settings-ai-denoise-help = 現像でネットワークによりノイズを除去し、従来の結果と混ぜます。{ PERCENT(100) } 表示と書き出しで動作します。
settings-ai-denoise-size = モデル
settings-ai-denoise-size-help = FFDNet には写真のノイズ量が渡されます。
settings-ai-denoise-size-recommended = 既定
settings-ai-denoise-size-small = FFDNet
settings-ai-denoise-size-standard = FFDNet
settings-ai-denoise-size-large = 大
