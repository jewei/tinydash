# Emoji search keywords

These tab-separated tables contain Simplified Chinese (`zh`), Malay (`ms`), and Spanish (`es`) short names and keywords from [Unicode CLDR JSON 48.0.0](https://github.com/unicode-org/cldr-json/tree/48.0.0). The source commit is `4d06be52b51bb2f75688d0abe55c52a66afed790`.

Each row starts with a Unicode sequence, followed by distinct search terms. The generator combines `annotations` and `annotationsDerived`, normalizes terms to NFC, and excludes skin-tone rows. The Rust provider accepts only sequences present in its `emojis` catalog and indexes only selected languages. The catalog supplies skin-tone variants and English result labels.

The data uses the [Unicode License V3](LICENSE). Keep that notice with redistributed data. There is no runtime download.

Regenerate from the repository root:

```sh
bun scripts/update-emoji-keywords.ts
```

The script checks SHA-256 hashes of all upstream inputs. To update the dataset, review a new CLDR release, pin its commit and source hashes in the script, regenerate, and check representative terms and unsupported sequences.
