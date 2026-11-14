# Visual refresh — «گرم و خانگی» (warm and homey)

**Date:** 2026-11-14

**Status:** Direction and screens approved in conversation, one mockup round at a
time (Home, Listing detail, then post form + dashboard + messages). This written
spec awaits the owner's review. **No application code has been written**; per
`AGENTS.md`, implementation starts only after this spec and its plan are approved.

**Branch:** `arena/76915f79-shabdar`. **Baseline:** `fda97b0` (merge of PR #25).

**Reference images** (before / approved proposal, same sample data on both sides):

- [Home](../../../design-system/shabdar/refresh-2026-11/home.webp)
- [Listing detail](../../../design-system/shabdar/refresh-2026-11/detail.webp)
- [Post form, dashboard, messages, empty state](../../../design-system/shabdar/refresh-2026-11/inner.webp)

The images are mockups rendered from static HTML, not screenshots of the app.

## 1. Intent

The owner's request: the site "feels ugly and dead" and needs one complete pass,
keeping what already works. The review of the current site found seven causes:

1. Photos come last (first photo at y=735 on a 390x844 phone).
2. A cold grey-green palette with almost no warm accent.
3. Every block is the same bordered box (28 bordered boxes on phone Home); cards
   carry no icons.
4. Raw form controls, including a native theme select in the header.
5. No scale: 11 font sizes, 9 weights and 9 corner radii on Home alone.
6. No motion or touch feedback (one `@keyframes`, one `:active` rule in `app/`).
7. Unfinished details: «بررسی‌شده برای انتشار» on every card, a floating nav that
   covers content, twelve «ورود برای…» prompts on one listing page.

**Kept as is:** the emerald brand colour, Shabdarak, the emerald-and-saffron hero,
Vazirmatn / RTL / Persian digits, light + dark themes, the messenger layout, the
order of information on a card, and all accessibility and performance groundwork.

**Scope:** presentation of `apps/web` only — CSS, templates, one icon sprite, and
the design-system documentation. No API, database, route, auth or data change.

## 2. Ground rules

These rules decide every case the sections below do not spell out.

1. **Mockups define the look, not the markup.** Existing class names, ids,
   accessible names, labels and messages are used by the Playwright suite and stay
   as they are. A change to any of them must be listed in §9 (or in the plan) with
   its test edit. Mockup class names (`.card`, `.sheet`, `.msg`, …) are not ported.
2. **CSS first.** A template changes only where an element is added, removed or
   wrapped.
3. **No new client behaviour.** Existing dialogs, menus, gallery, rating, favourite
   and chat logic keep their code. A new disclosure uses native `<details>`.
4. **No new dependency.** No Tailwind, UI kit, icon package, animation library or
   remote asset.
5. **Gates are not edited.** Client JS stays under 104,000 gzip bytes per route
   (Home is at 99,578, Detail at 100,464). Lighthouse mobile medians stay at
   LCP ≤ 2700 ms, CLS ≤ 0.1, TBT ≤ 200 ms, accessibility ≥ 0.95.
   `quality.spec.ts` keeps requiring no serious axe violation, no horizontal
   overflow and every target ≥ 44x44 px on 18 routes x 4 widths x 2 themes.
6. **Every stage in §10 ends with CI green.** If a gate cannot be met, work stops
   and the owner is asked; a gate is never loosened to pass.

## 3. Design tokens

All values live in `app/assets/css/tokens.css`. Existing token names are kept so
that no consumer breaks; values change and new tokens are added.
`tests/e2e/theme.spec.ts` pins the colour tokens and is updated in the same commit.

### 3.1 Colour

| Token | Light | Dark | |
|---|---|---|---|
| `--color-bg` | `#FBF7F0` | `#14120E` | changed: warm paper |
| `--color-surface` | `#FFFFFF` | `#1E1B16` | dark changed |
| `--color-surface-muted` | `#F4ECDF` | `#28241D` | changed: sand |
| `--color-text` | `#1F1B16` | `#F6F1E8` | changed |
| `--color-text-muted` | `#675C4F` | `#BDB2A1` | changed |
| `--color-primary` | `#047857` | `#6EE7B7` | unchanged |
| `--color-on-primary` | `#FFFFFF` | `#062E22` | unchanged |
| `--color-primary-hover` | `#065F46` | `#A7F3D0` | unchanged |
| `--color-primary-soft` | `#E2F1EA` | `#16342A` | new: active pill, focus halo |
| `--color-border` | `#EADFCF` | `#3A342A` | changed: hairlines only |
| `--color-control-border` | `#8F8474` | `#8C8172` | changed: field outlines (≥ 3:1) |
| `--color-focus` | `#047857` | `#6EE7B7` | unchanged |
| `--color-accent` | `#A2470B` | `#F3C876` | light changed: accent **text** |
| `--color-accent-soft` | `#FCEFD6` | `#33290F` | changed |
| `--color-accent-fill` | `#F2B33D` | `#F3C876` | new: saffron **fills** |
| `--color-on-accent-fill` | `#3A2903` | `#2B1F03` | new |
| `--color-danger` | `#B42318` | `#FDA29B` | changed |
| `--color-on-danger` | `#FFFFFF` | `#450A0A` | unchanged |
| `--color-error` | `#B42318` | `#FDA29B` | changed |
| `--color-error-bg` | `#FDECE9` | `#3B1D1A` | changed; dark is now a real tint |
| `--color-warning` | `#8A5A00` | `#F3C876` | changed |
| `--color-warning-bg` | `#FCEFD6` | `#3A2E12` | changed |
| `--color-info` | `#675C4F` | `#BDB2A1` | changed: neutral, no blue |
| `--color-info-bg` | `#F4ECDF` | `#28241D` | changed |
| `--color-tint-a` / `--color-ink-a` | `#FCEFD6` / `#8A5A00` | `#3A2E12` / `#F3C876` | new: saffron pair |
| `--color-tint-b` / `--color-ink-b` | `#E2F1EA` / `#047857` | `#16342A` / `#6EE7B7` | new: emerald pair |
| `--color-tint-c` / `--color-ink-c` | `#FBE5DA` / `#A2470B` | `#3D2417` / `#F2AB84` | new: clay pair |
| `--color-bubble` | `#047857` | `#1F4A3B` | new: own chat bubble |
| `--color-on-bubble` | `#FFFFFF` | `#F6F1E8` | new |
| `--color-bubble-meta` | `#D5F5E6` | `#B9D8CB` | new: time inside own bubble |
| `--color-hero`, `-deep`, `-text`, `-muted`, `-accent`, `-border` | | | unchanged |

Rules:

- Saffron is a fill, never text: `--color-accent-fill` carries
  `--color-on-accent-fill`. Accent-coloured text uses `--color-accent`.
- The three tint pairs are decorative (category tiles, avatars, stat and trust
  icons). Status never relies on a tint alone; it always has an icon and a word.
- `--color-on-dark` and `--color-on-dark-muted` are deleted if nothing uses them
  after the sidebar turns light.

Contrast, computed from these values: 35 text pairs per theme, minimum 4.68 in
light (primary on sand) and 6.54 in dark. Field outlines measure 3.13–3.67 in
light and 4.04–4.90 in dark against the three surfaces.

### 3.2 Type

- Sizes: `--font-size-12/14/16/18/22/28/36` (0.75–2.25rem) and
  `--font-size-display` 2.625rem, used only by the Home headline from 1280px.
- Weights: 400 body, 600 labels and controls, 800 headings, prices and names.
  No other weight.
- Line height 1.8 for body text, 1.4–1.6 for headings. Inputs stay at 16px or more.
- Font files and the `fonts-enhanced` loading plugin are unchanged.

### 3.3 Shape, elevation, motion

- Radii: `--radius-small` 10px (new), `--radius-control` 14px, `--radius-card` 20px,
  `--radius-panel` and `--radius-dialog` 28px, `--radius-pill` 999px.
- Two shadows: `--shadow-card` (resting surface) and `--shadow-lifted` (hover,
  search bar, overlays), warm-tinted in light. `--shadow-xs` and `--shadow-nav`
  are removed; their 22 uses are deleted or moved to `--shadow-card`.
- Two glows under filled buttons: `--glow-primary`, `--glow-accent`.
- Surfaces separate by shadow and background, not by a border. Borders remain for
  field outlines, dividers inside a list card, and the tab bar's top edge.
- Motion is CSS only and uses the existing `--duration-*` and `--ease-out` tokens:
  press feedback (`:active` scale) on buttons, cards, tiles and tabs; hover lift
  inside `@media (hover: hover)`; a one-shot Shabdarak greeting; a short pop on the
  heart and stars. Nothing animates server-rendered content into view, nothing
  loops, and no effect moves surrounding layout. `prefers-reduced-motion` turns
  all of it off.
- The spacing scale is unchanged. Layout breakpoints are 700, 1024 and 1280px.

## 4. Icons

- One family: Phosphor (MIT; the licence file is already in `apps/web/licenses/`).
  Regular weight by default, fill for the active or pressed state, duotone inside
  tinted tiles. No emoji and no text glyph as an icon (the card's «★» becomes an
  icon).
- Delivery: one static SVG sprite, `app/assets/icons/sprite.svg`, emitted by the
  build as a hashed same-origin file. About 70 symbols; a trial build of the
  mockup's set is 33 KB raw, 8.5 KB gzip. It costs no client JS.
- New component `AppIcon` (`name`, `size`): renders
  `<svg aria-hidden="true" focusable="false"><use href="…#name"/></svg>` with fixed
  width and height, so a late sprite cannot shift layout. `name` is a typed union.
- `ShellIcon.vue` is deleted; its six inline paths leave the shared chunk.
- A unit test fails if a template uses an icon name that is missing from the
  sprite. A browser test asserts that a rendered icon points at a same-origin file,
  not a `data:` URL (`<use>` cannot read one).
- An icon-only control keeps a real accessible name.

## 5. Shell (`layouts/default.vue`)

One `nav.shell__nav` stays in the DOM; CSS lays it out per width.

- **Below 1024px:** a paper top bar (Shabdarak mark, wordmark, theme control) that
  scrolls with the page, and a **docked, opaque** tab bar with a top hairline and
  safe-area padding. Main content has a matching bottom inset, so nothing is
  covered. The current item gets a `--color-primary-soft` pill and a fill icon;
  «ثبت» always carries a saffron pill.
- **From 1024px:** a light sidebar (264px, `--color-surface`, hairline edge) with
  brand, tagline, nav rows, admin links and the theme control at the bottom.
  «ثبت» is a saffron row. The dark-green sidebar and its blurb card are removed.
- The theme control stays the native `<select aria-label="پوسته">` with its three
  options. Below 1024px it is drawn as a round icon button (sun or moon, chosen in
  CSS from the active theme) with the select laid transparently on top; in the
  sidebar it is a labelled row. Focus shows on the wrapper.
- Footer: centred, muted, same three links. It is hidden on the conversation page.
- The auth-state notice, `.shell__brand`, `.shell__nav`, `.shell__footer`,
  `.shell__main` and the admin nav items keep their markup.
- Emerald as a block appears only in the Home hero and the Home seller band.

## 6. Shared parts

Existing components are restyled; only `AppIcon` is new.

| Part | Rule |
|---|---|
| Page head | `h1` at 28px/800, one muted lead line, optional status pill. No hero and no header card on inner pages. Content column max 820px on form, account and message pages; browsing pages keep the existing 75rem maximum. |
| Sheet | White surface, `--radius-card`, `--shadow-card`, no border. Replaces every bordered section, fieldset and row card. |
| List card | One sheet holding rows separated by hairlines (inbox, account links). |
| Buttons (`AppButton`, `.btn`) | Pill shape, 46px or taller. Primary: emerald with glow. Accent: saffron fill, used for publishing actions («ثبت آگهی», «ارسال برای بررسی»). Secondary: white surface with shadow. Soft pill: sand background, 44px, for row actions. Danger: quiet red text with an icon in rows; a filled red button only as the confirming action. Loading and disabled states stay. |
| Fields (`AppField`) | Label above; 50px control; 1.5px `--color-control-border`; `--radius-control`; focus = a 2px `--color-focus` outline drawn on the border plus a 4px `--color-primary-soft` halo; help and error text below, unchanged wiring. |
| Select | Same box; native control with `appearance: none` and a caret drawn as a themed background image. No wrapper element. |
| Segmented choice | Radio group drawn as three segments on a sand track; the checked one is a white raised segment. Used for the price type. |
| Photo input | The native file input covers a dashed drop area (camera icon, «افزودن تصویر», «N از ۸ تصویر انتخاب شده») and stays the labelled, focusable control. Previews are a 3-up square grid (5-up from 700px) with an icon remove button and a «کاور» tag on the first. |
| Alerts (`AppAlert`) | Tinted surface, icon, no border: error = `--color-error-bg`, success = `--color-tint-b`, warning = `--color-warning-bg`, info = `--color-info-bg`. Roles and focus behaviour unchanged. |
| Status chip (`.badge`) | Pill with icon and word: active = emerald, pending = saffron, rejected = red, sold / draft / archived = neutral. |
| Avatar | Circle with the first letter of the display name on a tint. No image. |
| Empty state (`AppEmptyState`) | A sheet with Shabdarak (existing `mascot` prop), title, one line and one action. Public and account pages show Shabdarak; admin pages do not. |
| Skeleton (`AppSkeleton`) | Sand blocks with the new radii; its existing animation stays and is off under reduced motion. |
| Dialog | Bottom sheet below 700px, centred panel above; `--radius-dialog`; existing focus handling. |

Buttons, links and custom controls keep the visible `:focus-visible` outline in
`--color-focus`; fields show focus as described above. Focus is never removed.

## 7. Pages

### 7.1 Home (`pages/index.vue`, `ListingBrowser`, `ListingCard`, `HomeAdvertisingCard`)

- **Hero.** The only emerald block at the top. Eyebrow with a pin icon, the
  existing headline, the existing lead (hidden under 700px and between 1024 and
  1279px), Shabdarak greeting once. The three bullet points and the ring emblem
  with its caption are removed.
- **Search inside the hero.** `ListingBrowser` gets an `intro` slot; a new wrapper
  `div.browser__hero` holds the slot, the existing `form#search` and
  `.browser__desktop-filters`. Home passes its intro and makes the wrapper emerald.
  The search is one white pill: icon, input, the existing «فیلترها» button as an
  icon button (below 1024px), and the submit as a saffron round icon button on
  phones or a labelled button on desktop. From 1024px the existing desktop filters
  sit under it as a row of pills. The filter dialog keeps its behaviour.
- **Categories.** `.browser__categories` becomes a row of icon tiles (tinted
  rounded square plus label), scrolling sideways on phones. Icons: all =
  `squares-four`, `home-sweets` = `cake`, `home-food` = `bowl-food`,
  `handicrafts` = `palette`, `clothing-sewing` = `t-shirt`, `plants-flowers` =
  `plant`, `beauty` = `flower-lotus`, `education` = `graduation-cap`; any other
  slug = `tag`.
- **Results header.** The existing kicker in 12px accent text, the heading at
  22px/800, and the result count as a small pill.
- **Cards.** Borderless sheet. Square, full-bleed photo (`object-fit: cover`);
  rating chip with a star icon; then category in 12px accent text, title (600,
  two lines max), price (800; 16px on phones, 18px from 700px), and place · day.
  Grid: 2 columns below 700px, 3 up to 1279px, 4 from 1280px. The no-photo and
  failed-photo placeholders stay. «بررسی‌شده برای انتشار» leaves the card.
- **Day label.** The card's `<time>` keeps its `datetime` and shows «امروز»,
  «دیروز», «N روز پیش» up to six days, then the current absolute date. It is
  computed in `Asia/Tehran` by one small pure function next to
  `formatListingDate`, so server and browser agree.
- **Advertising card.** Spans a full grid row after the fourth card, as a tinted
  banner. Its label, dismiss button, preview label and link rules stay. Shabdarak
  appears only when the card is Shabdar's own promotion (internal link), never
  beside a sponsor's advert.
- **Trust strip** (new, static): three icon rows —
  «بررسی پیش از انتشار — هر آگهی قبل از نمایش بازبینی می‌شود.»,
  «گفت‌وگو داخل شبدر — بدون نیاز به دادن شمارهٔ تماس.»,
  «گزارش و مسدودسازی — مزاحمت را گزارش کنید یا کاربر را مسدود کنید.»
- **Seller band.** The existing `aside.home__account` restyled as an emerald band.
  Its three auth states, its strings («خوش آمدید، …», «برای ثبت آگهی وارد شوید.»)
  and the «خروج» button stay; «ثبت آگهی» becomes the saffron action. The headline
  «شما هم چیزی برای عرضه دارید؟» is added above the anonymous state.
- Result on the mockup, 390x844 phone: first photo top 735 → 463px, complete
  photos in the first screen 0 → 2, bordered boxes 28 → 1. At 1440x900: complete
  cards in the first screen 0 → 4.

### 7.2 Category and city pages

Same browser. The emerald header card becomes a plain page head (breadcrumb, `h1`,
lead) passed through the `intro` slot; the search pill sits under it on paper.
Tiles, cards, result count, pagination and empty state as on Home.

### 7.3 Listing detail (`pages/listings/[slug].vue` and its components)

- Order on a phone: «بازگشت به جستجو», gallery, chips (category, place), title,
  one facts line (rating, comment count, day), price at 28px/800, seller row with
  avatar, actions, description, safety foldout, rating and comments.
- **Gallery.** Cover in a 4:3 frame with `object-fit: cover`, icon previous / next
  buttons, counter chip, thumbnail row. Button names and the counter text are
  unchanged.
- **Actions.** One primary action («گفت‌وگو با فروشنده», or «ورود برای گفت‌وگو»)
  and a heart icon button beside it whose accessible name stays «ذخیره در
  علاقه‌مندی‌ها» / «حذف از علاقه‌مندی‌ها» / «ورود برای ذخیره».
- **Safety foldout.** A `<details>` titled «گزارش و ایمنی» wraps the existing
  «گزارش آگهی», «گزارش فروشنده» and seller block controls, including their guest
  prompts.
- **Rating and comments** share one sheet. Each comment's «گزارش دیدگاه»,
  «گزارش نویسنده» and «مسدود کردن نویسنده» controls move into a `<details>` menu
  («گزینه‌های بیشتر») on that comment; a comment with none of these controls
  shows no menu. Guests see one «ورود برای امتیازدهی» and one
  «ورود برای ثبت دیدگاه».
- From 1024px: gallery and summary side by side, comments below.
- Result on the mockup: visible «ورود برای…» prompts 12 → 3, photo top 244 → 116px,
  main action bottom edge 979 → 757px on a 390x844 phone.

### 7.4 Post form (`pages/post.vue`)

- Page head with the existing status pill (three states, same words).
- Three sheets with numbered legends. Category, province and city stack on phones
  and sit in one row from 700px. Price type is the segmented choice.
- The price field stays `type="number"`; under it a read-back line shows the
  amount with Persian digits and «تومان».
- Photo section: the existing guide as a tinted tip box with its three checks,
  then the photo input and preview grid from §6. The remove button keeps its
  accessible name.
- Actions keep today's order and meaning: the submit button («ایجاد پیش‌نویس» /
  «ذخیرهٔ تغییرات») is primary; «ارسال برای بررسی» is the saffron publishing
  button beside it. `submitForReview()` does not save unsaved edits, so emphasis
  must not move to it in this pass.
- Result on the mockup, same draft with three photos on a 390x844 phone: page
  height 3225 → 1929px, preview block 1054 → 102px.

### 7.5 Login (`pages/login.vue`)

A single centred sheet (max 420px) with Shabdarak, the existing title and lead,
the two mode buttons drawn as two segments, and fields and buttons from §6. The
resend timer and error messages are unchanged.

### 7.6 Dashboard (`pages/me/index.vue`)

- Greeting sheet: avatar, «خوش آمدید، {name}» (text unchanged), saffron
  «ثبت آگهی».
- Four stat tiles, each a tinted duotone icon, the number at 22px/800 and its
  label. Two columns on phones, four from 700px.
- Account links («علاقه‌مندی‌های من», «مدیریت مسدودشدگان», and the admin links for
  an admin) as one list card with icons and carets.
- «آگهی‌های من»: heading with the status filter as a pill select (its label stays
  for screen readers). Each listing is a sheet: title (a link only when active, as
  today), price, status chip, rejection reason as a tinted note, actions as soft
  pills, «حذف آگهی» as quiet red text. The inline delete confirmation stays.
- Rows have no thumbnail: the owner-list API returns no image.

### 7.7 Favourites and blocks

- Favourites: page head, then the listing grid from Home. The card's action slot
  holds a filled-heart icon button on the photo corner whose accessible name stays
  «حذف از علاقه‌مندی‌ها»; a text button would not fit a two-column phone card.
- Blocks: a list card; each row has an avatar, the name, the blocked-on date and
  «رفع مسدودسازی» as a soft pill.
- Both empty states show Shabdarak.

### 7.8 Inbox (`pages/me/conversations/index.vue`, `ConversationListItem`)

One list card. Each row: avatar, name, time, a one-line preview with the «شما:»
prefix, a saffron unread count, and the listing note with a tag icon. An unread
row has a bold preview and an emerald time. The empty state shows Shabdarak.

### 7.9 Conversation (`pages/me/conversations/[publicId].vue`, `MessageBubble`, `ConversationComposer`)

- One sheet that fills the viewport between the top bar and the tab bar; only the
  message log scrolls and the footer is hidden on this page.
- Header: back, avatar, name, status line, «گزینه‌های بیشتر». Menu behaviour
  unchanged.
- Bubbles: own = `--color-bubble` at the inline end, other = sand at the inline
  start, 20px corners with one tight corner. The per-message avatar is removed;
  the sender stays available to screen readers as text. Time sits inside the
  bubble; «دیده شد» is shown as a double-check icon and stays as text for screen
  readers in `.bubble__seen`.
- A listing reference stays inside its message, as a white inset card.
- Day separators as small sand pills. No entrance animation on messages.
- Composer: pill textarea, round emerald send button, counter underneath. Wait,
  retry, expired-session and disabled notes keep their text and logic.
- Result on the mockup, same conversation on a 390x844 phone: complete messages
  in view 4 → 5, and the page itself no longer scrolls behind the log.

### 7.10 Advertising, legal, error

- `/advertising`: page head, the offer as sheets with icon rows, the request form
  with fields from §6.
- `/privacy`, `/terms`: a reading column (max 68ch) on paper, the contents list
  as a list card, section headings at 22px/800.
- `error.vue`: Shabdarak in the search pose, the status in accent text, title,
  line, two buttons. `.error-page__status` stays.

### 7.11 Admin (seven pages)

Same tokens, sheets, fields, chips and buttons; denser spacing; no Shabdarak and
no decorative tints. Queue and report rows are sheets with a status chip and soft
pill actions. Every button keeps its current role and confirmation step;
destructive ones use the danger style. No layout or behaviour change.

## 8. Accessibility and performance

- Contrast as in §3.1. Targets ≥ 44px. Focus visible. Status never by colour
  alone. RTL logical properties throughout.
- **LCP.** Photos now enter the first screen, so the LCP element on Home will
  often be the first card photo. `ListingCard` already gives the first card
  `loading="eager"` and `fetchpriority="high"`; its `sizes` attribute is rewritten
  for the new grid so phones fetch the small source. Width and height attributes
  stay, so the square crop cannot shift layout. Only CI can confirm the median;
  the sandbox cannot run Lighthouse.
- **Client JS.** Icons cost none (sprite). Removing `ShellIcon` paths, the hero
  bullets and the per-card status line offsets the new tiles, trust strip and day
  label. The plan measures Home and Detail after each stage that touches them.
- **CSS.** Styles are inlined in the SSR document, so the rewrite replaces rules
  instead of appending. Inline CSS bytes for Home and Detail are recorded before
  and after and reported in `docs/STATUS.md`; a rise above 10% is investigated.
- Icons, Shabdarak and decorative shapes are `aria-hidden`.

## 9. Test and documentation impact

Known test edits (the plan lists every one with its file and line):

| Change | Test |
|---|---|
| Colour token values and twelve new tokens | `theme.spec.ts` expected tables |
| «بررسی‌شده برای انتشار» leaves the card | `home.spec.ts` (no-cover card assertion) |
| Listing and seller report controls sit inside «گزارش و ایمنی» | `reports.spec.ts` opens the foldout first |
| Comment report and block controls sit inside the comment menu | `reports.spec.ts`, `blocks.spec.ts` open the menu first |
| Per-message avatar removed | `conversation-thread.spec.ts` (one-avatar assertion) |

Everything else the suite names stays: nav labels, «پوسته» and its options,
gallery button names, all five «ورود برای …» link names, form labels, status
words, `.bubble__seen` text, «خوش آمدید،», and the class names used as locators.

Documentation:

- `design-system/shabdar/MASTER.md` is rewritten to describe the new system. These
  of its current rules are replaced, by the owner's choice of direction:
  "neutral surfaces", "hierarchy from background and borders, no card shadows",
  weights 400/500/600/700, radii 12/16, icons from `@phosphor-icons/vue`, and
  "listing media 4:3 in cards". Its accessibility, privacy, performance and
  Shabdarak rules stand.
- `docs/STATUS.md` records the refresh and its verification run.

## 10. Build order

One spec, one staged plan. Each stage is pushed and must pass CI before the next.

1. **Foundation:** tokens and `theme.spec.ts`; type, radius and shadow scales; base
   elements; `AppIcon` and sprite; buttons, fields, alerts, chips, empty state,
   skeleton; the shell.
2. **Home**, category and city pages. This is where the LCP and client-JS risk is
   measured first.
3. **Listing detail.**
4. **Forms:** post, login, advertising.
5. **Account:** dashboard, favourites, blocks.
6. **Messages:** inbox and conversation.
7. **Admin, legal, error page**, remaining empty and skeleton states.
8. **Close-out:** delete dead CSS and unused tokens, rewrite `MASTER.md`, update
   `docs/STATUS.md`, final full CI run.

Between stages 1 and 7, pages not yet reached show the new colours and shell with
their old layout. That is expected on the branch and is gone by stage 8.

## 11. Decisions for the owner to confirm

Shown in the mockups and not objected to:

1. «بررسی‌شده برای انتشار» is said once in the trust strip, not on every card.
2. Card photos are cropped to a square; the detail cover is cropped to 4:3.
3. Report and block are one tap further away (foldout and comment menu).
4. The theme control looks like an icon button on phones.
5. No new feature is added (for example, no save button on cards).

Not mocked; built from the same parts, as agreed: login, favourites, blocks,
category and city pages, advertising, legal pages, admin, the error page and the
filter dialog (§7.2, §7.5, §7.7, §7.10, §7.11, §6).

Different from the mockups, on purpose:

6. Cards show the day («امروز», «دیروز», «۳ روز پیش»), not hours. An hour label
   can differ between the server render and the browser, and goes stale while the
   page stays open.
7. The third trust line no longer says "with one tap", because reporting now
   takes two.
8. Chat messages do not animate in; that would need new per-message state.
9. The post form keeps «ذخیره» as the main button (§7.4), and the price field
   keeps Latin digits while typing, with a Persian read-back.
10. The search placeholder becomes «دنبال چه می‌گردید؟» so it fits the pill on
    phones.

Not in this pass, noted for later:

11. Thumbnails in «آگهی‌های من» (needs an API field).
12. "Send for review" that saves first.
13. A formatted Persian price input.
14. A logout control on the dashboard; today «خروج» exists only on Home and stays
    there.

## 12. Non-goals

- No change to the API, database, routes, auth, moderation or chat logic.
- No new page, feature, copy rewrite or translation beyond §7 and §11.
- No two-pane desktop messenger, no image carousel, no autoplay.
- No change to CI gates, budgets or the test matrix.
- No real-device or field measurement claim; Lighthouse CI is not a phone.

## 13. Acceptance criteria

1. Every page in §7 matches its description in both themes at 375, 768, 1024 and
   1440px, and the three mocked rounds match their reference images in structure,
   colour and proportion. The owner checks a live preview before close-out.
2. `tokens.css` holds exactly the values in §3; `theme.spec.ts` pins them.
3. In `app/`: three font weights; the seven sizes plus display; radii from the
   five tokens (plus circles and the chat bubble's tight corner); two shadows and
   two glows; no emoji or text-glyph icon; no icon outside the sprite except
   Shabdarak.
4. The full CI workflow passes on the final commit with unedited gates: typecheck,
   build, API suite, Playwright, client-JS budget, Lighthouse.
5. Only the test edits in §9 and those listed in the approved plan were made.
6. `MASTER.md` and `docs/STATUS.md` describe what shipped, including measured
   client-JS, inline-CSS and Lighthouse numbers from CI.
