# Direct Chat MVP Design — Seller Threads with Listing References

**Date:** 2026-11-04

**Status:** The user reviewed and approved the four conversational design sections. Written-spec review is still required before an implementation plan is written. No product implementation or merge is authorized by this document.

**Branch:** `arena/c2c8a31c-shabdar`

**Baseline:** National-location implementation and documentation through `3ec9ca9cca16e71f7e60a5b4e259fdb68ae74427`; the complete real-stack gate passed for code candidate `37b796731f6fbf3ad4bb668bdf5d1bbfef9116dc` in CI `41643876582`.

## 1. Intent and accepted product decisions

Shabdar needs a small, dependable, Telegram-like text-chat experience. A listing is the entry point and optional context for a message; it is not the owner of the conversation. The conversation belongs to the two people.

The user made these explicit choices during design:

1. There is **one persistent conversation between two users**, even when they contact one another about multiple listings or reverse buyer/seller roles later.
2. A message can carry a reference to the listing that prompted it. Repeated contact clicks do not create system messages or duplicate references.
3. Blocking stops new messages in both directions. Existing history remains readable to both parties and becomes writable again only when no directed block remains.
4. The first version includes an Inbox, text bubbles and timestamps, listing-reference cards, unread counts, Seen for the sender, block enforcement, and polling.
5. Typing/online presence, Reply, Edit, Delete, attachments, push notifications, WebSocket/SSE, external chat services, end-to-end encryption, and automatic message deletion are not in this version.

Success means a buyer can start from a publicly visible active listing, a seller can answer from the Inbox, both parties receive new messages through real polling, and unread/Seen state changes only after confirmed rendering and server acknowledgment. Network failure, blocking, listing-state changes, or pagination must not fabricate a message, a read receipt, or a successful mutation.

## 2. Existing foundations and constraints

The implementation must follow the existing Rust/Axum/MariaDB/Nuxt architecture:

- Runtime `sqlx` queries, additive migrations, existing ULID public identifiers, standard JSON error envelopes, session authentication, and double-submit CSRF for mutations.
- Existing directed `blocks` rows and `BlockControl`, extended only where chat enforcement requires it. Comment behavior and report-alone semantics do not change.
- Existing process-local fixed-window rate limiter and its documented single-instance limitation.
- Persian/RTL, responsive light/dark themes, visible focus, reduced motion, 44px touch targets, plain semantic surfaces, and no unnecessary gradients, shadows, animation, or decorative controls.
- Public SSR and immediate content remain intact. Private message content is not placed in anonymous/public SSR HTML. Private pages are `noindex` and all private API responses use `private, no-store` with `Vary: Cookie`.
- Browser mutations are confirmed-response-only. Drafts, confirmed messages, and cursors survive failures. There is no optimistic message bubble, unread reset, block state, or fake delivery/notification state.
- Existing tests, accessibility checks, Lighthouse gates, and client-JS budgets remain independent and unchanged. Chat code is route-split; admin code remains out of public Home/Detail chunks.
- No VPS, SMS provider, external chat service, or new infrastructure is required.

## 3. Conversation and listing-reference model

### 3.1 One pair, one conversation

A conversation stores two user IDs in canonical numeric order. The pair is unique regardless of who initiates or which party is the seller on a later listing. `initiator_id` records only who created the conversation; it does not restrict later participation.

A user cannot start a conversation with themselves. Only a logged-in, non-seller user can start from a listing that is active and publicly visible under the existing image-review and deletion rules. Unknown or non-public listing reads retain the established 404/410 behavior.

Repeated starts return the existing pair conversation. Empty conversations are valid direct destinations while the first message is being composed, but they are omitted from the Inbox until a first confirmed message exists.

### 3.2 Listing references are message context

`conversations` deliberately has no `listing_id`. A message may have one nullable `listing_id` reference.

The flow for another listing by the same seller is:

1. The user chooses “گفتگو با فروشنده” on the listing.
2. The server returns the same pair conversation.
3. No message or system event is created by that click.
4. If the user sends a message through that entry flow, that one outgoing message stores the listing reference.

Ordinary messages composed from the Inbox have no listing reference unless the user entered through a listing contact flow. A reference is valid only when the referenced listing belongs to the other participant and the sender is not its seller. Existing sold, archived, removed, or soft-deleted listings can remain references in an existing conversation; their current state affects link availability, not the right to continue the pair conversation.

The API returns listing title, slug, and current link availability with a reference. A reference links only when the listing is currently public under the same visibility rules as Detail. Otherwise it is a plain, non-clickable context card. No phone number, internal database ID, seller-only data, or moderation reason is exposed.

### 3.3 Listing lifecycle does not own the thread

After creation, a conversation does not become read-only merely because a referenced listing becomes sold, archived, removed, rejected, or soft-deleted. Only participant access, account status, and the block contract control messaging. A removed listing reference remains as unavailable context; it does not delete message history.

## 4. Additive schema — migration `0014_direct_chat.sql`

Historical migrations, including national migration `0013`, remain unchanged. The new schema is:

### `conversations`

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id CHAR(26) UNIQUE NOT NULL`
- `user_low_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `user_high_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `initiator_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `last_message_id BIGINT UNSIGNED NULL` FK → `messages.id` (the physical FK may be added after message-table creation in the same migration)
- `last_message_at DATETIME(3) NULL`
- `created_at DATETIME(3) NOT NULL`
- `updated_at DATETIME(3) NOT NULL` — synchronization only; never used as Inbox recency order
- `CHECK (user_low_id < user_high_id)`
- `CHECK` requiring `last_message_id` and `last_message_at` to be both NULL or both non-NULL
- `UNIQUE (user_low_id, user_high_id)`
- Indexes `(user_low_id, last_message_at, id)` and `(user_high_id, last_message_at, id)` for ordered Inbox pages, plus `(user_low_id, updated_at, id)` and `(user_high_id, updated_at, id)` for synchronization

### `messages`

- `id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY`
- `public_id CHAR(26) UNIQUE NOT NULL`
- `conversation_id BIGINT UNSIGNED NOT NULL` FK → `conversations.id` (`ON DELETE CASCADE`; operational deletion is not exposed in this MVP)
- `sender_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `listing_id BIGINT UNSIGNED NULL` FK → `listings.id`
- `client_message_id CHAR(36) NOT NULL`
- `body VARCHAR(1000) NOT NULL`
- `created_at DATETIME(3) NOT NULL`
- `UNIQUE (conversation_id, sender_id, client_message_id)`
- Index `(conversation_id, id)` for older/newer keyset reads
- Index `(listing_id)` for the listing-reference join/FK path
- No per-message `read_at`; read state is high-water state below

The client ID is a canonical UUID generated by the browser for one logical send. The server validates its shape, body, listing reference, and conversation before writing.

### `conversation_reads`

- `conversation_id BIGINT UNSIGNED NOT NULL` FK → `conversations.id` (`ON DELETE CASCADE`)
- `user_id BIGINT UNSIGNED NOT NULL` FK → `users.id`
- `last_read_message_id BIGINT UNSIGNED NULL` FK → `messages.id`
- `last_read_at DATETIME(3) NULL`
- Primary key `(conversation_id, user_id)`
- `CHECK` requiring `last_read_message_id` and `last_read_at` to be both NULL or both non-NULL

Both participant rows are inserted with NULL read state when the conversation is created. An actual new message, read-state advance, block, or unblock advances `conversations.updated_at`; idempotent/no-op requests do not. Inbox ordering still uses only `last_message_at`, while synchronization uses `updated_at` so read/block changes are not missed.

The service verifies that any referenced read/message IDs belong to the same conversation and that message senders are participants; these cross-row invariants are not left to a polymorphic database assumption.

Migration/reset/drop helpers must include the new tables in FK-safe order. Existing listing, block, user, and national-reference identities are not rewritten.

## 5. API contract

All paths below are under `/api/v1`. Non-participant conversation reads return the same 404 as an unknown conversation. Responses containing participant or message data are `Cache-Control: private, no-store` and `Vary: Cookie`.

### 5.1 Start or return a pair conversation

`POST /listings/{slug}/conversations` — User + CSRF

- Requires a publicly visible active listing, a non-seller caller, active participants, and no block in either direction.
- Atomically finds or creates the canonical pair and both read rows.
- Returns conversation metadata, not an empty fabricated message.

Conversation metadata:

```json
{
  "public_id": "01…",
  "other_party": { "public_id": "01…", "display_name": "…" },
  "can_message": true,
  "viewer_blocked_other": false,
  "last_message_at": null,
  "created_at": "2026-11-04T00:00:00Z"
}
```

### 5.2 Inbox

`GET /me/conversations?before=…&after=…&limit=…` — User

- Default 20, maximum 50; only conversations with a last message. `before` and `after` are mutually exclusive.
- Initial and `before` reads use newest `(last_message_at, id)` keyset order and return an `older_cursor`. They also return a separate `sync_cursor` based on `(updated_at, id)`.
- `after=<sync_cursor>` returns changed non-empty conversations in `(updated_at, id)` forward pages with `next_sync_cursor`/`has_more_after`; the browser follows every page. This catches new messages, Read changes, and block changes without reordering the Inbox on a Read.
- Each item contains conversation metadata, `unread_count`, and a compact `last_message` with public ID, sender public ID, plain-text preview, timestamp, and optional listing reference.
- No phone number, canonical low/high IDs, read-state rows, or internal IDs are returned.
- There is no global Shell unread request or badge. Counts are Inbox/thread state only.

### 5.3 Conversation metadata and messages

`GET /conversations/{public_id}` — Participant

Returns the same participant-specific metadata, including `can_message` and the caller’s directed block state. A block never removes read access.

`GET /conversations/{public_id}/messages?before=…&after=…&limit=…` — Participant

- Default 30, maximum 50; `before` and `after` are mutually exclusive. Keysets use internal message ID order inside the authorized conversation, never offsets or timestamps alone; IDs remain absent from the payload.
- Initial reads return the latest page; `before` returns the immediately older page and `after` returns the immediately newer page. Every response orders `data` oldest → newest within that page and provides `{older_cursor,newer_cursor,has_more_older,has_more_newer}` so forward catch-up can continue beyond one page.
- The response envelope also contains current conversation metadata and `{viewer_read_through,other_read_through}`. Thus every poll updates block state and Seen on already-rendered own messages even when no new message arrives.
- Newer polling uses the last confirmed message cursor. The browser orders and deduplicates by message public ID rather than trusting response uniqueness.
- A message contains `public_id`, `sender_public_id`, `body`, `created_at`, optional listing reference, and sender-specific Seen state. Seen is meaningful only on the caller’s own messages.

### 5.4 Send a message

`POST /conversations/{public_id}/messages` — Participant + CSRF

Body:

```json
{
  "client_message_id": "123e4567-e89b-42d3-a456-426614174000",
  "body": "سلام، آیا هنوز موجود است؟",
  "listing_slug": "optional-persian-slug"
}
```

- Trim the body; require 1–1000 Unicode scalar characters. Reject disallowed controls other than newline, unknown fields, malformed UUIDs, and invalid listing references.
- Return the confirmed message and updated conversation metadata.
- A duplicate ID with identical conversation, sender, body, and listing reference returns the original message without changing `last_message_at`, `updated_at`, unread counts, or Inbox order.
- Reusing an ID with different content returns `400 validation_error` with `fields.client_message_id = "conflict"`.
- Message creation, idempotency lookup, conversation last-message update, and participant checks are one transaction.
- Default abuse allowance: 60 validated send requests per user per 600 seconds, including idempotent replays so retries cannot bypass the limit; malformed, unauthorized, and blocked requests do not consume the validated-send allowance. Configure it as `MESSAGE_USER_LIMIT` and `MESSAGE_USER_WINDOW_SECS`. A 429 uses the existing `Retry-After` contract. The limiter remains process-local and must move to shared storage before multiple API replicas.

### 5.5 Mark rendered messages read

`POST /conversations/{public_id}/read` — Participant + CSRF

Body: `{ "through_message_id": "01…" }`.

- The browser supplies only the latest confirmed message it actually rendered. The server verifies caller participation and that the cursor message belongs to the same conversation; actual viewport/render position remains a browser responsibility, not a claim the server can independently observe.
- Advance the caller’s high-water mark only forward. An older or duplicate request is a no-op and returns the current confirmed state.
- Update `last_read_at` and the conversation’s synchronization `updated_at` only for an actual advance; never change `last_message_at` for a Read.
- Return current `read_through`, `read_at`, and the caller’s resulting `unread_count`.
- Reads remain allowed while blocked because history remains readable.
- A message is Seen for its sender when the other participant’s high-water message ID is at least that message’s ID. IDs are compared only inside one conversation.

## 6. Atomicity, blocking, and race behavior

### 6.1 Block/send linearization

A directed block in either direction sets `can_message = false` and rejects conversation starts and sends with `403 conversation_blocked`. History, message pagination, and read marking remain available.

Block insertion/removal and chat start/send use a shared canonical user-row lock order, followed by conversation locks where needed. A confirmed block/unblock also advances that pair conversation’s `updated_at` when it exists so Inbox polling observes the metadata change. This establishes a clear order:

- If the send transaction commits first, that message precedes the block.
- If the block commits first, the send observes it and is rejected.

There is no optimistic browser transition. After a block/unblock response, the browser refreshes conversation metadata; unblocking an outgoing block does not falsely enable sending if a reverse block remains. Existing comment block behavior remains unchanged.

### 6.2 Idempotency and concurrent sends

The pair uniqueness and `(conversation_id, sender_id, client_message_id)` uniqueness are database boundaries. Transactions lock/read the existing pair or message so concurrent retries cannot create duplicates or move the Inbox twice. Deadlock and transaction-wide MariaDB 1020 handling retries the whole transaction using the established operational rule, never only one statement.

### 6.3 Read versus arriving message

The browser sends only the latest message it actually rendered. The server advances only through that message. A message inserted afterward remains above the high-water mark and therefore unread, even if its arrival races the read request.

## 7. Browser experience

### 7.1 Entry and authentication

- Listing Detail gains “گفتگو با فروشنده” near the existing save action for non-owners.
- The seller never sees a contact-self action.
- Anonymous users are sent to login with an internal-only validated `next` target that resumes the contact action. External, protocol-relative, credential-bearing, or malformed redirect targets are rejected in favor of a safe default.
- Starting uses the real POST endpoint and shows loading only during that genuine wait. Failure retains a retry action and does not navigate to a false thread.
- A successful start navigates to `/me/conversations/{public_id}?listing={slug}`. That internal query is the pending reference for the next logical send only; it is not stored as conversation state and creates no event. The composer shows a dismissible listing-context chip, sends the slug with the next message, and removes the query/chip only after confirmation or explicit dismissal. A failed send retains both draft and pending reference; an ordinary Inbox entry has no pending reference.

### 7.2 Inbox — `/me/conversations`

Each row shows the other party’s display name or neutral fallback, last-message preview, Persian timestamp, and unread count. Rows are keyboard-operable links. Empty, initial-loading, initial-error/retry, and older-page-error states are explicit.

Older pages append only confirmed, previously unseen conversation IDs. Polling follows every forward `after` page, updates existing rows and prepends genuinely new conversations without resetting the older-page cursor or duplicating rows. Empty conversations do not appear.

### 7.3 Thread — `/me/conversations/{public_id}`

- Header: other party, Inbox back link, and block/unblock control with a chat-appropriate label.
- Messages: simple RTL bubbles with distinct own/other alignment, Persian day separators and timestamps, plain escaped text with preserved newlines, and optional listing cards.
- The caller’s own messages display Seen only from confirmed high-water state. There is no typing, online, delivered, or fake pending indicator.
- Composer: labelled textarea, Unicode-aware 1000-character counter, Enter to send and Shift+Enter for newline. The send control is disabled during one in-flight send; the draft is not cleared until confirmation.
- A blocked thread replaces the composer with a read-only explanation while retaining the local draft. Unblock restores sending only if refreshed metadata confirms no reverse block.
- Private data loads after authentication on the client. The SSR shell remains immediate and contains no private message payload. Routes emit `noindex, nofollow, noarchive` metadata.

### 7.4 Navigation and route boundaries

“پیام‌ها” is added as a sixth primary-navigation action for all viewers; anonymous users reach the normal login/empty authenticated state. To keep six controls safe at 375px, the labels “ثبت آگهی” and “علاقه‌مندی‌ها” become “ثبت” and “ذخیره‌ها”. Existing destinations and admin-only visibility do not change.

There is no Shell-level chat polling or unread badge. Chat pages and components are route-split. Any Detail/Shell additions must fit the unchanged public client-JS ceilings; a budget failure requires optimization, never a higher limit.

## 8. Polling, scrolling, and Seen

Polling exists only while an authenticated Inbox/thread page is active:

- Thread: every 10 seconds.
- Inbox: every 30 seconds.
- No overlapping request; a new interval is skipped while the prior request is pending.
- Hidden tabs and offline browsers pause. Visibility/online restoration immediately resumes from the last confirmed cursor.
- Route exit cancels timers and stale responses cannot mutate a newer conversation.
- Poll failures retain all confirmed messages, conversations, and cursors. They expose a restrained stale/error state and manual retry without creating a retry storm.
- A thread poll applies the returned high-water metadata to existing own messages for Seen and refreshes `can_message`; Inbox forward polling follows all available update pages before scheduling the next interval.

On initial thread load, the latest page is rendered before a read request is considered. While the tab is visible and the user is at the latest position:

1. Confirmed new messages are merged and rendered.
2. The browser requests Read through the latest rendered message.
3. Only a confirmed response updates unread/Seen-related state.

If the user is reading older history, polling does not jump scroll position or mark new messages read. A “پیام‌های تازه” control shows the confirmed new count; activating it brings the latest messages into view before Read is sent. Older-page prepends preserve the user’s anchor and scroll offset.

All incoming pages and polls are deduplicated within the response and against every confirmed rendered message. Cursor metadata comes only from the real server response, never from an advertising row, local guess, or injected duplicate.

## 9. Failure and security behavior

| Situation | Required behavior |
|---|---|
| 401/expired session | Preserve confirmed history and draft; offer login with a safe resume target. |
| 403/block during send | Preserve draft, keep history readable, refresh metadata, and show read-only state only after the confirmed error/state. |
| 404/non-participant | Show a neutral unavailable conversation; do not reveal participants or messages. |
| 429 | Preserve draft and show the real retry delay; do not retry in a tight loop. |
| Network/500 on send | Do not append a message; retain draft and retry with the same client ID. |
| Lost success response | Retry with the same client ID; the server returns the original row and the browser renders one message. |
| Poll/read failure | Keep confirmed items, cursor, unread state, and draft; retry later or manually. |
| Pagination failure | Keep the current page and previous cursor for an identical retry. |
| Malicious HTML/Markdown | Render as plain text; scripts and markup never execute. |
| Listing becomes unavailable | Keep conversation and reference text; remove only link availability. |

Message bodies are plain text in the database and UI, not HTML or Markdown. Public IDs never expose sequential database IDs. Server logs and tests must not print real phone numbers, session cookies, message bodies, or production secrets.

## 10. Verification and acceptance

Implementation follows observable RED → GREEN slices. Formatting, compilation, or inspection alone is not behavioral evidence.

### 10.1 Migration and real API/MariaDB tests

- Empty-schema migration and FK-safe reset/drop; existing national and listing rows remain intact.
- Pair uniqueness in both initiation orders, no self-chat, active/public start rules, and empty-Inbox omission.
- Multiple listing references in one pair thread, reverse buyer/seller roles, invalid/foreign references, and unavailable-reference presentation data.
- Participant-only metadata/messages with 404 for non-participants and no anonymous private payload.
- Identical/concurrent idempotent sends, conflicting ID reuse, last-message ordering, body/UUID validation, CSRF, and rate limiting.
- Block in both directions, history/read access while blocked, unblock with reverse-block handling, and concurrent block/send ordering.
- Sold/archived/removed/soft-deleted listing transitions do not freeze an existing pair thread.
- Before/after keysets with equal timestamps, deleted/moved Inbox anchors, and forward catch-up beyond one page.
- Inbox synchronization observes more than one page of message, Read, and block changes; a Read never changes last-message ordering and no-op requests do not advance synchronization.
- High-water monotonicity, duplicate/older Reads, and a message arriving concurrently with Read remaining unread.
- Sender Seen and per-conversation unread counts with no cross-user leakage.

### 10.2 Real-stack Playwright

Use separate real buyer and seller browser contexts and the actual Axum/MariaDB/Nuxt stack:

- Start from two listings by one seller, reach the same thread, and show distinct listing-reference cards only on the intended confirmed messages.
- Login resumes the contact action; self-contact is absent.
- Inbox in both directions, real 10-second thread polling and 30-second Inbox polling, no duplicate rows, and no overlapping/hidden-tab polls.
- Unread count, visible latest-message Read, sender Seen, older-history reading without false Seen, and new-message control/scroll anchor behavior.
- Send failure, lost success response, same-ID retry, 401/403/429/500/offline, and draft/cursor preservation.
- Block/unblock through real controls/API, read-only history, and reverse-block correctness.
- More than one page of real messages and Inbox rows, legal overlap/repeated rows, confirmed cursor retention, and deduplication against confirmed items.
- Persian text, Unicode boundaries 1000/1001, multiline text, long unbroken content, XSS strings, and no HTML execution.
- Keyboard operation, focus management, accessible names/live announcements, light/dark, reduced motion, 375/768/1024/1440 layouts, six safe navigation actions, no overflow, and 44px targets.

Failure injection may drop/delay responses or add legal duplicate rows to prove browser resilience, but primary behavior and all persistence assertions use the real API and database. Tests are never replaced by inspection or mocked chat state.

### 10.3 Existing independent gates

Every existing test plus the new tests must pass, along with Rust formatting/lint, the full API suite, Nuxt typecheck/build, real-stack startup, Playwright, three-run Lighthouse medians, and both client-JS budget checks. Public Home/Detail chunk boundaries and the absolute 100,000-byte route ceilings remain unchanged. Chat routes receive the same accessibility/quality scrutiny without weakening public-route budgets.

## 11. Documentation and rollout

The implementation plan must update API, DATABASE, PRODUCT, ARCHITECTURE, package READMEs, and STATUS to replace the old listing-owned conversation proposal with this pair-owned model. It must document idempotency, block locking, high-water reads, polling, private caching, rate-limit deployment limitations, and migration `0014`.

Rollout is an additive database migration followed by compatible API/web deployment. No production migration, PR creation, merge, chat notification, or launch claim is implied by design approval. Backups and a real production rollout remain separate operational steps.

## 12. Explicit non-goals

- Listing-owned parallel threads, arbitrary group chats, or a public user directory.
- Attachments, voice/video, rich media, Markdown/HTML messages, Reply/Edit/Delete, typing/online presence, or message search.
- Push notifications, WebSocket/SSE, external chat providers, E2EE, automatic retention/deletion, or distributed rate-limit infrastructure.
- Admin rebuild, message-report/moderation workflow, or changes to accepted comment/report visibility rules.
- Fake delivery, fake unread counts, optimistic browser success, or claiming polling is real-time.

There are no open design questions in this approved scope. Any later request for one of these non-goals starts a new design and implementation cycle.
