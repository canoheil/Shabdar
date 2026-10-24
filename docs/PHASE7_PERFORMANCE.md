# Phase 7 performance measurements

## Baseline provenance

The legacy-UI baseline was measured against the real Axum/Nuxt/MariaDB stack in [GitHub Actions run 40124660239](https://github.com/canoheil/Shabdar/actions/runs/40124660239), at commit `1be6df14f3845fbea5255f2c0292f5cd1f085ef6` on 2026-10-24. The product-baseline reference remains `c07cdd9ef7df304acf665008a57e79c6c2597224`.

- Nuxt `4.6.2`, Node `26.12.1`, Playwright `1.64.1`
- `@lhci/cli` `0.15.1`, Lighthouse `12.6.1`
- Production Nuxt build, mobile form factor, simulated throttling
- Lighthouse screen emulation: 412 × 823, DPR 1.75
- Only Performance and Accessibility categories; three runs per route
- Client responses captured as decoded bodies and compressed locally with gzip level 9

The committed machine-readable route baseline is [`performance/phase7-client-baseline.json`](performance/phase7-client-baseline.json). Full HTML/JSON Lighthouse reports and the route report are uploaded as the run's `web-performance-40124660239` artifact. Compact machine-readable copies are also available in the job's check annotations.

## Initial-route client JavaScript

| Route | Scripts | Decoded bytes | Gzip bytes | Maximum allowed in Phase 7 |
| --- | ---: | ---: | ---: | ---: |
| Home `/` | 12 | 230,617 | **89,027** | **114,627** |
| Detail `/listings/آگهی-آزمون-شبدر` | 12 | 230,617 | **89,027** | **114,627** |

The independent allowance is exactly `25,600` bytes (25 KiB) of gzip growth per route. Equality is allowed; `25,601` bytes fails. The baseline legacy UI requested no font files (`0` bytes), because the self-hosted Vazirmatn integration is part of the redesign rather than the old UI.

## Lighthouse baseline: median of three runs

| Route | LCP | CLS | TBT | Accessibility |
| --- | ---: | ---: | ---: | ---: |
| Home | **2483.55075 ms** | **0** | **42.5 ms** | **100** |
| Detail | **2637.2716 ms** | **0** | **36 ms** | **100** |

These are baseline observations, not the final acceptance result. Home is only 16.44925 ms inside the final LCP budget, and Detail currently misses the final LCP target by 137.2716 ms. The redesign must improve Detail and must not regress Home. Lighthouse assertions intentionally remain outside the required legacy-UI gate until Task 13.

Final per-route gates are LCP `<= 2500 ms`, CLS `<= 0.1`, TBT `<= 200 ms`, and Accessibility `>= 0.95`, each asserted over the median of three runs.

## Commands

Run these against a production build with the real stack and fixture guard variables configured:

```bash
npm run measure:lighthouse
npm run measure:client
npm run test:client-budget
```

Run Lighthouse before client measurement in the same checkout: both measurement commands refresh fixtures, and Playwright clears `test-results` at startup. The CI workflow uses this order so the final client report is not deleted.

For the final acceptance gate, `npm run test:lighthouse` performs three collections per route, median assertions, and filesystem report upload. `npm run test:client-budget` also runs the comparator unit tests before comparing the current report with the committed baseline.
