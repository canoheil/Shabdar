# Phase 14 — Launch Preparation: legal pages, clean URLs, chat polish, budget truth

**Date:** 2026-11-09

**Status:** Approved in conversation for implementation on `arena/14b1d475-shabdar`.
Scope is limited to the four items below. Notifications (Phase 15), deployment,
SMS provider, backup automation, and moderation rollout are explicitly out.

**Baseline:** `4ce5866` (merge of PR #22) — chat messenger layout, rating/comment
copy fixes, and home filter placeholders are already on `main`.

## 1. Intent

Move Shabdar from "feature complete in a sandbox" toward "ready to be shown to
real users" without adding a new product domain. Three of the four items close
recorded launch blockers (`docs/ARCHITECTURE.md` §9) or SEO gaps named in
`docs/PRODUCT.md` §3; the fourth removes a budget trap that will fail CI on the
next Home change.

## 2. Item A — Legal pages

**Problem.** There is no `privacy`, `terms`, `about` or `contact` route. The only
mention of data handling is a sentence inside `/advertising`.

**Decision.**

- Two static Nuxt pages: `/privacy` (حریم خصوصی) and `/terms` (قوانین استفاده).
- Content is Persian, SSR-rendered, `index, follow`, with a real `<h1>`, a
  visible «آخرین به‌روزرسانی» date, and sections as `<h2>`/`<p>`/`<ul>`.
- Content must state only behaviour that exists in this codebase: mobile number +
  OTP, session cookie, display name, listings/photos, comments/ratings/reports,
  private conversations (participants only), block relationships, media access
  control, admin audit log. No invented claims about retention periods, GDPR,
  or third parties — retention is still undecided (`PRODUCT.md` §9), so the text
  says retention is limited to operating the service and that deletion requests
  go through the contact channel.
- Footer in the shell gets links to both pages plus `/advertising`. Footer is
  shared markup, so the links cost no extra request and no extra JS chunk.
- No new dependency, no CMS, no markdown pipeline: plain SFC markup.

**Risks.** Legal text is not legal advice; the pages carry a visible line telling
the operator to have them reviewed before launch.

## 3. Item B — Clean URLs for category and city

**Problem.** `PRODUCT.md` §3 requires "Category pages and city pages with clean
slug URLs". Today everything is `/?category=…&city=…`, so a category or city has
no shareable, indexable address, and the sitemap cannot list them.

**Decision.**

- New routes: `/category/[slug]` and `/city/[slug]`.
- Both render **the same listing browser as Home** (hero-lite header instead of
  the full hero, same filters/search/pagination). One shared component, no
  duplicated feed logic: Home keeps its composition, the two new pages supply a
  fixed `category`/`city` base and hide the corresponding filter control.
- Unknown slug → `404` rendering the existing error page (server-rendered, not a
  silent empty feed). The check happens in SSR via the existing
  `/api/v1/categories` and `/api/v1/locations/provinces/{slug}/cities` data,
  never by trusting the client.
- `/city/[slug]` needs the province for the province filter label; the city
  reference data already returns `name_fa`, so the page fetches only the
  province's city list plus the provinces list. No new API endpoint.
- Canonical + `<title>` per page (`دستهٔ X | شبدر`, `آگهی‌های X | شبدر`), and the
  category/city page is **not** `noindex`.
- Internal links: Home category chips and the filter apply become real links to
  `/category/<slug>` when the category is the only active filter; the listing
  card's city becomes a link to `/city/<slug>`. Query-parameter URLs keep working
  (they are the pagination/filter state), so no redirect is required and no old
  link breaks.
- Category/city pages are included in the new sitemap (see Item B2).

### B2 — `sitemap.xml` and `robots.txt` (the dead endpoint fix)

`GET /api/v1/sitemap/listings` exists and has tests, but nothing consumes it.
Add two Nuxt server routes:

- `server/routes/sitemap.xml.ts`: pages through the API endpoint with a bounded
  page count, emits `<urlset>` with `/`, category pages, city pages (only cities
  that are referenced by an active listing response? — no: keep it simple and
  list the reference cities only when they have listings, decided below), and
  listing URLs. Cached with a short `max-age` and `s-maxage`.
- `server/routes/robots.txt.ts`: allow public routes, disallow `/me`, `/admin`,
  `/login`, and point at the sitemap absolute URL.
- Origin comes from a runtime config key (`NUXT_PUBLIC_SITE_URL`) with a
  localhost default, documented in the README.

**Scope guard (ponytail).** To avoid a new "which cities have listings" API, the
first sitemap lists `/`, `/category/*` and all listing URLs; city pages are added
only when a cheap source exists. Category pages are a fixed, small set and are
the higher-value index. City coverage is deferred to a follow-up that adds
`GET /api/v1/sitemap/locations` — recorded, not built here.

## 4. Item C — Chat polish and empty-message guard

**Problem.** The messenger layout landed in PR #22. Two gaps remain:

1. Client-side, a draft of only spaces / ZWNJ / zero-width characters passes
   `length > 0` and posts to the API, which rejects it — the user sees a server
   error instead of an obvious disabled send button.
2. Nothing pins the behaviour, so a later refactor can silently remove the
   avatar/time layout or reintroduce an optimistic bubble.

**Decision.**

- One shared predicate `isSendableDraft(value)` in `app/utils/chatFormat.ts`
  (trim + strip ZWNJ/ZWJ/zero-width marks/direction marks; a body is sendable
  when at least one non-whitespace character remains). The composer uses it to
  disable the send button and to short-circuit `submit()`; the character counter
  keeps counting raw scalars (that is what the server counts).
- Add a unit test file for the predicate (`node --test`, no new dependency) and
  extend the Playwright composer spec with a whitespace/ZWNJ case.
- Screenshot-free structural Playwright assertions for the messenger layout:
  own bubbles on the opposite side from other bubbles, one avatar per bubble, a
  `<time>` per bubble, and the block action present inside the header menu but
  absent from the conversation body.

## 5. Item D — Client-JS budget truth

**Problem.** `npm run test:client-budget` compares each route against
`docs/performance/phase7-client-baseline.json` (Home 89,027 / Detail 99,437) with
a +25,600 ceiling and an absolute 100,000 ceiling. Home is now 99,320, so the
next Home change fails CI even though Phase 12 recorded 94,982. The baseline is
two phases stale and the "delta" is meaningless.

**Decision.**

- Record a fresh measured baseline from the **production build of this branch**
  (`docs/performance/phase14-client-js-baseline.json`) using the existing
  measurement script, with `measuredHead` filled by the real commit.
- Point the npm script at the new baseline; keep the absolute 100,000-byte
  ceiling and the +25,600 delta rule untouched (they are the real guard).
- Add a guard test that the baseline file exists, has the expected keys, and is
  older-or-equal to the current measurement (no hand-edited numbers): a
  `node --test` case that fails if the baseline totals are not plausible
  (non-zero, under the ceiling).
- Measurement here runs against a local `node .output/server/index.mjs` with the
  same response-capture logic as CI; if the sandbox cannot produce it, the plan
  says so explicitly and CI produces the authoritative number.

**Honest limitation.** The local sandbox has no MariaDB/Rust/Chromium, so the
authoritative gzip numbers and the full Playwright suite run in CI. Anything not
run locally is marked as such in `STATUS.md`; no run is claimed that did not
happen.

## 6. Non-goals

No notifications, no WebSocket, no image pipeline changes, no new API domain,
no CSS framework, no CMS, no redirects of existing query URLs, no changes to
moderation, favourites, reports or chat semantics.

## 7. Acceptance criteria

- `npm run typecheck` and production build pass.
- `node --test` unit suites pass, including the new draft-predicate and baseline
  guard tests.
- `/privacy`, `/terms` render in SSR, are linked from every page footer, pass the
  existing quality spec at 375/768/1024/1440 in both themes.
- `/category/<known>` and `/city/<known>` render a filtered feed in SSR with a
  canonical and a real title; unknown slugs return `404`; the existing query-URL
  feeds still work unchanged.
- `/sitemap.xml` and `/robots.txt` return `200`, are same-origin, list only
  public routes, and are covered by a spec.
- Composer send is disabled for whitespace/ZWNJ-only drafts and no request is
  made; the messenger structure assertions pass.
- Public route client JS stays under 100,000 gzip bytes with the refreshed
  baseline, and no chat marker appears in a public route script.
