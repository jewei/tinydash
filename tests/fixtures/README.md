# Search result fixture

`app-selection.json` contains fixed results recorded before changes to search memory use. It preserves the expected search behavior independently of the current implementation.

The fixture uses 100 applications, tied names, aliases, Unicode, hidden applications, fixed usage timestamps, and more than 30 pins. It records result identities, order, scores, pin data, and actions. It omits icon payloads. The test compares the new search implementation with these results.

The fixture was generated in the isolated original source with `TINYDASH_WRITE_SELECTION_FIXTURE`. Normal tests only read it. Do not regenerate it from the changed search implementation to make a failed comparison pass.
