# Memory comparison tables

Each run starts a fresh application process. Each idle-state value is the median of 31 physical-footprint samples after a 30-second wait. Values include the host and every attributed helper. The final column shows the range of the five run values.

Visible idle footprint in MiB.

| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Original / controlled | 172.4 | 133.2 | 157.6 | 164.8 | 162.1 | 162.1 | 133.2 to 172.4 |
| Settings release / controlled | 132.8 | 135.8 | 166.2 | 162.3 | 169.1 | 162.3 | 132.8 to 169.1 |
| Deferred copies / controlled | 172.3 | 166.4 | 158.9 | 169.7 | 162.6 | 166.4 | 158.9 to 172.3 |
| On-demand data URL / controlled | 167.8 | 155.2 | 162.8 | 169.7 | 160.8 | 162.8 | 155.2 to 169.7 |
| Binary PNG trial (deferred) / controlled | 174.6 | 153.9 | 146.8 | 160.6 | 154.7 | 154.7 | 146.8 to 174.6 |
| Original / daily | 318.8 | 315.2 | 387.1 | 286.0 | 380.9 | 318.8 | 286.0 to 387.1 |
| Binary PNG trial (deferred) / daily | 199.7 | 462.2 | 455.1 | 264.6 | 440.6 | 440.6 | 199.7 to 462.2 |

Hidden footprint in MiB.

| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Original / controlled | 163.1 | 125.3 | 165.4 | 165.0 | 152.3 | 163.1 | 125.3 to 165.4 |
| Settings release / controlled | 125.9 | 125.7 | 163.9 | 154.5 | 158.1 | 154.5 | 125.7 to 163.9 |
| Deferred copies / controlled | 165.4 | 158.7 | 162.6 | 161.8 | 152.2 | 161.8 | 152.2 to 165.4 |
| On-demand data URL / controlled | 117.6 | 147.0 | 128.6 | 121.9 | 150.4 | 128.6 | 117.6 to 150.4 |
| Binary PNG trial (deferred) / controlled | 120.2 | 145.1 | 148.5 | 162.1 | 116.2 | 145.1 | 116.2 to 162.1 |
| Original / daily | 310.1 | 269.9 | 190.7 | 269.5 | 168.2 | 269.5 | 168.2 to 310.1 |
| Binary PNG trial (deferred) / daily | 178.4 | 217.4 | 218.0 | 248.1 | 190.7 | 217.4 | 178.4 to 248.1 |

Hidden footprint after 20 Settings cycles in MiB.

| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Original / controlled | 224.8 | 221.6 | 199.7 | 233.2 | 212.0 | 221.6 | 199.7 to 233.2 |
| Settings release / controlled | 152.9 | 186.9 | 197.1 | 197.1 | 185.8 | 186.9 | 152.9 to 197.1 |
| Binary PNG trial (deferred) / controlled | 155.3 | 235.3 | 171.1 | 210.7 | 160.0 | 171.1 | 155.3 to 235.3 |
| Original / daily | 338.3 | 329.7 | 277.1 | 212.7 | 221.3 | 277.1 | 212.7 to 338.3 |
| Binary PNG trial (deferred) / daily | 205.8 | 279.9 | 271.3 | 324.0 | 217.8 | 271.3 | 205.8 to 324.0 |

Sampled peak for the common workload before Settings in MiB.

| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Original / controlled | 341.8 | 341.9 | 325.3 | 340.5 | 336.1 | 340.5 | 325.3 to 341.9 |
| Settings release / controlled | 328.7 | 336.3 | 321.3 | 318.7 | 348.1 | 328.7 | 318.7 to 348.1 |
| Deferred copies / controlled | 343.3 | 327.3 | 328.9 | 342.8 | 325.9 | 328.9 | 325.9 to 343.3 |
| On-demand data URL / controlled | 320.9 | 292.1 | 312.6 | 319.8 | 310.5 | 312.6 | 292.1 to 320.9 |
| Binary PNG trial (deferred) / controlled | 325.4 | 300.7 | 289.7 | 309.1 | 265.8 | 300.7 | 265.8 to 325.4 |
| Original / daily | 484.9 | 481.4 | 533.9 | 454.4 | 552.2 | 484.9 | 454.4 to 552.2 |
| Binary PNG trial (deferred) / daily | 454.7 | 614.2 | 604.3 | 443.6 | 601.8 | 601.8 | 443.6 to 614.2 |

Sampled peak for the full workload in MiB.

| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Original / controlled | 341.8 | 360.3 | 370.1 | 367.5 | 342.6 | 360.3 | 341.8 to 370.1 |
| Settings release / controlled | 328.7 | 336.3 | 321.3 | 318.7 | 348.1 | 328.7 | 318.7 to 348.1 |
| Deferred copies / controlled | 343.3 | 327.3 | 328.9 | 342.8 | 325.9 | 328.9 | 325.9 to 343.3 |
| On-demand data URL / controlled | 320.9 | 292.1 | 312.6 | 319.8 | 310.5 | 312.6 | 292.1 to 320.9 |
| Binary PNG trial (deferred) / controlled | 325.4 | 300.7 | 289.7 | 309.1 | 265.8 | 300.7 | 265.8 to 325.4 |
| Original / daily | 484.9 | 481.4 | 533.9 | 454.4 | 552.2 | 484.9 | 454.4 to 552.2 |
| Binary PNG trial (deferred) / daily | 454.7 | 614.2 | 604.3 | 443.6 | 601.8 | 601.8 | 443.6 to 614.2 |

P2 and P3a omit the Settings cycle phase. Use the common-workload peak when comparing those steps. Sampling can miss short peaks.

Hidden host footprint in MiB. This table excludes helpers. It must not replace the total-application tables.

| Build and profile | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median | Range |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Original / controlled | 32.4 | 33.9 | 34.4 | 32.2 | 32.0 | 32.4 | 32.0 to 34.4 |
| Settings release / controlled | 33.9 | 33.9 | 31.0 | 33.9 | 31.8 | 33.9 | 31.0 to 33.9 |
| Deferred copies / controlled | 33.9 | 33.9 | 31.1 | 33.9 | 33.1 | 33.9 | 31.1 to 33.9 |
| On-demand data URL / controlled | 26.1 | 27.9 | 26.5 | 27.3 | 27.8 | 27.3 | 26.1 to 27.9 |
| Binary PNG trial (deferred) / controlled | 27.5 | 27.6 | 26.3 | 27.9 | 26.2 | 27.5 | 26.2 to 27.9 |
| Original / daily | 95.3 | 89.5 | 79.7 | 77.1 | 68.7 | 79.7 | 68.7 to 95.3 |
| Binary PNG trial (deferred) / daily | 74.1 | 74.2 | 72.9 | 71.1 | 58.9 | 72.9 | 58.9 to 74.2 |

Latency in milliseconds. The main launcher timings use native accessibility checks. Settings reopen timings end when the restored editing fields are ready. The first Settings visit also includes selection of an application and entry of the test draft. These timings do not measure exact display-pixel arrival.

| Build and profile | Operation | Samples | p50 | p95 | Range of run p50 values |
| --- | --- | ---: | ---: | ---: | --- |
| Original / controlled | App search | 100 | 31.7 | 63.2 | 28.8 to 37.0 |
| Original / controlled | Arithmetic | 100 | 21.3 | 50.6 | 14.1 to 26.0 |
| Original / controlled | Warm launcher reopen | 100 | 38.5 | 46.2 | 29.3 to 40.5 |
| Original / controlled | First Settings visit and draft setup | 5 | 274.3 | 288.8 | 237.9 to 288.8 |
| Original / controlled | Settings reopen | 95 | 7.8 | 38.0 | 4.2 to 11.0 |
| Settings release / controlled | App search | 100 | 32.7 | 59.6 | 31.2 to 37.1 |
| Settings release / controlled | Arithmetic | 100 | 21.3 | 49.9 | 15.6 to 25.3 |
| Settings release / controlled | Warm launcher reopen | 100 | 37.7 | 45.3 | 31.3 to 43.3 |
| Settings release / controlled | First Settings visit and draft setup | 5 | 277.6 | 348.2 | 230.9 to 348.2 |
| Settings release / controlled | Settings reopen | 95 | 170.7 | 181.6 | 166.4 to 179.2 |
| Deferred copies / controlled | App search | 100 | 31.8 | 57.1 | 29.8 to 34.3 |
| Deferred copies / controlled | Arithmetic | 100 | 15.2 | 56.3 | 12.5 to 23.9 |
| Deferred copies / controlled | Warm launcher reopen | 100 | 33.5 | 45.5 | 26.3 to 38.4 |
| On-demand data URL / controlled | App search | 100 | 33.4 | 57.5 | 23.6 to 50.3 |
| On-demand data URL / controlled | Arithmetic | 100 | 24.2 | 54.5 | 19.3 to 30.7 |
| On-demand data URL / controlled | Warm launcher reopen | 100 | 33.0 | 45.3 | 28.4 to 41.7 |
| Binary PNG trial (deferred) / controlled | App search | 100 | 33.3 | 60.4 | 31.2 to 35.4 |
| Binary PNG trial (deferred) / controlled | Arithmetic | 100 | 15.5 | 50.0 | 13.8 to 25.8 |
| Binary PNG trial (deferred) / controlled | Warm launcher reopen | 100 | 31.4 | 44.8 | 28.2 to 38.1 |
| Binary PNG trial (deferred) / controlled | First Settings visit and draft setup | 5 | 285.6 | 301.4 | 221.1 to 301.4 |
| Binary PNG trial (deferred) / controlled | Settings reopen | 95 | 172.7 | 184.2 | 170.9 to 174.8 |
| Original / daily | App search | 100 | 52.1 | 62.8 | 40.9 to 53.7 |
| Original / daily | Arithmetic | 100 | 21.5 | 53.0 | 13.3 to 41.9 |
| Original / daily | Warm launcher reopen | 100 | 35.3 | 45.1 | 31.6 to 39.9 |
| Original / daily | First Settings visit and draft setup | 5 | 269.7 | 276.4 | 228.2 to 276.4 |
| Original / daily | Settings reopen | 95 | 6.8 | 41.0 | 3.0 to 9.1 |
| Binary PNG trial (deferred) / daily | App search | 100 | 54.0 | 64.1 | 44.6 to 58.7 |
| Binary PNG trial (deferred) / daily | Arithmetic | 100 | 16.3 | 51.9 | 13.5 to 22.3 |
| Binary PNG trial (deferred) / daily | Warm launcher reopen | 100 | 36.1 | 45.6 | 29.6 to 39.6 |
| Binary PNG trial (deferred) / daily | First Settings visit and draft setup | 5 | 284.9 | 290.5 | 278.4 to 290.5 |
| Binary PNG trial (deferred) / daily | Settings reopen | 95 | 168.7 | 182.4 | 164.9 to 173.7 |

The following verdicts apply the plan's latency budgets and measured run-to-run spread. An inconclusive verdict is not a passed performance gate.

| Parent | Candidate | Profile | Operation | p50 change | p95 change | Verdict |
| --- | --- | --- | --- | ---: | ---: | --- |
| Original | Binary PNG trial (deferred) | controlled | apps | +1.6 | -2.8 | inconclusive |
| Original | Binary PNG trial (deferred) | controlled | arithmetic | -5.8 | -0.6 | inconclusive |
| Original | Binary PNG trial (deferred) | controlled | reopen | -7.1 | -1.4 | inconclusive |
| Original | Settings release | controlled | apps | +0.9 | -3.6 | inconclusive |
| Original | Settings release | controlled | arithmetic | +0.0 | -0.7 | inconclusive |
| Original | Settings release | controlled | reopen | -0.7 | -0.9 | inconclusive |
| Settings release | Deferred copies | controlled | apps | -0.9 | -2.5 | inconclusive |
| Settings release | Deferred copies | controlled | arithmetic | -6.1 | +6.4 | inconclusive |
| Settings release | Deferred copies | controlled | reopen | -4.2 | +0.2 | inconclusive |
| Deferred copies | On-demand data URL | controlled | apps | +1.6 | +0.4 | inconclusive |
| Deferred copies | On-demand data URL | controlled | arithmetic | +9.0 | -1.8 | inconclusive |
| Deferred copies | On-demand data URL | controlled | reopen | -0.5 | -0.3 | inconclusive |
| On-demand data URL | Binary PNG trial (deferred) | controlled | apps | -0.1 | +2.9 | inconclusive |
| On-demand data URL | Binary PNG trial (deferred) | controlled | arithmetic | -8.7 | -4.5 | inconclusive |
| On-demand data URL | Binary PNG trial (deferred) | controlled | reopen | -1.6 | -0.5 | inconclusive |
| Original | Binary PNG trial (deferred) | daily | apps | +1.9 | +1.3 | inconclusive |
| Original | Binary PNG trial (deferred) | daily | arithmetic | -5.1 | -1.1 | inconclusive |
| Original | Binary PNG trial (deferred) | daily | reopen | +0.8 | +0.5 | inconclusive |

All values come from `summary.json`. Raw records retain process identities, environment data, individual latency samples, and failures. No helper or valid outlier was removed.
