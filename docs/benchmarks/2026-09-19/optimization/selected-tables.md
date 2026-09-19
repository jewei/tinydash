# Memory comparison tables

Each run starts a fresh application process. Each idle-state value is the median of 31 physical-footprint samples after a 30-second wait. Values include the host and every attributed helper. The final column shows the range of the five run values.

Visible idle footprint in MiB.

| Build and profile                  | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range          |
| ---------------------------------- | ----: | ----: | ----: | ----: | ----: | -----: | -------------- |
| Original / controlled              | 130.2 | 136.6 | 135.5 | 164.1 | 163.2 |  136.6 | 130.2 to 164.1 |
| Selected icon changes / controlled | 158.6 | 165.3 | 155.0 | 154.2 | 283.2 |  158.6 | 154.2 to 283.2 |
| Original / daily                   | 369.3 | 352.8 | 209.2 | 372.4 | 251.1 |  352.8 | 209.2 to 372.4 |
| Selected icon changes / daily      | 431.8 | 294.3 | 318.7 | 286.2 | 392.2 |  318.7 | 286.2 to 431.8 |

Hidden footprint in MiB.

| Build and profile                  | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range          |
| ---------------------------------- | ----: | ----: | ----: | ----: | ----: | -----: | -------------- |
| Original / controlled              | 122.0 | 126.5 | 127.1 | 156.4 | 153.5 |  127.1 | 122.0 to 156.4 |
| Selected icon changes / controlled | 150.3 | 121.0 | 146.7 | 146.2 | 128.2 |  146.2 | 121.0 to 150.3 |
| Original / daily                   | 186.1 | 153.7 | 200.7 | 167.7 | 225.7 |  186.1 | 153.7 to 225.7 |
| Selected icon changes / daily      | 182.2 | 276.3 | 286.2 | 277.7 | 176.4 |  276.3 | 176.4 to 286.2 |

Hidden footprint after 20 Settings cycles in MiB.

| Build and profile                  | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range          |
| ---------------------------------- | ----: | ----: | ----: | ----: | ----: | -----: | -------------- |
| Original / controlled              | 180.8 | 191.3 | 187.9 | 189.4 | 208.5 |  189.4 | 180.8 to 208.5 |
| Selected icon changes / controlled | 185.5 | 177.0 | 226.2 | 178.5 | 196.1 |  185.5 | 177.0 to 226.2 |
| Original / daily                   | 278.2 | 212.1 | 239.2 | 221.0 | 284.1 |  239.2 | 212.1 to 284.1 |
| Selected icon changes / daily      | 241.8 | 333.5 | 340.4 | 296.4 | 243.0 |  296.4 | 241.8 to 340.4 |

Sampled peak for the common workload before Settings in MiB.

| Build and profile                  | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range          |
| ---------------------------------- | ----: | ----: | ----: | ----: | ----: | -----: | -------------- |
| Original / controlled              | 343.4 | 322.9 | 359.0 | 325.6 | 338.3 |  338.3 | 322.9 to 359.0 |
| Selected icon changes / controlled | 288.7 | 320.2 | 303.1 | 280.1 | 434.9 |  303.1 | 280.1 to 434.9 |
| Original / daily                   | 545.5 | 561.7 | 516.0 | 553.1 | 429.9 |  545.5 | 429.9 to 561.7 |
| Selected icon changes / daily      | 601.8 | 463.1 | 497.2 | 507.3 | 559.6 |  507.3 | 463.1 to 601.8 |

Sampled peak for the full workload in MiB.

| Build and profile                  | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range          |
| ---------------------------------- | ----: | ----: | ----: | ----: | ----: | -----: | -------------- |
| Original / controlled              | 343.4 | 322.9 | 359.0 | 325.6 | 338.3 |  338.3 | 322.9 to 359.0 |
| Selected icon changes / controlled | 376.1 | 320.2 | 399.4 | 350.8 | 434.9 |  376.1 | 320.2 to 434.9 |
| Original / daily                   | 545.5 | 561.7 | 516.0 | 553.1 | 429.9 |  545.5 | 429.9 to 561.7 |
| Selected icon changes / daily      | 601.8 | 466.4 | 497.2 | 507.3 | 559.6 |  507.3 | 466.4 to 601.8 |

Sampling can miss short peaks.

Hidden host footprint in MiB. This table excludes helpers. It must not replace the total-application tables.

| Build and profile                  | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range        |
| ---------------------------------- | ----: | ----: | ----: | ----: | ----: | -----: | ------------ |
| Original / controlled              |  30.6 |  34.1 |  33.4 |  33.9 |  32.8 |   33.4 | 30.6 to 34.1 |
| Selected icon changes / controlled |  27.2 |  27.9 |  27.5 |  27.8 |  27.5 |   27.5 | 27.2 to 27.9 |
| Original / daily                   |  77.4 |  54.3 |  67.2 |  68.8 |  54.1 |   67.2 | 54.1 to 77.4 |
| Selected icon changes / daily      |  70.4 |  82.8 |  61.9 |  79.2 |  65.6 |   70.4 | 61.9 to 82.8 |

Latency in milliseconds. The main launcher timings use native accessibility checks. Settings reopen timings end when the restored editing fields are ready. The first Settings visit also includes selection of an application and entry of the test draft. These timings do not measure exact display-pixel arrival.

| Build and profile                  | Operation                            | Samples |   p50 |   p95 | Range of run p50 values |
| ---------------------------------- | ------------------------------------ | ------: | ----: | ----: | ----------------------- |
| Original / controlled              | App search                           |     100 |  29.5 |  53.7 | 27.6 to 30.7            |
| Original / controlled              | Arithmetic                           |     100 |  16.9 |  50.3 | 15.5 to 18.9            |
| Original / controlled              | Warm launcher reopen                 |     100 |  39.1 |  49.0 | 30.6 to 41.5            |
| Original / controlled              | First Settings visit and draft setup |       5 | 276.9 | 283.8 | 262.7 to 283.8          |
| Original / controlled              | Settings reopen                      |      95 |   7.5 |  37.9 | 3.5 to 10.8             |
| Selected icon changes / controlled | App search                           |     100 |  30.5 |  57.1 | 29.3 to 35.7            |
| Selected icon changes / controlled | Arithmetic                           |     100 |  16.3 |  50.1 | 13.7 to 23.5            |
| Selected icon changes / controlled | Warm launcher reopen                 |     100 |  36.9 |  47.8 | 27.2 to 41.5            |
| Selected icon changes / controlled | First Settings visit and draft setup |       5 | 277.8 | 289.2 | 274.3 to 289.2          |
| Selected icon changes / controlled | Settings reopen                      |      95 |   9.3 |  39.9 | 8.0 to 12.3             |
| Original / daily                   | App search                           |     100 |  49.7 |  64.1 | 39.4 to 55.0            |
| Original / daily                   | Arithmetic                           |     100 |  15.8 |  48.8 | 14.4 to 19.3            |
| Original / daily                   | Warm launcher reopen                 |     100 |  34.4 |  47.3 | 29.0 to 39.5            |
| Original / daily                   | First Settings visit and draft setup |       5 | 276.8 | 284.7 | 272.2 to 284.7          |
| Original / daily                   | Settings reopen                      |      95 |   9.5 |  38.3 | 7.2 to 12.7             |
| Selected icon changes / daily      | App search                           |     100 |  52.2 |  65.7 | 35.8 to 54.3            |
| Selected icon changes / daily      | Arithmetic                           |     100 |  18.1 |  51.5 | 15.9 to 26.7            |
| Selected icon changes / daily      | Warm launcher reopen                 |     100 |  37.0 |  46.2 | 28.3 to 40.4            |
| Selected icon changes / daily      | First Settings visit and draft setup |       5 | 283.6 | 285.6 | 274.0 to 285.6          |
| Selected icon changes / daily      | Settings reopen                      |      95 |   9.1 |  38.4 | 7.5 to 10.5             |

The following verdicts apply the plan's latency budgets and measured run-to-run spread. An inconclusive verdict is not a passed performance gate.

| Parent   | Candidate             | Profile    | Operation  | p50 change | p95 change | Verdict      |
| -------- | --------------------- | ---------- | ---------- | ---------: | ---------: | ------------ |
| Original | Selected icon changes | controlled | apps       |       +1.0 |       +3.4 | inconclusive |
| Original | Selected icon changes | controlled | arithmetic |       -0.6 |       -0.2 | inconclusive |
| Original | Selected icon changes | controlled | reopen     |       -2.2 |       -1.2 | inconclusive |
| Original | Selected icon changes | daily      | apps       |       +2.4 |       +1.6 | inconclusive |
| Original | Selected icon changes | daily      | arithmetic |       +2.2 |       +2.7 | inconclusive |
| Original | Selected icon changes | daily      | reopen     |       +2.6 |       -1.2 | inconclusive |

All values come from `selected-summary.json`. Raw records retain process identities, environment data, individual latency samples, and failures. No helper or valid outlier was removed.
