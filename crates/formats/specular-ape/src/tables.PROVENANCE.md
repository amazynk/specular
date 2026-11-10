# Provenance of `tables.rs`

- **What:** the two 64-entry symbol-width tables of Monkey's Audio's range-coded overflow
  model (`RANGE_WIDTH_1`, `RANGE_WIDTH_2`). Data only; no code was copied. The cumulative
  totals are computed from them (and checked against the SDK's printed totals in a test).
- **Source:** Monkey's Audio SDK 13.43, `Source/MACLib/UnBitArrayBase.h`, downloaded from
  <https://monkeysaudio.com/files/MAC_1343_SDK.zip> (SHA-256
  `3fd1c60f994daba536902f30b1bebc2235099c8b8a039d8b37d9d27d4827e0ca`), 2026-10-25.
- **Licence:** "Monkey's Audio License Agreement (3 clause BSD)", Copyright 2000-2026
  Matthew T. Ashland. The SDK moved to the 3-clause BSD licence in version 10.18
  (2023-08-10; version history on monkeysaudio.com).
- **Notice:** Redistribution and use in source and binary forms, with or without
  modification, are permitted provided that the following conditions are met: (1)
  redistributions of source code must retain the above copyright notice, this list of
  conditions and the following disclaimer; (2) redistributions in binary form must
  reproduce the above copyright notice, this list of conditions and the following
  disclaimer in the documentation and/or other materials provided with the distribution;
  (3) neither the name of the copyright holder nor the names of its contributors may be
  used to endorse or promote products derived from this software without specific prior
  written permission. THIS SOFTWARE IS PROVIDED "AS IS" AND ANY EXPRESS OR IMPLIED
  WARRANTIES ARE DISCLAIMED.
