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

## Initial-route client JavaScript baseline

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

These are baseline observations, not the final acceptance result. Home was only 16.44925 ms inside the final LCP budget, and Detail missed the final LCP target by 137.2716 ms. Lighthouse assertions intentionally remained outside the required legacy-UI gate until Task 13.

## Final production gate — PASS

Final acceptance ran in [GitHub Actions run 40182538514](https://github.com/canoheil/Shabdar/actions/runs/40182538514), at commit `85d63f0eaa3397b6bde80890d0cabd5d5631bb5a` on 2026-10-25. It used the same real Axum/Nuxt/MariaDB production procedure, dependency versions, mobile simulated throttling, screen emulation, and three-run median method as the baseline. Unlike the baseline collection, this run executed `npm run test:lighthouse` with all median assertions enforced.

### Initial-route client JavaScript and fonts

| Route | Scripts | Decoded script bytes | Gzip bytes | Delta from 89,027 | Allowed maximum |
| --- | ---: | ---: | ---: | ---: | ---: |
| Home `/` | 3 | 257,878 | **93,008** | **+3,981** | 114,627 |
| Detail `/listings/آگهی-آزمون-شبدر` | 3 | 253,181 | **91,796** | **+2,769** | 114,627 |

Both independent route budgets passed. The two self-hosted variable WOFF2 files total **80,832 bytes**: Arabic **46,308** and Latin **34,524**. No third-party font or icon request is made.

The initial render uses the system fallback and enables Vazirmatn after the initial load/paint so the font files do not compete with the LCP image; the `@font-face` rules retain `font-display: swap`, and no-JavaScript rendering applies Vazirmatn immediately. This late font enhancement is included in the measured CLS below. Rendered HTML and public static assets use content compression. PNG media uses negotiated streaming gzip without changing its decoded bytes; JPEG/WebP, Range/HEAD, statuses, and API behavior are not rewritten.

### Enforced Lighthouse medians

| Route | FCP (observed) | LCP | CLS | TBT | Accessibility |
| --- | ---: | ---: | ---: | ---: | ---: |
| Home | 672.03785 ms | **2406.0757 ms** | **0.0066187523** | **88 ms** | **100** |
| Detail | 675.91445 ms | **2403.8289 ms** | **0.0036143110** | **84 ms** | **100** |
| Required gate | — | **≤2500 ms** | **≤0.1** | **≤200 ms** | **≥95** |

Every individual final LCP run was also below 2500 ms (Home 2403.5383–2407.1478 ms; Detail 2403.6257–2404.9403 ms). The run passed the production build, `nuxi typecheck`, all **55** real-stack Playwright tests, comparator unit tests, independent route budgets, and Lighthouse assertions. Raw `.lighthouseci` data, filesystem HTML/JSON reports, route JSON, and labelled quality screenshots are in `web-performance-40182538514` (9,784,081 uploaded bytes); compact summaries are in the job annotations.

## Accepted measurement ceilings

- The representative 1200×900 PNG fixture remains unchanged at 197,567 decoded bytes. Transport compression is not an image resize, thumbnail pipeline, or modern-format conversion; those remain outside Phase 7.
- Lighthouse used headless Chromium with simulated mobile throttling, not a physical phone. Physical-device testing and real-user monitoring (RUM/field Core Web Vitals) remain open and are not implied by this PASS.
- The measurements cover seeded Home and Detail. They do not claim identical lab performance for arbitrary user media, production hosting, or every route.

## Commands

Run these against a production build with the real stack and fixture guard variables configured:

```bash
npm run test:lighthouse
npm run measure:client
npm run test:client-budget
```

Run Lighthouse before client measurement in the same checkout: both measurement commands refresh fixtures, and Playwright clears `test-results` at startup. The CI workflow uses this order so the final client report is not deleted.

For collection-only diagnosis, `npm run measure:lighthouse` performs collect/upload without assertions. For final acceptance, `npm run test:lighthouse` performs three collections per route, median assertions, and filesystem report upload. `npm run test:client-budget` also runs the comparator unit tests before comparing the current report with the committed baseline.
