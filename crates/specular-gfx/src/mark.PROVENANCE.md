# Provenance of the Discord mark in `mark.rs`

- What: Discord's symbol (the "Clyde" logo), black version. Only its SVG path data is used, as
  data, unchanged: `DISCORD_PATH` is the `d` attribute of the file, character for character
  (viewBox `0 0 126.644 96`, non-zero fill). The app tints it like its other icons (the theme's
  icon colour: light on dark, dark on light).
- Source: Discord's brand guidelines, <https://discord.com/branding>, "Symbol", black mark SVG
  download, fetched 2026-11-08:
  - <https://cdn.discordapp.com/assets/content/aecd9009fd6a24c06c8ce71f6bb84463eaf7252bc5259022fb887b381a428250.zip>
    (sha256 `aecd9009fd6a24c06c8ce71f6bb84463eaf7252bc5259022fb887b381a428250`)
  - `Discord_Symbol_Black/Discord-Symbol-Black.svg` inside it
    (sha256 `2d1bd85831cdab67a5b8c893a218a47c3892b66a9fd073659022b8c8669fff2e`).
- Trademark: DISCORD and the "Clyde" logo are trademarks of Discord Inc. Specular is not made,
  sponsored or endorsed by Discord. The brand guidelines allow the Discord Marks "to inform
  people that you ... have a Discord server", in digital use only, in colour, black or white,
  without editing, distorting or reconfiguring the logo. Specular uses the mark only on the
  button and entries that open the invite to the Specular Discord server
  (`specular_core::links::DISCORD_INVITE`), always with "Join the Specular Discord" as the
  tooltip and accessible name.
- Not for anything else: not as Specular's own logo, not in other icons, not on merchandise.
