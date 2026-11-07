# Shabdar Product Brief

> **خلاصه فارسی:** شبدر یک بازارچه آگهی برای سراسر ایران با رابط فارسی و راست‌به‌چپ است. استان و شهر از داده‌های مرجع ۳۱ استان انتخاب می‌شوند؛ رشت و گیلان نیز مانند سایر شهرها و استان‌ها در دسترس‌اند. هر آگهی قبل از نمایش عمومی باید توسط ادمین تأیید شود. کاربران می‌توانند آگهی‌ها را امتیاز بدهند، کامنت بگذارند و موارد نامناسب را گزارش کنند. فیلتر قطعی دیدگاه فارسی با قوانین مدیریت‌شده و صف بازبینی پیاده‌سازی شده است؛ پیش از ادعای عبور corpus فعلی از این مانع انتشار، بررسی کامل و رسیدگی به backlog لازم است.

## 1. Product intent

Shabdar is a Persian-language, mobile-first classifieds marketplace for Iranian buyers and sellers. It makes local discovery and direct contact simple, shows prices in toman, and builds trust through admin-reviewed listings, public ratings, and user reporting.

The interface is RTL-first, works on small screens, supports light and dark themes, and keeps listing pages crawlable through server-rendered HTML.

National coverage is the product baseline: all 31 provinces and their reference cities are available for discovery and posting. Rasht/Gilan have no special product restriction or launch-only positioning.

The previous version is not available. The feature list from the user's screenshots and notes is the only product reference. Details not in that list are proposed defaults.

## 2. Users and jobs

- **Buyer:** finds an item or service by keyword, category, city, or price; reads details, ratings, and comments; saves it; contacts the seller.
- **Seller:** verifies a mobile number, creates a listing with photos and location, waits for approval, responds to messages, and manages listings from the dashboard.
- **Commenter/rater:** logged-in user who comments on a listing or rates somebody else’s listing.
- **Reporter:** any logged-in user who reports a listing, comment, or user that breaks the rules.
- **Admin:** reviews listings and reports, manages literal comment rules, resolves automatic comment-review cases, runs existing-comment rescans, and can hide/remove content or suspend accounts.

## 3. MVP scope (v1)

### Accounts
- Passwordless login and registration with Iranian mobile number and six-digit SMS OTP.
- Separate login and registration flows, editable phone number before a code is sent.
- Minimal profile: display name.
- Blocking another user.

### Listings and moderation
- Create, edit, and submit a listing for review. Fields: category, province, city, title, description, price in toman (or free, or negotiable), photos. Province is selected first; changing it clears the previous city selection.
- Every listing starts as `draft`, then goes to `pending` when submitted. Only an admin can move it to `active` (approved) or `rejected` (with a reason).
- Seller actions after approval: mark as sold, archive, or delete (soft).
- Admin moderation queue: pending listings plus a separate legacy image-review backlog. Detail shows every current image; exact human image approval publishes/restores, while rejection/removal keeps its required reason.

### Discovery
- Public home feed of active listings only.
- Sort options: newest, cheapest, most expensive, popular.
- "Popular" = weighted average rating (see section 6). Not raw views.
- Advanced filters: category, province, city, price range. Province and city are represented in URL query state; a city is loaded only for its selected province.
- Category pages and city pages with clean slug URLs.
- Listing detail page: title, price, city, category, photos, seller summary, average rating, comments, and contact/chat button.
- Search by keyword (basic, title and description). Persian-specific search quality is tested before promising more.

### Engagement
- **Favorites:** save and unsave listings; favorites page for the logged-in user.
- **Ratings:** logged-in users rate a listing from 1 to 5 stars. One rating per user per listing; cannot rate own listing; can change their rating.
- **Comments:** logged-in users comment on a listing. Clean comments publish immediately; a deterministic rule match makes the comment non-public pending admin review (see section 6).
- **Reports:** any logged-in user can report a listing, a comment, or a user. Reported content is reviewed by an admin.
- **Chat:** one conversation per buyer/seller **user pair**, not per listing. A listing is the entry point and an optional per-message reference, so contacting the same seller from another listing continues the same thread. Text messages only, RTL bubbles with Persian day separators and timestamps, listing-reference cards, unread counts, sender Seen, block enforcement, idempotent sending, and polling (10 seconds in a thread, 30 seconds in the Inbox).

### Dashboard (seller)
- Counts: active listings, favorites, sold.
- Listing table with filters: all, active, sold, pending, rejected.
- Add listing button.
- Views and weekly activity chart are deferred (see section 4).

### Admin
- Separate admin area with its own layout and API namespace.
- Listing moderation queue (pending listings with flags, if any) and active legacy image backlog. Accessible RTL detail requires every loaded image to be confirmed, shows dimensions/reviewer attribution, preserves state on errors, and never publishes optimistically.
- Report queue: open reports with target, reason, and details; actions: hide comment, remove listing, suspend user, or dismiss report.
- Comment-rule administration and a separate automatic-review queue with confirm-hidden/restore, safe matched-rule snapshots, and revision-checked rescan batches. Report restore cannot bypass an active automatic case.
- Every admin action is written to the audit log.

### Global
- Responsive layout: desktop sidebar and mobile bottom navigation (home, search, post listing, messages, account).
- Dark mode, notification bell (basic in-app list), loading/empty/error states.
- SEO: server-rendered listing pages, canonical slug URL, meta tags, sitemap, basic JSON-LD for Product/Offer.
- Listings removed by admin or seller return HTTP `410 Gone` (so search engines drop them).

## 4. Deferred (not in v1)

- **Semantic, fuzzy, phonetic, transliteration, and external moderation services.** Phase 10 ships deterministic literal comment rules instead; any broader detection upgrade needs separate scope and evidence.
- Automated image moderation (AI or external service). Phase 11 local decode validates files only and human review determines suitability.
- Image thumbnails, resizing, metadata stripping, and AVIF/modern derivative generation; these need a separate optimization scope and measurements.
- Nearby sellers map and radius search (the feature exists in the old version and returns in a later phase, with privacy rules).
- Paid ad reservations, billing, and ad inventory. The feed slot can be shown as a disabled placeholder until then.
- Seller reviews separate from ratings (text reviews with eligibility rules).
- Full support ticket system. v1 has a simple contact form only if needed.
- Dashboard views counter and weekly activity chart (needs a defined event model).
- Push notifications, real-time transport (WebSocket/SSE), typing/online presence, reply/edit/delete, attachments, end-to-end encryption, message moderation, and automatic deletion.
- Recommendations and personalized ranking.
- S3-compatible storage and multiple storage backends.

## 5. Core user flows

### A. Publish a listing
1. Logged-in seller opens "Post listing", chooses a province and then a city, and fills in category, title, description, price, and photos.
2. Seller saves as draft or submits for review. Submit moves the listing to `pending`.
3. Seller sees the status in the dashboard ("waiting for review").
4. Admin opens the queue and listing detail, reviews every loaded current image, and either confirms the exact set or rejects with a reason. Broken/unloaded images cannot be confirmed.
5. Approved listing becomes `active` with reviewer/timestamp/exact-ID attribution and appears in public feeds. Rejected listing shows the reason to the seller, who can edit and resubmit; image edits or resubmission require fresh review.

### B. Rate and comment
1. Logged-in user opens an active listing.
2. Selects 1 to 5 stars. The average rating updates. Users cannot rate their own listing.
3. Writes a comment. A clean comment is published immediately; a matched comment returns pending-review feedback and is not publicly listed.
4. Other users can report the comment.

### C. Report content
1. User clicks "report" on a listing, comment, or user, chooses a reason, and optionally adds details.
2. The report goes to the admin queue. One user can report the same target only once.
3. Admin reviews and resolves the report: hide comment, remove listing, suspend user, or dismiss.
4. Reporter sees no personal details about the admin action.

### D. Sign in
1. User enters an Iranian mobile number, and can edit it before requesting a code.
2. Receives a six-digit SMS code (in local development, the code is printed in the terminal, never in production).
3. Enters the code. Login returns the existing account. Registration creates one only after verification.

### E. Browse and contact
1. Visitor opens the home feed or a category/city page (server-rendered).
2. Applies sort and filters. Sees loading, empty, and error states.
3. Opens a listing, reads details and comments, and saves it.
4. Starts a conversation (login required). After login, the action resumes.

## 6. Business rules

- **Listing states:** `draft` → `pending` → `active` or `rejected`. From `active`: `sold`, `archived`, or `removed` (admin). Active listings with images are public only with a current exact human image review; active zero-image listings retain public behavior.
- **Admin approval:** only admins can set `active`, and only after reviewing the exact current image set. Local decoding never approves content. Legacy active image listings remain suppressed in the re-review backlog until restored or removed.
- **Comments:** clean comments publish immediately. Deterministic Persian/Finglish literal rules normalize Arabic/Persian variants, listed controls/diacritics, repeated characters, and separator evasions; matched new or rescanned comments become hidden pending review, never automatically deleted or suspended. Authors receive pending feedback without matched-rule details. A user report alone never hides a comment. Admins resolve automatic cases or independent reports with audited transitions and exact counters. Comments longer than 1000 characters are rejected.
- **Ratings:** integers 1 to 5. One rating per user per listing. Cannot rate own listing. Changing a rating updates the averages.
- **Popular sort:** weighted average (Bayesian) so that a listing with one 5-star rating does not beat a listing with many 4.8-star ratings.
  - Formula: `score = (C × m + rating_sum) / (C + rating_count)`
  - `m` = average rating across all listings (updated periodically)
  - `C` = minimum ratings before a listing is treated as reliable (start with 5)
  - Listings with no ratings use `m` as their score.
- **Blocks:** the blocker’s authenticated comment view excludes the blocked author’s comments, and a blocked user pair cannot exchange new messages in either direction while both sides keep their read-only history. Blocking does not change anonymous SSR, global counters, or another viewer’s page; unblocking restores sending only when no reverse block remains.
- **Location:** province and city are required for every listing. The additive national migration supplies 31 provinces and 1,547 cities while preserving existing city IDs, slugs, and listing relationships. Exact coordinates are not collected in v1.
- **Moderation rollout:** migration does not claim that old comments were reviewed. Before using this filter to clear the launch blocker for the existing corpus, an admin must complete one full rescan and review the initial pending backlog. Later rule changes need another full rescan to affect existing visible comments; deactivation never auto-restores.

## 7. UX and design direction

UI UX Pro Max is the design reference for the marketplace. Direction: search-led, listing-first, flat style with strong contrast and restrained effects. Persian typography: a self-hosted Persian font with verified glyph coverage (evaluate Vazirmatn). Right-aligned RTL layout, RTL-aware icons, strong price and location hierarchy, text labels next to icons, visible focus states, reduced-motion support, touch targets at least 44 px. Test at 375, 768, 1024, and 1440 px in light and dark modes.

Design choices are proposals until the user approves the visual direction.

## 8. Success measures (proposed)

- Listing submission rate and admin approval time.
- Rejection reasons distribution (to refine the rules).
- Search-to-listing-open and listing-open-to-contact conversion.
- Comment rate and report rate per 100 listings.
- OTP delivery and verification success rate.
- Mobile Core Web Vitals and index coverage for public listings.

Do not optimize for raw page views at the cost of trust, relevance, or privacy.

## 9. Open questions and risks

See `docs/STATUS.md` for the current list and `docs/ARCHITECTURE.md` section 9 for the full risk list.
