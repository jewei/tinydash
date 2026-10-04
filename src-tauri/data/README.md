# Bundled data

Third-party data that is compiled into the app. Nothing here is downloaded at run time.

| File                     | Source                                                                                                                                                    | License                                                                 |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `emoji/{zh,ms,es}.tsv`   | Unicode CLDR JSON 48.0.0 annotations, commit `4d06be52`                                                                                                   | Unicode License V3 ([`emoji/LICENSE`](emoji/LICENSE))                   |
| `eff_large_wordlist.txt` | [EFF Large Wordlist](https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt), SHA-256 `addd35536511597a02fa0a9ff1e5284677b8883b83e986e43f15a3db996b903e` | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), unchanged |

Each emoji row is: emoji, localized short name (may be empty), then related keywords, tab-separated. Skin-tone variants are excluded; the app derives them. Regenerate with `bun run emoji-data`; the script checks the SHA-256 of every source file.
