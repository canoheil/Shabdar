# Visual Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every page of `apps/web` the approved «گرم و خانگی» look without changing
behaviour, routes, data or any CI gate.

**Architecture:** Presentation only. Tokens and shared parts live in
`app/assets/css/tokens.css` and `main.css`; page composition stays in each SFC's scoped style;
icons come from one static SVG sprite referenced by a new `AppIcon` component, so they add no
client JS. Nine tasks follow the spec's build order, and each ends with one push and one green
run of `web-verification.yml`.

**Tech Stack:** Nuxt 4 (SSR with inlined styles), Vue 3 SFCs, plain CSS custom properties,
Phosphor Icons 2.1.1 (MIT) as a committed sprite, `node --test`, Playwright with axe,
Lighthouse CI. No new dependency.

**Spec:** `docs/superpowers/specs/2026-11-14-visual-refresh-design.md` (approved 2026-11-14),
with its boards in `design-system/shabdar/refresh-2026-11/`. Read it first: this plan cites its
sections and does not repeat its tables.

**Branch:** `arena/76915f79-shabdar` only. Never rename it or force-push; never open, merge or
close a PR unless the owner asks.

Paths below are relative to `apps/web/` unless they start with `docs/` or `design-system/`.
Line numbers are as of `fda97b0`.

## Global Constraints

- Files that may change: `app/**`, `tests/**`, `scripts/build-icon-sprite.mjs` (new), the two
  `node --test` file lists in `package.json`, `design-system/shabdar/MASTER.md`,
  `docs/STATUS.md`, the spec's status lines and this plan. Nothing else: no API, migration,
  server route, `nuxt.config.ts`, workflow, `lighthouserc.cjs` or budget script.
- No new dependency: `dependencies`, `devDependencies` and `package-lock.json` do not change.
- Gates stay as they are: client JS ≤ 104,000 gzip bytes per route; Lighthouse mobile medians
  LCP ≤ 2700 ms, CLS ≤ 0.1, TBT ≤ 200 ms, accessibility ≥ 0.95; `quality.spec.ts` (no serious
  axe violation, no horizontal overflow, every target ≥ 44×44 px, on 18 routes × 4 widths ×
  2 themes).
- A red gate stops the work. Report the numbers to the owner and wait. Never edit a ceiling,
  skip a test, or re-run until green.
- Existing class names, ids, `data-*` attributes, accessible names, labels and messages stay.
  The only edits to existing tests are the rows of "Test edits".
- CSS first. A template changes only to add, remove or wrap an element, or where a task says
  so. A new disclosure is a native `<details>`. No new client state except `listing-day-now`
  (Task 3).
- Never wrap existing text that a test finds by its exact words. Playwright's exact-text
  locator returns the innermost element that holds the text, so a new wrapper takes the place
  of the element the test meant, and a check of focus, role or class then reads the wrapper.
  Before a template gains an element around text, search `tests/e2e` for that text. Most of
  these specs need the real stack and cannot run locally, so only CI shows the failure
  (found in Task 2: run 43701510977, `comments.spec.ts:124`). A scan of the suite on
  2026-11-15 found one place where an element found by text is then checked for focus, a
  class, an attribute or a style: `comments.spec.ts:122-124`.
- Inside an `AppAlert`, the tone icon is pinned to the first line. The slot is a direct child
  of the alert, so a paragraph that opens it has `margin: 0`, and the first line of text
  starts at the top. Today a row of text and a button (`.browser__error`, `.inbox__state`,
  `.thread__state`, `.favorites__error`, `.blocks-page__error`, `.detail__contact-state`, the
  admin `__error` rows) centres its text against the 46px button from 768px, which puts the
  text 8px below the icon. The task that restyles such a row puts icon and text on one line
  again: the button goes under the text, or the row aligns to its start.
- Scale (spec §3): weights 400, 600, 800; font sizes only
  `--font-size-12/14/16/18/22/28/36/display`; radii only
  `--radius-small/control/card/panel/dialog/pill`, `50%` or `0`; `box-shadow` only
  `--shadow-card`, `--shadow-lifted`, `--glow-primary`, `--glow-accent`, `--ring-focus` or `none`.
- Icons: only `<AppIcon>`. Names are written as whole literals, never built from parts. Every
  icon is `aria-hidden`; an icon-only control keeps a real accessible name, as visually hidden
  text or `aria-label`. No emoji, no text glyph, no inline `<svg>` except `BrandMascot.vue`.
- Colour: saffron is a fill (`--color-accent-fill` with `--color-on-accent-fill`), never text.
  A status always has an icon and a word.
- Motion: CSS only, with the existing `--duration-*` and `--ease-out`; hover effects inside
  `@media (hover: hover)`; nothing loops or moves surrounding layout; the global
  `prefers-reduced-motion` rule in `main.css` stays.
- RTL: logical properties only. Directional icons are picked for RTL: back `arrow-right-bold`,
  row caret `caret-left-bold`, previous `caret-right-bold`, next `caret-left-bold`.
- Breakpoints: 700, 1024 and 1280px. Content column: 820px on form, account and message
  pages; 75rem on browsing pages.
- State where each result comes from: "local" (typecheck, build, unit tests, stub-API checks)
  or "CI run <id>". A local browser check is an approximation, never a gate verdict.

## Review Focus

Five conditions the spec implies and no existing test covers, most likely first. Each has its
pinning test in the task that owns the code.

1. **Photos that are not 4:3** (portrait phone shots, panoramas). Expected: never stretched; a
   card shows a centred square crop; the listing page shows the whole photo (decision D1).
   Pinned in Task 3 (card) and Task 4 (gallery).
2. **Long text in the two-column phone card** (165px wide at 375px, 138px at 320px): a
   twelve-digit price (the API has no upper bound), an unbroken title. Expected: text wraps
   inside the card; no horizontal scroll; nothing clipped sideways. Pinned in Task 3.
3. **The icon sprite is slow or blocked.** Expected: labels, sizes, names and focus are
   unaffected; only the glyphs are missing. Pinned in Task 2.
4. **The device clock or timezone differs from the server's, or a request spans Tehran
   midnight.** Expected: no hydration mismatch; the day label is the server's at first paint.
   Pinned in Task 3.
5. **Keyboard-only use of the two new disclosures** («گزارش و ایمنی» and the comment menu).
   Expected: the summary takes focus, Enter or Space opens it, the controls inside follow in
   order. Pinned in Task 4.

---

## Corrections to the spec, found while planning

Where the spec and this section differ, this section wins. The owner approved the plan with
this table on 2026-11-15; the rows a person can see or must know about (C1, C3 to C7, C9) were
also spelled out in the conversation with the Task 1 report.

| # | Spec says | Found | This plan |
|---|---|---|---|
| C1 | Baseline `fda97b0` | `main` is red since PR #25: `shell-ssr.spec.ts:31` fails with a strict-mode violation, because two buttons are named «جستجو» (run 43628236875: 167 passed, 1 failed). Lighthouse is skipped when Playwright fails, so that commit has no Lighthouse verdict. | Task 1 fixes the test and records a green baseline before any redesign. Executing it showed a second defect behind the first: line 46 expects the link «صف بررسی» by role at 375px, where admin entries are `display: none` (`navigation.spec.ts:109` pins count 0). CI never reached that line. |
| C2 | §9 lists five test edits | Four more existing assertions pin things the spec changes. | "Test edits" rows T2, T3, T5 and T7. |
| C3 | §5: sidebar 264px | `navigation.spec.ts:93` pins 272px. | The sidebar stays `17rem`. |
| C4 | §7.3, §11.2: detail cover uses `object-fit: cover` | `detail.spec.ts:30` pins `contain`. With `cover`, a portrait photo loses about half its height in a 4:3 frame, and the page has no zoom. | Owner decision D1 (2026-11-15): the whole photo, `contain`. |
| C5 | §7.3: foldout after the description | The approved board shows it right under the actions. | It stays where `.detail__reports` is today. |
| C6 | §7.3: title under the gallery | `detail.spec.ts:74-97` pins gallery-above-summary on phones and equal tops from 1024px. | The title block moves into `.detail__summary` (Task 4). |
| C7 | §7.3: the foldout wraps "seller block controls" | The page has no seller block control, and §11.5 forbids new features. | The foldout wraps the two report controls only. |
| C8 | §7.1: category above the title | `admin-errors.spec.ts:62` pins the card link's exact accessible name. | Markup stated in Task 3. |
| C9 | §7.10: the error page shows Shabdarak | `admin-errors.spec.ts:67,80,104` allow it on 404 only, as today. | The `status === 404` condition stays. |
| C10 | §4: regular, fill and duotone weights | The mockups also draw small glyphs (carets, check, x) in bold. | A symbol id may end in `-bold`. |
| C11 | §13.3: "two shadows and two glows" | The field focus halo of §6 is a third `box-shadow` value. | Token `--ring-focus`. |
| C12 | §6: the skeleton's "existing animation stays" | The old CSS and the component have no animation, and §3 says nothing loops (found while executing Task 2). | The skeleton stays static. |

## Baseline

| | Home | Detail | Source |
|---|---|---|---|
| Client JS, gzip bytes | 99,578 | 100,464 | CI run 43616929450 on `9ba038e`, the last green run (before PR #25) |
| Client JS, local | 95,643 | 96,522 | headless Chromium and a stub API on `fda97b0`; an approximation |
| Client JS after Task 1 | 95,634 | 96,516 | CI run 43691418582 on `a5e2da6`; the row every later task is compared with is in `docs/STATUS.md` |
| LCP median | 2404.4 ms, `h1` in `.home__intro` | 2556.7 ms, gallery image | CI run 43616929450 |
| CLS median | 0.0727 | 0.0022 | same |
| TBT median | 122 ms | 86 ms | same |
| Accessibility | 1.00 | 1.00 | same |
| Inline CSS bytes | 33,341 | 33,340 | local, `fda97b0`, smoke stub (command in "Stage gate") |

Two numbers leave little room: Detail LCP (143 ms under the ceiling) and Home CLS (0.027
under). Home's shift happens when Vazirmatn replaces the fallback font after `load`
(`plugins/font-enhancement.client.ts`), so a first-screen block whose height depends on how
text wraps adds to it. A rise of inline CSS above 10% (Home 36,675; Detail 36,674) is
investigated before the push.

## File structure

| File | Responsibility after this plan |
|---|---|
| `app/assets/css/tokens.css` | Every token of spec §3, both themes. The only file with raw colours, sizes, radii and shadows. |
| `app/assets/css/main.css` | Base elements, the shell, buttons, fields, alerts, empty state, skeleton, `.card`, `.avatar`, `.app-icon`, and the listing-card, listing-gallery and error-page blocks that already live there. |
| `app/utils/icons.ts` (new) | `ICON_NAMES`, `AppIconName`, `categoryIcon`. Pure module, importable by `node --test`. |
| `app/assets/icons/sprite.svg` (new) | Generated; one `<symbol>` per name. Never edited by hand. |
| `scripts/build-icon-sprite.mjs` (new) | Rebuilds the sprite from `ICON_NAMES` and an unpacked `@phosphor-icons/core@2.1.1` kept outside the repository. |
| `app/components/AppIcon.vue` (new) | The only place that renders `<svg><use>`. |
| `app/components/ShellIcon.vue` | Deleted in Task 2. |
| `app/utils/format.ts` | Gains `formatListingDay`. |
| `app/utils/images.ts` | `LISTING_CARD_IMAGE_SIZES` for the new grid. |
| SFCs under `app/pages`, `app/components`, `app/layouts` | Scoped composition only, written with tokens. |
| `tests/unit/icon-sprite.test.mjs`, `design-system.test.mjs`, `listing-format.test.mjs` (new) | Local red/green for sprite contents, the scale, the day label and category icons. |
| `design-system/shabdar/MASTER.md` | Rewritten in Task 9. |

The mockup pages that produced the boards are in
`quality-artifacts/design-review/mockup/` in this workspace only (gitignored). Where the spec
gives no value, take it from the boards or that mockup; where they disagree, the spec and this
plan win.

## Stage gate

Run at the end of every task, from `apps/web`.

```bash
npm run typecheck          # exit 0
npm run test:unit          # 0 failures
npm run build              # ends with "Build complete"
for port in 8099 3105; do curl -s -o /dev/null -m 2 "http://127.0.0.1:$port/" && { echo "port $port is busy"; exit 1; }; done
SMOKE_STUB_PORT=8099 node scripts/smoke-stub-api.mjs & STUB=$!
NUXT_API_BASE=http://127.0.0.1:8099 HOST=127.0.0.1 PORT=3105 node .output/server/index.mjs & WEB=$!
trap 'kill $WEB $STUB' EXIT
until curl -sf http://127.0.0.1:3105/api/v1/health >/dev/null; do kill -0 $WEB || exit 1; sleep 0.5; done
node --input-type=module -e '
for (const path of ["/", "/listings/" + encodeURIComponent("دوچرخه-کوهستان")]) {
  const html = await (await fetch("http://127.0.0.1:3105" + path)).text()
  const css = [...html.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].reduce((n, m) => n + Buffer.byteLength(m[1]), 0)
  console.log(decodeURIComponent(path), "inline-css", css)
}'
```

Then commit, `git push origin arena/76915f79-shabdar`, and read the verdict. A run takes about
22 minutes. Raw logs and artifacts cannot be downloaded from this sandbox; annotations can.

```bash
RUN=$(gh run list --branch arena/76915f79-shabdar --workflow web-verification.yml --limit 1 --json databaseId --jq '.[0].databaseId')
gh run view "$RUN" --json status,conclusion,headSha,jobs --jq '{status, conclusion, sha: .headSha[0:7], red: [.jobs[0].steps[] | select(.conclusion != "success" and .conclusion != "skipped") | .name]}'
JOB=$(gh run view "$RUN" --json jobs --jq '.jobs[0].databaseId')
gh api "repos/canoheil/Shabdar/check-runs/$JOB/annotations?per_page=100" > /tmp/annotations.json
node -e '
for (const row of JSON.parse(require("fs").readFileSync("/tmp/annotations.json", "utf8"))) {
  const match = /Phase 7 (Lighthouse|client|LCP diagnostics) JSON/.exec(row.title ?? "")
  if (!match) { if (row.annotation_level === "failure") console.log("FAILURE", row.path, (row.message ?? "").slice(0, 400)); continue }
  const data = JSON.parse(Buffer.from(row.message, "base64").toString())
  if (match[1] === "client") console.log("client JS", data.home.totalGzip, data.detail.totalGzip)
  if (match[1] === "Lighthouse") for (const route of ["home", "detail"]) console.log(route, data[route].median)
  if (match[1] === "LCP diagnostics") console.log("LCP elements", data.map((entry) => entry.element?.selector))
}'
```

Green means: `status` is `completed`, `conclusion` is `success`, `sha` is the pushed commit and
`red` is empty. Then append one row to the "Visual refresh" table in `docs/STATUS.md` (task,
commit, run id, client JS, the four Lighthouse medians and the LCP element for Home and Detail,
inline CSS) and commit it; a docs-only push starts no run. Rebuild the owner's preview and say
which pages changed.

The inline-CSS lines run as one script, so the `exit` and the `trap` stay inside it. The
check for busy ports is there because a server left over from an earlier measurement once
answered in place of the fresh build.

The real-stack Playwright suite cannot run in this sandbox (no MariaDB, no Rust API). A spec
that creates no data can still run against the built app and the sample-data stub, with
hand-written fixture files in the gitignored `tests/e2e/.artifacts/` and a local config kept
outside the repository; Task 1 used that for a red and a green of `shell-ssr.spec.ts`. Such a
run is an approximation: the CI run on the real stack is the only verdict. Other end-to-end
edits are argued from the code before the push; unit tests give a real local red and green.

A task's restyling step lists only template changes and what the tests pin. The look itself is
the spec section named in the task's title, and the boards. A task that needs a new icon adds
its name to `ICON_NAMES` and re-runs `scripts/build-icon-sprite.mjs`; an undeclared name fails
the typecheck and an unused or missing one fails the unit test.

## Test edits

Every edit to an existing end-to-end test. "§9" marks the five the spec already lists.

| # | Where | Today | Becomes | Task |
|---|---|---|---|---|
| T1 | `shell-ssr.spec.ts:49` | Page-wide button «جستجو» | The same button inside the `ناوبری اصلی` landmark | 1 |
| T1b | `shell-ssr.spec.ts:46` | Role link «صف بررسی», count 1 | The same, with `includeHidden: true` (found while executing Task 1; see C1) | 1 |
| T2 | `shell-ssr.spec.ts:20-24` | Each action has one inline `svg path` | Each action's `svg[aria-hidden="true"] use` points at `/_nuxt/sprite.<hash>.svg#<id>`; the sprite answers 200 as `image/svg+xml` and holds every referenced id; exactly one icon per action is visible and its `<use>` has a non-empty box, so it really painted | 2 |
| T3 | `shell-ssr.spec.ts:47` | `.shell__nav-item--admin svg path` count 6 | `.shell__nav-item--admin svg use` count 6 | 2 |
| T4 | `theme.spec.ts:8-66` (§9) | 27 tokens per theme | The 39 rows of spec §3.1 per theme | 2 |
| T5 | `navigation.spec.ts:53-70` | At 768px the nav is not fixed and sits under the brand | At 768px the nav is fixed at the bottom, brand and theme control share one row, main bottom padding ≥ nav height | 2 |
| T6 | `home.spec.ts:350` (§9) | The card shows «بررسی‌شده برای انتشار» | That text has count 0 in the card | 3 |
| T7 | `home.spec.ts:381` | At 375px `currentSrc` contains `--w480.webp` | `--w240.webp` (a 165px image) | 3 |
| T8 | `reports.spec.ts:70,82,115,134,155,184` (§9) | Report controls are visible | The foldout or the comment menu is opened first | 4 |
| T9 | `blocks.spec.ts:109,131,171` (§9) | The block button is visible | The comment menu is opened first | 4 |
| T10 | `conversation-thread.spec.ts:360,403` (§9) | One `.bubble__avatar` per bubble | Count 0; the title says "time per bubble" | 7 |
| T11 | `detail.spec.ts:30` | `object-fit: contain` | Dropped: the owner chose `contain` in D1, so this line stays | none |

---

### Task 1: Green baseline

**Files:**
- Modify: `tests/e2e/shell-ssr.spec.ts:49`
- Modify: `docs/STATUS.md` (the "Visual refresh" section)

**Interfaces:**
- Produces: a table in `docs/STATUS.md` with one "baseline" row from a green run of this
  branch. Every later task appends its row.

- [x] **Step 1: Scope the ambiguous locator (T1) and match the hidden admin link (T1b)**

```ts
await page.getByRole('navigation', { name: 'ناوبری اصلی' }).getByRole('button', { name: 'جستجو', exact: true }).click()
```

`navigation.spec.ts:123` already uses this form.

```ts
// Admin entries are desktop-only: at this 375px viewport they are rendered but display: none.
await expect(page.getByRole('link', { name: 'صف بررسی', exact: true, includeHidden: true })).toHaveCount(1)
```

T1b was added during execution. With the spec run locally against the sample-data stub, the
unchanged file failed at line 49 exactly as in CI; with line 49 fixed it failed at line 46;
with both it passed. Nothing else in the file changes. The comment moves the later lines of
the file down by one.

- [x] **Step 2: Run the local part of the stage gate**

Expected: typecheck exit 0, 37 unit tests pass, inline CSS 33,341 and 33,340.

- [x] **Step 3: Commit and push**

```bash
git add apps/web/tests/e2e/shell-ssr.spec.ts
git commit -m "test: make the shell-ssr hydration test pass on the real shell"
git push origin arena/76915f79-shabdar
```

- [x] **Step 4: Read the CI verdict**

Expected: green, Playwright reports 168 passed. If any step is red here, stop: the baseline is
over a gate with no redesign in it, and the owner decides what happens next.

Result: run 43691418582 on `a5e2da6`, every step green, 168 passed.

- [x] **Step 5: Record the baseline row, commit and push**

```bash
git add docs/STATUS.md
git commit -m "docs: record the visual refresh baseline"
git push origin arena/76915f79-shabdar
```

### Task 2: Foundation — tokens, scales, icons, shared parts, shell (spec §3–§6)

**As executed (2026-11-15, commits `86738a0` and `92efcde`).** Five things differ from the
first version of these steps. The steps below are corrected to match; the reasons are in the
ledger.

- The end-to-end edits of Step 7 were written first and run red against the old build; the
  implementation followed. These specs run locally against the sample-data stub.
- The unit test that compared `tokens.css` with the `theme.spec.ts` tables was not written:
  `theme.spec.ts` checks the computed values in a browser, locally and in CI. The unit suite
  has 46 tests, not 47.
- T2 also requires that exactly one icon per action is visible and that its `<use>` has a
  non-empty box. The box is empty when the sprite is blocked or an id is missing.
- `main.css` lost the dead `.card__title a` rules as well, its two slow motions are multiples
  of `--duration-normal`, and an invalid field keeps its red edge under the pointer.
- `AppAlert` has no `div.app-alert__body`. The first version of Step 5 put the slot inside
  that wrapper, and CI run 43701510977 on `86738a0` went red: 168 passed, 1 failed.
  `comments.spec.ts:122-124` finds the failed-comment alert by its exact text and expects that
  element to hold focus; with the wrapper the locator found the wrapper. Commit `92efcde` puts
  the slot back as a direct child of the root and hangs the icon in the root's start padding.
  The rendered alerts are pixel-identical to the wrapper layout.

**Files:**
- Create: `app/utils/icons.ts`, `app/assets/icons/sprite.svg`, `app/components/AppIcon.vue`,
  `scripts/build-icon-sprite.mjs`, `tests/unit/icon-sprite.test.mjs`,
  `tests/unit/design-system.test.mjs`
- Delete: `app/components/ShellIcon.vue`
- Modify: `app/assets/css/tokens.css`, `app/assets/css/main.css`, `app/layouts/default.vue:11-79`,
  `app/components/AppThemeToggle.vue`, `AppAlert.vue`, `AppButton.vue:15`,
  `package.json` (`test:unit` and the `node --test` list inside `test:client-budget`), and the
  eleven SFCs that use `--shadow-xs`
- Test: `tests/e2e/theme.spec.ts:8-66` (T4), `tests/e2e/shell-ssr.spec.ts` (T2, T3, one new
  test), `tests/e2e/navigation.spec.ts:53-70` (T5)

**Interfaces:**
- Produces `ICON_NAMES` (an `as const` array of strings) and
  `type AppIconName = typeof ICON_NAMES[number]` in `app/utils/icons.ts`. A symbol id is the
  Phosphor file name: `house`, `house-fill`, `x-bold`, `tag-duotone`.
- Produces `<AppIcon name: AppIconName size?: number>` (default size 20).
- Produces `AppButton` variants `'primary' | 'secondary' | 'accent' | 'soft' | 'danger' |
  'danger-quiet'` with classes `.btn--accent`, `.btn--soft`, `.btn--danger-quiet`. `danger`
  stays the filled confirming button.
- Produces every token of spec §3 plus the values below, and the global classes `.app-icon`
  and `.avatar` (`.avatar--b` for the viewer's own avatar).
- Produces `SCOPE` in `tests/unit/design-system.test.mjs`; every later task appends its files.

- [x] **Step 1: Write the two failing unit tests**

`tests/unit/icon-sprite.test.mjs`:

```js
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { test } from 'node:test'
import { ICON_NAMES } from '../../app/utils/icons.ts'

const WEB = new URL('../../', import.meta.url).pathname
const sprite = readFileSync(`${WEB}app/assets/icons/sprite.svg`, 'utf8')
const symbols = [...sprite.matchAll(/<symbol id="([a-z0-9-]+)" viewBox="0 0 256 256">/g)].map((match) => match[1])

test('the sprite holds exactly the declared icons', () => {
  assert.deepEqual(symbols, [...ICON_NAMES].sort())
})

test('every declared icon is written as a literal somewhere in app/', () => {
  const sources = readdirSync(`${WEB}app`, { recursive: true })
    .filter((name) => /\.(vue|ts)$/.test(name))
    .map((name) => readFileSync(`${WEB}app/${name}`, 'utf8'))
    .join('\n')
  // One occurrence is the declaration in icons.ts, so a used icon appears at least twice.
  const unused = ICON_NAMES.filter((name) => sources.split(new RegExp(`['"\`]${name}['"\`]`)).length < 3)
  assert.deepEqual(unused, [])
})

test('the sprite is plain local geometry under 48 KB', () => {
  assert.doesNotMatch(sprite, /<(script|style|image|foreignObject)\b|href=|url\(/i)
  assert.ok(Buffer.byteLength(sprite) < 48_000)
})
```

`tests/unit/design-system.test.mjs`:

```js
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { test } from 'node:test'

const WEB = new URL('../../', import.meta.url).pathname
// Files already on the new scale. Each task appends its files; Task 9 replaces the list with ['app/'].
const SCOPE = ['app/assets/css/main.css', 'app/layouts/', 'app/components/App', 'app/error.vue']

const files = readdirSync(`${WEB}app`, { recursive: true })
  .map((name) => `app/${name}`)
  .filter((name) => /\.(vue|css)$/.test(name))
const scoped = files.filter((name) => name !== 'app/assets/css/tokens.css' && SCOPE.some((prefix) => name.startsWith(prefix)))
const read = (name) => readFileSync(`${WEB}${name}`, 'utf8')
const css = (name) => (name.endsWith('.css') ? read(name) : [...read(name).matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((match) => match[1]).join('\n'))
  .replace(/\/\*[\s\S]*?\*\//g, '')
const template = (name) => (name.endsWith('.vue') ? read(name).split(/<script[\s>]/)[0] : '')
const values = (name, property) => [...css(name).matchAll(new RegExp(`(?<![\\w-])${property}\\s*:\\s*([^;}]+)`, 'g'))].map((match) => match[1].trim())

const RADIUS = /^(var\(--radius-(small|control|card|panel|dialog|pill)\)|50%|0|inherit)$/
const SHADOW = /^(var\(--(shadow-card|shadow-lifted|glow-primary|glow-accent|ring-focus)\)|none|inherit)$/
const RULES = [
  ['font-weight', (value) => /^(400|600|800|inherit)$/.test(value)],
  ['font-size', (value) => /^(var\(--font-size-(12|14|16|18|22|28|36|display)\)|inherit)$/.test(value)],
  ['font', (value) => value === 'inherit'],
  // The chat bubble's one tight corner is the only raw radius (spec §13.3).
  ['border(?:-[a-z]+)*-radius', (value, name) => value.split(/[\s/]+/).every((part) => RADIUS.test(part) || (part === '6px' && name.endsWith('MessageBubble.vue')))],
  ['box-shadow', (value) => value.split(/,\s*(?![^(]*\))/).every((part) => SHADOW.test(part.trim()))],
]

for (const [property, allowed] of RULES) {
  test(`${property} uses only the scale`, () => {
    const offenders = scoped.flatMap((name) => values(name, property).filter((value) => !allowed(value, name)).map((value) => `${name}: ${value}`))
    assert.deepEqual(offenders, [])
  })
}

test('no glyph or emoji stands in for an icon, and no inline svg outside AppIcon and BrandMascot', () => {
  const offenders = scoped.flatMap((name) => {
    const markup = template(name)
    const glyphs = markup.match(/[★☆→←⋮✓✔]|\p{Extended_Pictographic}/gu) ?? []
    const inlineSvg = /<svg[\s>]/.test(markup) && !/(AppIcon|BrandMascot)\.vue$/.test(name)
    return [...glyphs.map((glyph) => `${name}: ${glyph}`), ...(inlineSvg ? [`${name}: <svg>`] : [])]
  })
  assert.deepEqual(offenders, [])
})
```

- [x] **Step 2: Register both files and see them fail**

Append the two paths to the `node --test` list in `test:unit` and to the `node --test` list
at the start of `test:client-budget`. Change nothing else in those scripts.

Run: `npm run test:unit`
Expected: FAIL. `icon-sprite` cannot import `icons.ts`; `design-system` lists the off-scale
declarations of `main.css` and the inline `<svg>` in `layouts/default.vue`. As run: 44 tests,
6 failed.

- [x] **Step 3: Build the icon system**

`app/utils/icons.ts` exports `ICON_NAMES` and `AppIconName`. This task's names: `house`,
`house-fill`, `magnifying-glass`, `plus-circle-fill`, `chat-circle`, `chat-circle-fill`,
`heart`, `heart-fill`, `user-circle`, `user-circle-fill`, `shield-check`, `sun`, `moon`,
`warning-circle-fill`, `check-circle-fill`, `warning-fill`, `info-fill`.

`scripts/build-icon-sprite.mjs <dir of @phosphor-icons/core@2.1.1>` imports `ICON_NAMES`
from `../app/utils/icons.ts`, sorts it, reads `<dir>/assets/<weight>/<id>.svg` (weight from
the `-bold`, `-fill` or `-duotone` suffix, otherwise `regular`), keeps each file's inner
markup, and writes `<symbol id="<id>" viewBox="0 0 256 256">` elements inside one
`<svg xmlns="http://www.w3.org/2000/svg">`, with a first-child comment naming Phosphor 2.1.1
and `licenses/phosphor-icons-MIT.txt`. Unpack the package outside the repository; it is not
added to `package.json`.

`app/components/AppIcon.vue`, exactly (the origin strip was found by building a spike: without
it the client rewrites every `href` during hydration, and without `?no-inline` a small sprite
becomes a `data:` URL that `<use>` cannot read):

```vue
<template>
  <svg class="app-icon" :width="size" :height="size" aria-hidden="true" focusable="false">
    <use :href="`${sprite}#${name}`" />
  </svg>
</template>

<script setup lang="ts">
import spriteUrl from '~/assets/icons/sprite.svg?no-inline'
import type { AppIconName } from '~/utils/icons'

// The client build resolves the asset to an absolute URL and the server to a
// root-relative one. Strip the origin so both render the same attribute.
const sprite = spriteUrl.replace(/^https?:\/\/[^/]+/, '')

withDefaults(defineProps<{ name: AppIconName; size?: number }>(), { size: 20 })
</script>
```

`.app-icon { fill: currentColor; flex: none; }` goes in `main.css`.

- [x] **Step 4: Write the tokens and the 39-row tables (T4)**

`tokens.css` gets every colour of spec §3.1 in both themes, and these values, which the spec
names without numbers:

```css
--font-size-12: 0.75rem;  --font-size-14: 0.875rem; --font-size-16: 1rem;   --font-size-18: 1.125rem;
--font-size-22: 1.375rem; --font-size-28: 1.75rem;  --font-size-36: 2.25rem; --font-size-display: 2.625rem;
--radius-small: 10px; --radius-control: 14px; --radius-card: 20px; --radius-panel: 28px; --radius-dialog: 28px; --radius-pill: 999px;
--shadow-card: 0 1px 2px rgb(74 52 20 / 0.06), 0 8px 22px -10px rgb(74 52 20 / 0.18);
--shadow-lifted: 0 2px 6px rgb(74 52 20 / 0.08), 0 20px 40px -14px rgb(74 52 20 / 0.3);
--glow-primary: 0 8px 18px -10px var(--color-primary);
--glow-accent: 0 8px 18px -10px var(--color-accent-fill);
--ring-focus: 0 0 0 4px var(--color-primary-soft);
/* :root.dark */
--shadow-card: 0 1px 2px rgb(0 0 0 / 0.35), 0 8px 22px -10px rgb(0 0 0 / 0.6);
--shadow-lifted: 0 2px 6px rgb(0 0 0 / 0.4), 0 20px 40px -14px rgb(0 0 0 / 0.75);
```

Write every colour as six-digit hex. Delete `--shadow-xs`, `--shadow-nav`, `--color-on-dark` and
`--color-on-dark-muted`; in the eleven SFCs that still use `--shadow-xs`, replace it with
`--shadow-card` so nothing dangles. Replace the `LIGHT` and `DARK` tables of `theme.spec.ts`
with the 39 rows of spec §3.1 (the 27 existing names and the 12 new ones).

- [x] **Step 5: Rewrite `main.css` and the shell to spec §3.3, §5 and §6**

Rewrite, do not append: every declaration in `main.css` uses the scale, and the unused
`.alert-error`, `.alert-ok` and `.empty` rules go. The base `h1` is 28px/800 with a muted lead
under it, so an inner page needs no header card. These are pinned by tests and must hold:

- `<nav class="shell__nav" aria-label="ناوبری اصلی">` keeps exactly that class attribute, and
  each action keeps one text `<span>` (`shell-ssr.spec.ts:16`, `navigation.spec.ts:27,32`).
- Below 1024px the nav is `position: fixed` at the bottom and opaque. `.shell__main` and
  `.shell__footer` both keep a `padding-block-end` of at least the nav's height
  (`navigation.spec.ts:46-47`), and `html` gets the same `scroll-padding-block-end`
  (`quality.spec.ts:149-165` focuses the search at 844×390).
- From 1024px the header is `position: fixed` and `17rem` wide (`navigation.spec.ts:93`);
  admin items show only there (`navigation.spec.ts:109-110`).
- `.shell__brand` keeps the mark with `color: var(--color-primary)` (`brand.spec.ts:55-61`).
- The loading spinner stays `.app-button[aria-busy='true']::before` with
  `@keyframes app-button-spin` (`post.spec.ts:105-116`).
- `:focus-visible` keeps an outline of at least 2px (`quality.spec.ts:211-216`).
- A select's caret is two `linear-gradient` backgrounds in `currentColor`: no data URL, no
  wrapper element.
- An alert's words are direct children of the element that takes focus
  (`comments.spec.ts:122-124`).

Template changes: in `default.vue`, each nav link renders its regular and its `-fill` icon and
CSS shows one by `[aria-current='page']`; «جستجو» and the admin rows render one icon; «ثبت»
renders `plus-circle-fill`; the inline bubble `<svg>` and `p.shell__sidebar-note` go.
`AppThemeToggle` keeps its label and select and gains `sun` and `moon`, one shown by
`html.dark`; below 1024px the select lies transparent over a 44px round button. `AppAlert`
renders its tone icon (`warning-circle-fill`, `check-circle-fill`, `warning-fill`,
`info-fill`) before the slot, which stays a direct child of the root: the root has
`position: relative` and a start padding that holds the absolutely placed icon. `AppButton`
gains the three variants. Delete `ShellIcon.vue`.

If the CI run reports a serious axe violation on the transparent select, redraw it as a
visible pill select with the same markup and tell the owner. Run 43704168232 reported none.

- [x] **Step 6: Run the unit tests**

Run: `npm run test:unit`
Expected: PASS, 46 tests.

- [x] **Step 7: Edit the shell tests (T2, T3, T5) and add the sprite test**

In `shell-ssr.spec.ts`, the label loop of the no-JavaScript test (lines 20-24) becomes:

```ts
const SPRITE = /^\/_nuxt\/sprite\.[\w-]+\.svg#([a-z0-9-]+)$/

const hrefs: string[] = []
for (const label of actions) {
  const action = navigation.locator('.shell__nav-action').filter({ hasText: label })
  const found = await action.locator('svg[aria-hidden="true"] use').evaluateAll((nodes) => nodes.map((node) => node.getAttribute('href') ?? ''))
  expect(found.length, label).toBeGreaterThan(0)
  await expect(action.locator('svg:not([aria-hidden="true"])')).toHaveCount(0)
  // The shown icon really paints: an external <use> has a box only once the sprite loaded and its id resolved.
  const shown = action.locator('svg:visible use')
  await expect(shown, label).toHaveCount(1)
  await expect.poll(() => shown.evaluate((node) => (node as SVGGraphicsElement).getBBox().width), { message: label }).toBeGreaterThan(0)
  hrefs.push(...found)
}
for (const href of hrefs) expect(href).toMatch(SPRITE)
const sprite = await request.get(hrefs[0]!.split('#')[0]!)
expect(sprite.status()).toBe(200)
expect(sprite.headers()['content-type']).toContain('image/svg+xml')
const body = await sprite.text()
for (const href of hrefs) expect(body).toContain(`<symbol id="${href.split('#')[1]}" `)
```

In the hydration test, line 47 counts `.shell__nav-item--admin svg use` (still 6), and after the
click every `use` inside the nav still has an `href` matching `SPRITE`, which proves hydration
did not rewrite it. Add one test (it imports `expectNoOverflow` and `expectTouchTargets` from
`./support/quality`):

```ts
test('the shell keeps its labels, names and sizes when the sprite cannot load', async ({ page }) => {
  await page.route(/\/_nuxt\/sprite\.[\w-]+\.svg$/, (route) => route.abort())
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/')
  const navigation = page.getByRole('navigation', { name: 'ناوبری اصلی' })
  for (const label of actions) await expect(navigation.getByText(label, { exact: true })).toBeVisible()
  await expect(page.getByRole('combobox', { name: 'پوسته', exact: true })).toBeVisible()
  await expect(page.getByRole('searchbox', { name: 'جستجو', exact: true })).toBeVisible()
  await expectTouchTargets(page)
  await expectNoOverflow(page)
})
```

In `navigation.spec.ts`, the tablet test (lines 53-70) becomes:

```ts
test('@navigation tablet keeps the docked tab bar under a one-row top bar', async ({ page }) => {
  await page.setViewportSize({ width: 768, height: 1024 })
  await page.goto('/')
  // Measure .shell__brand, .shell__theme, .shell__nav and .shell__main the way lines 33-44 do, then:
  expect(nav.position).toBe('fixed')
  expect(nav.bottom).toBeCloseTo(viewportHeight, 0)
  expect(theme.top < brand.bottom && brand.top < theme.bottom).toBe(true)
  expect(mainPaddingBottom).toBeGreaterThanOrEqual(nav.height)
  await expect(navigation(page).locator('.shell__nav-action:visible')).toHaveCount(6)
  await expectNoOverflow(page)
})
```

- [x] **Step 8: Run the stage gate, commit and push**

Extra local check: the SSR document of `/` contains `<use href="/_nuxt/sprite.` and no
`data:image/svg`. This is the first run with the sprite request, so compare Detail's LCP with
the baseline row before going on.

Verdict: run 43701510977 on `86738a0` was red (the alert wrapper, above). Run 43704168232 on
`92efcde` is green: Playwright 169 passed; client JS 94,736 / 95,619; LCP 2405.1 / 2556.6 ms;
CLS 0.0789 / 0.0021; TBT 62 / 50.5 ms; accessibility 1.00 / 1.00. Detail's LCP is about
3 ms above the baseline. Home's CLS rose by 0.0062; Task 3 owns it.

```bash
git add apps/web/app apps/web/scripts/build-icon-sprite.mjs apps/web/tests apps/web/package.json
git commit -m "feat: warm tokens, icon sprite, shared parts and the new shell"
git push origin arena/76915f79-shabdar
```

### Task 3: Home, category and city pages (spec §7.1, §7.2)

**Files:**
- Modify: `app/pages/index.vue`, `app/components/ListingBrowser.vue:2-29,154-171,173-314`
  and its style, `app/components/ListingCard.vue:44-71`, `HomeAdvertisingCard.vue`,
  `app/pages/category/[slug].vue:3-22`, `app/pages/city/[slug].vue`,
  `app/utils/format.ts`, `app/utils/images.ts:3`, `app/utils/icons.ts`,
  `app/assets/css/main.css` (listing-card block), the sprite
- Create: `tests/unit/listing-format.test.mjs`
- Test: `tests/e2e/home.spec.ts` (T6, T7, three new tests)

**Interfaces:**
- Consumes: `AppIcon`, `AppIconName`, the tokens and button variants of Task 2.
- Produces `formatListingDay(value: string, now: number): string` in `app/utils/format.ts`.
- Produces `categoryIcon(slug: string): AppIconName` in `app/utils/icons.ts`.
- Produces the `intro` slot and the `emeraldHero?: boolean` prop (default `false`) of
  `ListingBrowser`, and the shared state key `listing-day-now` (`useState<number>`), which
  Task 4 reads.

- [ ] **Step 1: Write the failing unit test**

`tests/unit/listing-format.test.mjs`:

```js
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { formatListingDate, formatListingDay } from '../../app/utils/format.ts'
import { categoryIcon } from '../../app/utils/icons.ts'

const NOW = Date.parse('2026-11-14T12:00:00Z') // 15:30 in Tehran (UTC+03:30, no DST)
const cases = [
  ['the same Tehran day', '2026-11-14T08:10:00Z', NOW, 'امروز'],
  ['the first minute of the Tehran day', '2026-11-13T20:30:00Z', NOW, 'امروز'],
  ['the last second of the previous Tehran day', '2026-11-13T20:29:59Z', NOW, 'دیروز'],
  ['two days', '2026-11-12T12:00:00Z', NOW, '۲ روز پیش'],
  ['six days', '2026-11-08T12:00:00Z', NOW, '۶ روز پیش'],
  ['seven days falls back to the date', '2026-11-07T12:00:00Z', NOW, formatListingDate('2026-11-07T12:00:00Z')],
  ['a timestamp ahead of the clock', '2026-11-15T09:00:00Z', NOW, 'امروز'],
  ['31 minutes ago across Tehran midnight', '2026-11-14T20:00:00Z', Date.parse('2026-11-14T20:31:00Z'), 'دیروز'],
  ['an unparseable value', 'not-a-date', NOW, ''],
]
for (const [label, value, now, expected] of cases) {
  test(`day label: ${label}`, () => assert.equal(formatListingDay(value, now), expected))
}

test('category icons cover the seven seeded slugs, the "all" tile and a slug added later', () => {
  const expected = {
    '': 'squares-four-duotone', 'home-sweets': 'cake-duotone', 'home-food': 'bowl-food-duotone',
    handicrafts: 'palette-duotone', 'clothing-sewing': 't-shirt-duotone', 'plants-flowers': 'plant-duotone',
    beauty: 'flower-lotus-duotone', education: 'graduation-cap-duotone', 'a-slug-added-later': 'tag-duotone',
  }
  for (const [slug, icon] of Object.entries(expected)) assert.equal(categoryIcon(slug), icon)
})
```

- [ ] **Step 2: Register it, extend `SCOPE`, and see both fail**

Add the file to both `node --test` lists. Append to `SCOPE`: `app/pages/index.vue`,
`app/pages/category/`, `app/pages/city/`, `app/components/ListingBrowser.vue`,
`app/components/ListingCard.vue`, `app/components/HomeAdvertisingCard.vue`.

Run: `npm run test:unit`
Expected: FAIL: `formatListingDay` and `categoryIcon` are not exported; scale offenders in the
six files; `★` in `ListingCard.vue`.

- [ ] **Step 3: Implement `formatListingDay` and `categoryIcon`**

`formatListingDay(value: string, now: number): string` goes next to `formatListingDate` in
`app/utils/format.ts`. It compares the two calendar days as seen in `Asia/Tehran`, taken from
one module-level `Intl.DateTimeFormat` through `formatToParts`; it never reads the device
timezone. `categoryIcon` is a lookup with the nine values of the test; add those names to
`ICON_NAMES`.

- [ ] **Step 4: Restructure the browser and Home**

- `ListingBrowser`: a new `div.browser__hero` (class `browser__hero--emerald` when
  `emeraldHero`) wraps `<slot name="intro" />` and the existing `form#search`, whose children
  stay as they are. The placeholder becomes «دنبال چه می‌گردید؟». The submit and «فیلترها»
  buttons gain `magnifying-glass-bold` and `sliders-horizontal-bold`; their text stays in the
  button, visually hidden where the spec shows an icon button. Each category link gains
  `<AppIcon :name="categoryIcon(slug)" />` in a tinted square, tints a, b, c in turn.
- `index.vue`: Home passes `emerald-hero`. `header.home__intro` moves into
  `<template #intro>` and keeps the eyebrow (with `map-pin-fill`), the `h1` and the lead. Add
  `<BrandMascot pose="full" class="home__mascot" />` with a one-shot `mascot-greet` keyframe
  (transform only). `.home__hero`, the three points,
  the emblem and the unused category count go. Add `section.home__trust` with the three lines
  of spec §7.1 (`shield-check-duotone`, `chat-circle-duotone`, `flag-duotone`). In
  `aside.home__account`, add `<h2 class="home__account-title">شما هم چیزی برای عرضه
  دارید؟</h2>` above the anonymous text and give «ثبت آگهی» `btn btn--accent`.
- `category/[slug].vue`, `city/[slug].vue`: breadcrumb and header move into
  `<template #intro>`; no `emeraldHero`.
- `ListingCard`: the rating glyph becomes `<AppIcon name="star-fill" :size="13" />`. The link
  gains `<span class="listing-card__category" aria-hidden="true">` between the media and the
  title; `.listing-card__meta` holds `map-pin`, the city (link or span), a dot and the
  `<time>`, plus the category as `.sr-only` text; `.listing-card__context` goes (C8). The
  label is `formatListingDay(listing.published_at, renderNow)` where
  `const renderNow = useState('listing-day-now', () => Date.now())`, and
  `onMounted(() => { if (Date.now() - renderNow.value > 3_600_000) renderNow.value = Date.now() })`.
- `HomeAdvertisingCard`: «بستن» becomes `x-bold` with the text visually hidden;
  `<BrandMascot pose="full" v-if="internal" />`; `li.browser__advertising` spans the full row.
- `.browser__filter-dialog` is a bottom sheet below 700px and a centred panel above, with
  `--radius-dialog`; its script is untouched (`home.spec.ts:146-205`).
- `LISTING_CARD_IMAGE_SIZES` is rewritten from the final grid (2 columns below 700px, 3 up to
  1279px, 4 from 1280px, sidebar from 1024px) so that it is within 4px of the rendered image
  at 375, 768, 1024, 1280 and 1440px.

Pinned by tests:

- Home has exactly one `h1` and it contains «جستجو» (`home.spec.ts:12`); `.home__intro` holds
  no province or city name (`home.spec.ts:59-60`); only one element contains «خوش آمدید،»
  (`auth.spec.ts:39`); «برای ثبت آگهی وارد شوید.» stays an element of its own
  (`auth.spec.ts:75`).
- The search keeps `form#search`, `#listing-search`, its label «جستجو» and `type="search"`.
  «فیلترها» keeps its name and is `display: none` from 1024px (`home.spec.ts:25,39`);
  `.browser__desktop-filters` shows from 1024px with its labelled fields
  (`home.spec.ts:31-45`).
- A category tile's accessible name is the category name only (`home.spec.ts:18-19`). On
  `/category/*` and `/city/*` nothing precedes the text inside the `h1`
  (`clean-urls.spec.ts:12,35`).
- `ul.browser__cards > li`, with the ad as the fifth `li` (`advertising.spec.ts:25-28`);
  `.browser__result-count` keeps its text.
- `.listing-card__link` stays the card's only element with that class and its accessible name
  is the title alone (`admin-errors.spec.ts:62`, `comments.spec.ts:286`); the title stays
  `h3.listing-card__title`; the image keeps `width="480" height="360"` and the one
  `fetchpriority="high"` rule; «بدون تصویر» and «تصویر در دسترس نیست» stay visible text
  (`home.spec.ts:343-357`); `.listing-card__city-link` stays at least 44×44
  (`clean-urls.spec.ts:64-72`).
- First-screen blocks keep a height that does not depend on the font: the search pill, the
  tiles (one-line labels), the results header, and a card title box that always reserves two
  lines. This is the defence for Home's CLS; only the CI run can confirm it. The baseline was
  0.0727. After Task 2 it is 0.0789 (run 43704168232): the font swap moves the same 70px as
  before, and the new shell starts the page 20 to 33px higher, so more of the moved content
  is on the first screen. The ceiling is 0.1. This task must end below 0.0789.
- Home's LCP element is no longer fixed. The baseline's diagnosed run had the `h1` (1699 ms);
  the Task 2 run's diagnosed run had the first card image (2552 ms, 148 ms under the ceiling).
  The median was 2405.1 ms both times. The first card image keeps the one
  `fetchpriority="high"`, and the new first screen must not make a late-loading element the
  largest one.

- [ ] **Step 5: Run the unit tests**

Run: `npm run test:unit`
Expected: PASS.

- [ ] **Step 6: Edit and add the Home tests (T6, T7, Review Focus 1, 2, 4)**

Line 350 becomes `await expect(noCover.getByText('بررسی‌شده برای انتشار', { exact: true })).toHaveCount(0)`;
line 381 expects `'--w240.webp'`. Add:

```ts
test('@home card photos are centred square crops, never stretched', async ({ page }) => {
  const title = 'آگهی قاب مربع شبدر'
  await createListing({ title, state: 'active' })
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto(`/?q=${encodeURIComponent(title)}`)
  await expect(page.locator('.listing-card__image').first()).toHaveCSS('object-fit', 'cover')
  const box = await page.locator('.listing-card__media').first().boundingBox()
  expect(Math.abs((box?.width ?? 0) - (box?.height ?? 99))).toBeLessThanOrEqual(1)
})

test('@home a twelve-digit price and an unbroken title stay inside the two-column card at 320px', async ({ page }) => {
  const term = 'آگهی نوشتهٔ بی‌فاصلهٔ شبدر'
  await createListing({ title: `${term} ${'ک'.repeat(48)}`, priceToman: 999_999_999_999, imageCount: 0, state: 'active' })
  await page.setViewportSize({ width: 320, height: 568 })
  await page.goto(`/?q=${encodeURIComponent(term)}`)
  const card = page.locator('.listing-card').filter({ hasText: term })
  await expect(card).toBeVisible()
  expect((await card.boundingBox())?.width ?? 999).toBeLessThanOrEqual(160)
  const clipped = await card.evaluate((element) => [...element.querySelectorAll<HTMLElement>('.listing-card__title, .listing-card__price')]
    .filter((node) => node.scrollWidth > node.clientWidth + 1).map((node) => node.className))
  expect(clipped).toEqual([])
  await expectNoOverflow(page)
  await expectTouchTargets(page)
})

test('@home a device clock 36 hours ahead of the server causes no hydration mismatch', async ({ page }) => {
  const errors: string[] = []
  page.on('console', (message) => { if (/hydrat|mismatch/i.test(message.text())) errors.push(message.text()) })
  page.on('pageerror', (error) => errors.push(error.message))
  await page.clock.setFixedTime(new Date(Date.now() + 36 * 60 * 60 * 1000))
  await page.goto('/')
  await expect(page.locator('.listing-card time').first()).toBeVisible()
  await page.waitForFunction(() => document.documentElement.classList.contains('fonts-enhanced'))
  expect(errors).toEqual([])
})
```

- [ ] **Step 7: Run the stage gate, commit and push**

This run decides the two risks the spec names for Home. Report to the owner whatever the
result: the LCP element and median, CLS, and client JS, against the baseline row.

```bash
git add apps/web/app apps/web/tests apps/web/package.json
git commit -m "feat: photo-first home, category and city pages"
git push origin arena/76915f79-shabdar
```

### Task 4: Listing detail (spec §7.3)

**Files:**
- Modify: `app/pages/listings/[slug].vue:13-165` and its style,
  `app/components/ListingGallery.vue:38-60`, `ListingRating.vue:49`,
  `ListingComments.vue:119-150`, `ReportControl.vue`, `ReportForm.vue`, `BlockControl.vue`,
  `app/assets/css/main.css` (listing-gallery block), `app/utils/icons.ts`, the sprite
- Test: `tests/e2e/reports.spec.ts` (T8, one new test), `tests/e2e/blocks.spec.ts` (T9),
  `tests/e2e/detail.spec.ts` (one new assertion)

**Interfaces:**
- Consumes: `AppIcon`, `.avatar`, `formatListingDay`, `listing-day-now`.
- Produces `details.detail__reports` and `details.listing-comments__menu`, the two selectors
  the edited tests open.

**D1, decided by the owner on 2026-11-15: the whole photo.** The main photo keeps
`object-fit: contain` in a 4:3 sand frame: the whole photo shows, a portrait photo gets side
bars, and `detail.spec.ts:30` is not edited. Listing cards still crop to a square.

- [ ] **Step 1: Extend `SCOPE` and see it fail**

Append `app/pages/listings/`, `app/components/ListingGallery.vue`,
`app/components/ListingRating.vue`, `app/components/ListingComments.vue`,
`app/components/Report` and `app/components/BlockControl.vue`.

Run: `npm run test:unit`
Expected: FAIL: scale offenders, `→` in `[slug].vue`, `★` and `☆` in `ListingRating.vue`.

- [ ] **Step 2: Restructure the page**

- `header.detail__header` keeps the back link, with `arrow-right-bold` instead of the glyph.
  The block with `.detail__category` and the `h1` moves to the top of `.detail__summary`,
  which becomes a `<section>` with the same class and `aria-label` (C6). Under the title add
  one facts line: `star-fill` with the average and count, the comment count, and
  `formatListingDay(listing.published_at, renderNow)` in a `<time>`.
- `.detail__seller-avatar` also gets the class `avatar`.
- A new `div.detail__actions` wraps `.detail__contact` and `.detail__favorite`. The favourite
  control becomes an icon button (`heart`, `heart-fill`) whose current text stays inside it,
  visually hidden; the guest link «ورود برای ذخیره» likewise.
- `div.detail__reports` becomes `<details class="detail__reports">` with
  `<summary class="detail__reports-title">` holding `shield-check`, «گزارش و ایمنی» and
  `caret-down-bold`. Its `v-if` and its two report rows are unchanged (C5, C7).
- A new `div.detail__feedback` wraps `ListingRating` and `ListingComments` as one sheet. They
  stay siblings.
- `ListingComments`: in each item, `BlockControl` and the two `ReportControl`s move into
  `<details class="listing-comments__menu">` whose `<summary>` holds `dots-three-bold` and the
  visually hidden text «گزینه‌های بیشتر». The menu has the union of their `v-if` conditions,
  so an item with none of them has no menu. «حذف دیدگاه» stays outside it.
- The `gone` and `missing` alerts (`[slug].vue:4-11`) open with a bare `<p>` that keeps the
  browser's 16px margin, so the tone icon sits one line above the words (found in Task 2).
  They show only when a later client-side read fails; the first load throws the error page.
  Give the paragraph `margin: 0`.
- `ListingGallery`: the previous and next buttons show `caret-right-bold` and
  `caret-left-bold`; their `aria-label`s stay. `ListingRating`: each star shows `star-fill`
  or `star`. Heart and stars get the short pop of spec §3.3.

Pinned by tests:

- On phones `.detail__gallery` ends above `.detail__summary`; from 1024px their tops are equal
  (`detail.spec.ts:74-97`). `.detail__description` keeps `white-space: pre-line`.
- The gallery image keeps its attributes, the buttons «تصویر قبلی» and «تصویر بعدی», the
  thumbnails' `aria-pressed` and the swipe handlers (`detail.spec.ts:24-71`).
- The favourite button keeps its two names and `aria-pressed`; `.detail__favorite
  [role="status"]` keeps its text (`detail.spec.ts:142-214`).
- The server HTML contains none of «گزارش آگهی», «ورود برای گزارش», «مسدود کردن نویسنده» or
  `/me/blocks` (`reports.spec.ts:65-66`, `blocks.spec.ts:87-88`): both disclosures render only
  after hydration, as their contents do today.
- `.listing-comments` holds one `role="status"` element in the tested states, so the rating
  section is never nested inside it (`comments.spec.ts:191`).
- `.listing-rating__summary` keeps its exact text; the radio names stay (`ratings.spec.ts`).

- [ ] **Step 3: Run the unit tests**

Run: `npm run test:unit`
Expected: PASS.

- [ ] **Step 4: Edit the report and block tests (T8, T9) and add the keyboard test**

Add to both spec files:

```ts
async function openDisclosure(disclosure: Locator): Promise<void> {
  if (!(await disclosure.evaluate((element) => (element as HTMLDetailsElement).open))) {
    await disclosure.locator('summary').click()
  }
}
```

Call `openDisclosure(page.locator('details.detail__reports'))` before `reports.spec.ts` lines
70, 82, 134, 155 and before the seller target at 115. Call
`openDisclosure(item.locator('details.listing-comments__menu'))` before the comment and author
targets at 115, before 184, and before `blocks.spec.ts` lines 109, 131 and 171. After
`blocks.spec.ts:108` add that the viewer's own comment has no `details.listing-comments__menu`.
In `detail.spec.ts`, after line 30 add that `.listing-gallery__main` has a width-to-height
ratio within 0.02 of 4/3. Add to `reports.spec.ts`:

```ts
test('@reports the safety foldout and the comment menu work from the keyboard', async ({ page }) => {
  const created = await createListing({ title: 'مسیر صفحه‌کلید گزارش', state: 'active' })
  await createComment('admin', created.slug, 'دیدگاه برای منوی صفحه‌کلید')
  await applyStorage(page, readFixtures().buyerStorage)
  await page.goto(detailPath(created.slug))
  const safety = page.locator('details.detail__reports > summary')
  await safety.focus()
  await expect(safety).toBeFocused()
  await page.keyboard.press('Enter')
  await page.keyboard.press('Tab')
  await expect(page.getByRole('button', { name: 'گزارش آگهی', exact: true })).toBeFocused()
  const menu = page.locator('details.listing-comments__menu > summary').first()
  await menu.focus()
  await page.keyboard.press('Space')
  await expect(page.locator('.listing-comments__item').first().getByRole('button', { name: 'مسدود کردن نویسنده', exact: true })).toBeVisible()
})
```

- [ ] **Step 5: Run the stage gate, commit and push**

Detail's LCP has the least room; compare it with the baseline row before going on.

```bash
git add apps/web/app apps/web/tests
git commit -m "feat: listing detail with one action row and quiet safety controls"
git push origin arena/76915f79-shabdar
```

### Task 5: Forms — post, login, advertising (spec §7.4, §7.5, §7.10)

**Files:**
- Modify: `app/pages/post.vue:3-241` and its style, `app/pages/login.vue`,
  `app/pages/advertising.vue`, `app/utils/icons.ts`, the sprite
- Test: `tests/e2e/post.spec.ts` (one new assertion)

**Interfaces:**
- Consumes: `AppIcon`, the `accent` variant and the field styles of Task 2.
- Produces: nothing later tasks use.

- [ ] **Step 1: Extend `SCOPE` and see it fail**

Append `app/pages/post.vue`, `app/pages/login.vue`, `app/pages/advertising.vue`.

Run: `npm run test:unit`
Expected: FAIL with scale offenders in the three files.

- [ ] **Step 2: Restyle the three pages**

- `post.vue`: the three fieldsets become sheets; `.post__price-options` becomes the segmented
  choice (the radios stay). Under the price field add
  `<p id="post-price-readback" class="post__price-readback">`, shown when the price is a
  positive number, with `formatPrice(form.price_toman, 'fixed')`, and add its id to the
  input's `aria-describedby`. The file input covers a dashed area holding `camera-plus-duotone`,
  «افزودن تصویر» and «N از ۸ تصویر انتخاب شده»; the area is decoration and the input's
  accessible name stays «تصاویر آگهی». Previews are square; the remove button shows `x-bold`
  with its text «حذف پیش‌نمایش N» visually hidden; the first preview gets
  `<span class="post__preview-cover">کاور</span>`. «ارسال برای بررسی» gets `variant="accent"`;
  the order and the handlers of the action row do not change.
- `login.vue`: one centred sheet, at most 420px; the two mode buttons drawn as two segments.
- `advertising.vue`: page head, sheets with icon rows, fields from Task 2.

Pinned by tests:

- `/post` has exactly one heading matching «ثبت آگهی» (`advertising.spec.ts:120`);
  `.post__state` keeps its exact words; one `.post__preview` per preview; all labels stay
  (`post.spec.ts`).
- `.login__panel` has the same box in the phone step and the code step, within 0.5px
  (`login.spec.ts:39-51`); it holds `svg[data-pose="full"]`; the mode buttons keep
  `aria-pressed`.
- `.advertising-form button[type="submit"]` keeps its text.

- [ ] **Step 3: Pin the read-back**

In `post.spec.ts`, after the price is filled at line 170:

```ts
await expect(page.locator('.post__price-readback')).toHaveText('۱۲٬۵۰۰٬۰۰۰ تومان')
```

- [ ] **Step 4: Run the stage gate, commit and push**

```bash
git add apps/web/app apps/web/tests
git commit -m "feat: post, login and advertising forms in the new system"
git push origin arena/76915f79-shabdar
```

### Task 6: Account — dashboard, favourites, blocks (spec §7.6, §7.7)

**Files:**
- Modify: `app/pages/me/index.vue`, `app/pages/me/favorites.vue:13-54`,
  `app/pages/me/blocks.vue`, `app/utils/icons.ts`, the sprite
- Test: none edited; `tests/e2e/account.spec.ts` and `blocks.spec.ts` must pass unchanged

**Interfaces:**
- Consumes: `AppIcon`, `.avatar`, the `soft`, `accent`, `danger` and `danger-quiet` variants,
  the card grid of Task 3.
- Produces: nothing later tasks use.

- [ ] **Step 1: Extend `SCOPE` and see it fail**

Append `app/pages/me/index.vue`, `app/pages/me/favorites.vue`, `app/pages/me/blocks.vue`.

Run: `npm run test:unit`
Expected: FAIL with scale offenders in the three files.

- [ ] **Step 2: Restyle the three pages**

- Dashboard: a greeting sheet with `<span class="avatar avatar--b">` and a saffron «ثبت
  آگهی»; each `.stat` gains a duotone icon in a tint; the account links become one list card
  with icons and `caret-left-bold`; each `.mine__row` is a sheet; `.badge` gains an icon by
  status (active `check-circle-fill`, pending `clock-fill`, rejected `x-circle-fill`, the
  others neutral); row actions use `soft`, «حذف آگهی» uses `danger-quiet`, «تأیید حذف» stays
  `danger`.
- Favourites: the Home grid; the action is an icon button with `heart-fill` and
  `:aria-label` set to the expression that is its text today; CSS places
  `.listing-card__actions` on the photo corner.
- Blocks: one list card; each row has an avatar, the name, the date and «رفع مسدودسازی» as a
  soft pill.
- The six `AppEmptyState` uses in these three pages get `mascot="full"`.

Pinned by tests:

- Four `.stat`, each with its label and a `.stat__value` matching `^[۰-۹٬]+$`
  (`account.spec.ts:35-40`).
- `.badge` holds the status word only; `.mine__actions` is absent when a row has no action
  (`account.spec.ts:42,60,73`); the label «فیلتر وضعیت» stays attached to its select;
  `.dashboard__notice` keeps its focus behaviour.
- `.favorites [role="status"]`, the button name «حذف از علاقه‌مندی‌ها» and `[data-block-id]`
  stay.

- [ ] **Step 3: Run the stage gate, commit and push**

```bash
git add apps/web/app
git commit -m "feat: dashboard, favourites and blocks in the new system"
git push origin arena/76915f79-shabdar
```

### Task 7: Messages — inbox and conversation (spec §7.8, §7.9)

**Files:**
- Modify: `app/pages/me/conversations/index.vue`, `app/pages/me/conversations/[publicId].vue`,
  `app/components/ConversationListItem.vue:3-23`, `MessageBubble.vue:3-19,39-42`,
  `ConversationComposer.vue`, `ListingReferenceCard.vue`, `app/assets/css/main.css` (one shell
  rule), `app/utils/icons.ts`, the sprite
- Test: `tests/e2e/conversation-thread.spec.ts:360,403` (T10)

**Interfaces:**
- Consumes: `AppIcon`, `.avatar`, `--color-bubble`, `--color-on-bubble`, `--color-bubble-meta`.
- Produces: nothing later tasks use.

- [ ] **Step 1: Extend `SCOPE` and see it fail**

Append `app/pages/me/conversations/`, `app/components/Conversation`,
`app/components/MessageBubble.vue` and `app/components/ListingReferenceCard.vue`.

Run: `npm run test:unit`
Expected: FAIL: scale offenders and the inline `<svg>` in `ConversationComposer.vue` and
`[publicId].vue`.

- [ ] **Step 2: Restyle the inbox and the conversation**

- Inbox: one list card. Each row gains `<span class="avatar" aria-hidden="true">` with the
  first letter of the name; the listing note gains the `tag` icon and stays plain text. The
  four `AppEmptyState` uses in the two pages get `mascot="full"`.
- Conversation: one sheet between the top bar and the tab bar. `main.css` gets
  `.shell:has(.thread) .shell__footer { display: none; }`. `.thread__scroller` stays the only
  scroll container.
- The sheet must fit in phone landscape as well. Since Task 2 the tab bar is fixed at
  844×390 (it was static there before), and the card's minimum height is 28rem, so the
  composer sits 176px under the bar until the page is scrolled. It is fine at 375×812,
  375×667, 768×1024 and from 1024px. Measure the composer against the bar at 844×390 and
  667×375.
- `MessageBubble`: the `.bubble__avatar` element and its `initial` go; the `.sr-only` sender
  stays. Own bubbles use `--color-bubble`, others sand; 20px corners with one tight 6px
  corner. `.bubble__seen` holds `checks-bold` and its text «دیده شد», visually hidden.
- Re-run axe on the conversation page locally in both themes: CI's `quality.spec.ts` has no
  conversation route. Before the refresh «دیده شد» was emerald text on a 15% emerald tint,
  4.43:1 (found in Task 2; the refresh did not cause it). The new bubble colours must pass.
- Composer: pill textarea, round send button, counter underneath. The three inline `<svg>`s
  become `AppIcon`: back `arrow-right-bold`, menu trigger `dots-three-vertical-bold`, send
  `paper-plane-right-fill` (mirrored for RTL).

Pinned by tests:

- `.inbox h1` reads «پیام‌ها»; each `.conversation-row` holds exactly one link;
  `.conversation-row__unread` holds only the number (`conversation-inbox.spec.ts:57,75,78`).
- `.thread__name` holds only the name (`conversation-inbox.spec.ts:80`).
- `.bubble__surface` is at most 75% wide, own messages at the inline end
  (`conversation-thread.spec.ts:373-398`); one `.bubble__time[datetime]` per bubble; the scroll
  anchor tests at `:216-262` must still pass.
- `#composer-body`, `#composer-count` and its `composer__counter--over` class,
  `.composer__send`, `.composer__note`, `.composer__chip`, `.thread__day`, `.thread__jump`,
  `.thread__menu-trigger` and `.thread__menu-panel` stay.

- [ ] **Step 3: Edit the avatar assertion (T10)**

Line 403 becomes `await expect(page.locator(`${selector} .bubble__avatar`)).toHaveCount(0)`
and the title at line 360 ends with "opposite sides and a time per bubble".

- [ ] **Step 4: Run the stage gate, commit and push**

```bash
git add apps/web/app apps/web/tests
git commit -m "feat: inbox and conversation in the new system"
git push origin arena/76915f79-shabdar
```

### Task 8: Admin, legal and error pages (spec §7.10, §7.11)

**Files:**
- Modify: the seven files under `app/pages/admin/`, `app/pages/privacy.vue`,
  `app/pages/terms.vue`, `app/assets/css/main.css` (error-page block), `app/utils/icons.ts`,
  the sprite
- Test: none edited; the four `admin-*.spec.ts` files, `legal.spec.ts` and the 404 test of
  `quality.spec.ts` must pass unchanged

**Interfaces:**
- Consumes: the sheets, fields, chips and button variants of Task 2.
- Produces: nothing later tasks use.

- [ ] **Step 1: Extend `SCOPE` and see it fail**

Append `app/pages/admin/`, `app/pages/privacy.vue`, `app/pages/terms.vue`.

Run: `npm run test:unit`
Expected: FAIL with scale offenders in those files.

- [ ] **Step 2: Restyle**

Admin: the same tokens and parts with denser spacing; no Shabdarak, no decorative tint; no
layout or behaviour change; destructive buttons use `danger`. Legal: a reading column of at
most 68ch, the contents list as a list card, section headings at 22px/800. Error page: the
status in accent text, the title, one line and the two buttons.

Pinned by tests: `.card` rows inside `.queue` and `.legacy-queue`; `#queue-action` with its
`data-tone`; no `.brand-mascot` inside `.queue`; every `data-report-id`, `data-user-id`,
`data-rule-id`, `data-action`, `data-request-id` and `data-rescan-*` attribute;
`.admin-rule`, `.admin-rule__phrase`, `.admin-rule-create`, `.admin-moderation-case`,
`.review-image`, `.advertising-slot__state`, `.advertising-request__status`;
`.error-page__status`, and Shabdarak on 404 only (C9).

- [ ] **Step 3: Run the stage gate, commit and push**

```bash
git add apps/web/app
git commit -m "feat: admin, legal and error pages in the new system"
git push origin arena/76915f79-shabdar
```

### Task 9: Close-out (spec §9, §10 stage 8, §13)

**Files:**
- Modify: `tests/unit/design-system.test.mjs` (`SCOPE`), any file it then reports,
  `app/assets/css/tokens.css`, `design-system/shabdar/MASTER.md`, `docs/STATUS.md`, the spec
  (status lines), this plan (an "Outcome" section)

**Interfaces:**
- Consumes: every earlier task.
- Produces: the final commit that the owner reviews.

- [ ] **Step 1: Widen the scale test to the whole app and see what is left**

Set `SCOPE` to `['app/']`.

Run: `npm run test:unit`
Expected: FAIL only if a file was missed; fix each offender it lists.

- [ ] **Step 2: Delete what nothing uses**

Remove every token in `tokens.css` and every rule in `main.css` that no file under `app/`
references. Run `npm run test:unit` and `npm run build`; expected: PASS, "Build complete".

- [ ] **Step 3: Check the spec's acceptance list (§13)**

`git diff --stat fda97b0 -- apps/web/tests/e2e` lists only the files named in "Test edits"
and in the tasks' new tests. `git diff --stat fda97b0 -- apps/web/package.json
apps/web/package-lock.json apps/web/nuxt.config.ts apps/web/lighthouserc.cjs
apps/web/scripts/compare-client-js.mjs .github` lists only `package.json`, and its diff is the
two test lists.

- [ ] **Step 4: Rewrite `MASTER.md` and update the records**

`MASTER.md` describes the system that shipped and drops the six rules spec §9 names; its
accessibility, privacy, performance and Shabdarak rules stay. `docs/STATUS.md` gets the final
table and what was verified where. The spec's status lines say it is implemented. This plan
gets an "Outcome" section with the run ids.

- [ ] **Step 5: Run the stage gate, commit and push**

```bash
git add apps/web design-system/shabdar/MASTER.md docs
git commit -m "feat: finish the visual refresh and document the new system"
git push origin arena/76915f79-shabdar
```

Expected: green on this commit with every gate unedited.

- [ ] **Step 6: Hand over**

The owner looks at the live preview at 375, 768, 1024 and 1440px in both themes (§13.1). Then
follow `verification-before-completion` and `finishing-a-development-branch`. The owner opens
the PR.
