# Search result fixture

`app-selection.json` contains results from the source frozen before memory optimization. The base commit is `568972bbc92df37f2bbfd2951e8ab478b061e205`. The frozen source also includes the working changes recorded by the memory benchmark's baseline manifest and patch.

The fixture uses 100 applications, tied names, aliases, Unicode, hidden applications, fixed usage timestamps, and more than 30 pins. It records result identities, order, scores, pin data, and actions. It omits icon payloads. The test compares the new search implementation with these results.

The fixture was generated in the isolated original source with `TINYDASH_WRITE_SELECTION_FIXTURE`. Normal tests only read it. Do not regenerate it from the changed search implementation to make a failed comparison pass.
