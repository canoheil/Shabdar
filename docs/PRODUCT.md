# Shabdar Product Brief

> **خلاصه فارسی:** شبدار یک بازارچه آگهی ایرانی با رابط فارسی و راست‌به‌چپ است. نسخهٔ اولیه روی مرور و جست‌وجوی آگهی، ثبت و مدیریت آگهی، ورود با شمارهٔ موبایل، علاقه‌مندی‌ها و گفت‌وگوی خریدار و فروشنده تمرکز می‌کند؛ قابلیت‌های پرهزینه‌تر به مراحل بعد موکول می‌شوند.

## 1. Product intent

Shabdar is a Persian-language, mobile-first classifieds marketplace for Iranian buyers and sellers. It should make local discovery and direct contact simple, present prices in toman, and build trust through clear listing details, seller information, moderation, and safe interaction patterns. The interface is RTL-first, works on small screens, supports light and dark themes, and remains crawlable through server-rendered listing pages.

The old implementation is unavailable. This brief treats the supplied feature list as the sole product reference; all details not specified there are proposed defaults, not recovered requirements.

## 2. Users and primary jobs

- **Buyer:** find a relevant item or service by keyword, category, city, price, or distance; assess the offer; save it; contact its seller.
- **Seller:** verify a mobile number, publish an offer with photos and location, respond to inquiries, and track/manage listings.
- **Support/admin staff:** handle reported content and support requests through a separately authorized management interface.
- **Advertiser (later):** reserve a clearly labeled paid placement in a feed slot.

## 3. Information architecture and feature inventory

### Public marketplace
- Home feed with listing image, title, province/city, category, price in toman, and favorite action.
- Sort choices: newest, cheapest, most expensive, most popular. “Popular” needs a defined metric before implementation.
- Advanced filters; category browsing: home sweets, home food, handicrafts, clothing/sewing, plants/flowers, beauty, and education.
- Search and clean, readable listing slug URLs; listing details with seller, location, and contact/chat entry point.
- Paid ad card at feed position 1 (“Your ad here / Reserve ad”), visually distinct from organic listings.

### Account and seller tools
- Passwordless Iranian mobile sign-in using a six-digit SMS OTP, separate login and registration flows, and an edit-number step before verification.
- Favorites, notifications, account/dashboard, listing creation and management.
- Dashboard metrics: active listings, total views, favorites, sold count, weekly activity chart; product filters: all, active, sold.
- Nearby sellers with location request, radius selector (default proposal: 10 km), and OpenStreetMap/Leaflet map.
- Per-listing conversations, conversation list and messenger view.
- Seller reviews/ratings (later MVP boundary below).
- Support ticket form: subject, title, message.

### Global and operations
- Responsive desktop sidebar and mobile bottom navigation (home, search, post listing, messages, account); notification bell; dark mode; loading/empty/error states.
- Separate admin dashboard for moderation and marketplace operations.
- SEO metadata, sitemap, and JSON-LD for public listing pages.

## 4. Core user flows

### A. Discover and contact
1. Visitor opens the SSR home feed or a category/city page.
2. Searches or applies filters and sorting; results show useful loading, empty, and error states.
3. Opens a listing with a stable slug URL, sees price/location/seller and saves it or starts a conversation.
4. If unauthenticated, completes OTP login; the intended action resumes after successful verification.
5. Buyer and seller exchange messages in a conversation associated with that listing.

### B. Register / login by OTP
1. User chooses Login or Register (home page offers a login modal).
2. Enters Iranian mobile number and can edit it before requesting a code.
3. Receives and submits a six-digit SMS OTP; the server limits resend/verification attempts.
4. Registration collects only the minimum profile data needed to use the marketplace; login returns the existing account.

### C. Publish and manage a listing
1. Authenticated seller opens “Post listing” and supplies category, title, description, price in toman, city/location, and photos.
2. Preview and submit; listing enters the agreed publication/moderation state.
3. Seller finds it in dashboard, edits or marks sold, and tracks views/favorites and messages.

### D. Find nearby sellers
1. User chooses nearby sellers and explicitly grants browser location permission, or chooses an area manually.
2. Selects a radius (10 km proposed default); map and accessible result list show nearby sellers.
3. User can open the seller/listing without disclosing their exact position publicly.

### E. Support and admin (later)
User submits a support ticket and receives confirmation. Authorized staff review tickets, listings, reports, and users in the isolated admin experience; every privileged action is auditable.

## 5. Proposed MVP

### MVP 1 — useful, safe marketplace
- Passwordless OTP auth (separate login/register, edit number, rate limits).
- Public feed, category pages, listing detail pages, search, core filters, sort by newest/price.
- Seller listing create/edit/archive/mark-sold, local-disk image upload, favorites.
- Listing-associated conversations with basic text messages and unread state.
- Minimal dashboard: own listings and statuses; accurate views/favorites counts only if inexpensive to maintain.
- Responsive RTL navigation, light/dark theme, empty/loading/error states.
- SSR metadata and canonical slug URLs for listing pages; sitemap and basic JSON-LD.
- Admin-only listing moderation and account/report handling before broad public launch.

### Defer until validated
- **Paid ad reservations/billing** (retain only a disabled/clearly configured placement until inventory, pricing, payment and refund rules exist).
- Nearby-seller map and geospatial discovery; privacy, location accuracy and spatial-query behavior need validation. Launch city-based discovery first.
- Seller reviews/ratings; defer until transaction/abuse policy and eligibility are defined.
- Weekly analytics chart, popular sort, detailed view analytics and advanced dashboard metrics.
- Support ticket workflow beyond a minimal contact channel; full admin support operations.
- Push notifications, real-time chat transport, rich media chat, recommendation/personalization, and complex ranking.
- S3 migration, multiple storage backends, microservices, and speculative abstractions.

## 6. UX and design direction

UI UX Pro Max was consulted for the marketplace design system and Nuxt stack guidance. Its marketplace recommendation prioritizes search, clear category discovery, and trust cues; the recommended lightweight flat visual style favors strong contrast and restrained effects. Translate that into a Persian-first implementation: right-aligned layouts, RTL-aware icons/navigation, readable self-hosted Persian-capable typography (evaluate Vazirmatn), strong price and location hierarchy, and a compact listing-first feed. Use explicit text labels with icons where ambiguity is possible; do not rely on color alone. Preserve keyboard focus, reduced-motion preferences, sufficient contrast, and mobile touch targets. Evaluate at 375, 768, 1024, and 1440 px.

Design choices remain proposals, not final brand approval. The skill's generic Latin font recommendation is not appropriate as the sole Persian font; select and verify Persian glyph coverage and performance during design implementation.

## 7. Success measures (proposed)

- Search-to-listing-open and listing-open-to-contact conversion.
- Time to first relevant result; search empty-result rate.
- Listing completion rate and moderation rejection rate.
- OTP delivery/verification success and abuse rate.
- Message response rate, report rate, and support resolution time.
- Mobile Core Web Vitals and crawl/index coverage for public listings.

Do not optimize for raw page views at the expense of trust, relevance, or seller privacy.

## 8. Open questions and risks

See **Open questions and risks** in `docs/ARCHITECTURE.md` for the consolidated decision log. Important blockers include SMS provider/cost and OTP policy, moderation and prohibited categories, listing lifecycle, payment/ad rules, geographic coverage, privacy/retention, and the definition of “popular.”
