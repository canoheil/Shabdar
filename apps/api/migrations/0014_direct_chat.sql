-- 0014: direct two-user chat (approved spec 2026-11-04, §4).
--
-- A conversation belongs to one canonical unordered user pair; a listing is
-- optional per-message context, never conversation ownership. Read state is a
-- per-participant high-water mark, so there is no per-message read_at column
-- and no mass update path. `updated_at` is a synchronization clock only;
-- Inbox recency ordering uses `last_message_at`.
--
-- `conversations.last_message_id` and `messages.conversation_id` form an
-- intentional cycle. conversations is therefore created without its
-- last-message foreign key, messages is created next, and the key is added by
-- ALTER TABLE at the end of this migration. Scratch resets must disable
-- FOREIGN_KEY_CHECKS on one pinned connection, drop children first, and restore
-- the setting before releasing the connection.

CREATE TABLE conversations (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,
  user_low_id BIGINT UNSIGNED NOT NULL,
  user_high_id BIGINT UNSIGNED NOT NULL,
  initiator_id BIGINT UNSIGNED NOT NULL,
  last_message_id BIGINT UNSIGNED NULL,
  last_message_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL,
  updated_at DATETIME(3) NOT NULL,
  CONSTRAINT chk_conversations_ordered_pair CHECK (user_low_id < user_high_id),
  CONSTRAINT chk_conversations_last_message_pair CHECK (
    (last_message_id IS NULL AND last_message_at IS NULL)
    OR (last_message_id IS NOT NULL AND last_message_at IS NOT NULL)),
  CONSTRAINT uq_conversations_pair UNIQUE (user_low_id, user_high_id),
  CONSTRAINT fk_conversations_low FOREIGN KEY (user_low_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_conversations_high FOREIGN KEY (user_high_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_conversations_initiator FOREIGN KEY (initiator_id) REFERENCES users(id) ON DELETE CASCADE,
  INDEX idx_conversations_low_recency (user_low_id, last_message_at, id),
  INDEX idx_conversations_high_recency (user_high_id, last_message_at, id),
  INDEX idx_conversations_low_sync (user_low_id, updated_at, id),
  INDEX idx_conversations_high_sync (user_high_id, updated_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE messages (
  id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
  public_id CHAR(26) NOT NULL UNIQUE,
  conversation_id BIGINT UNSIGNED NOT NULL,
  sender_id BIGINT UNSIGNED NOT NULL,
  listing_id BIGINT UNSIGNED NULL,
  client_message_id CHAR(36) NOT NULL,
  body VARCHAR(1000) NOT NULL,
  created_at DATETIME(3) NOT NULL,
  CONSTRAINT uq_messages_client_id UNIQUE (conversation_id, sender_id, client_message_id),
  CONSTRAINT fk_messages_conversation FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
  CONSTRAINT fk_messages_sender FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_messages_listing FOREIGN KEY (listing_id) REFERENCES listings(id) ON DELETE SET NULL,
  INDEX idx_messages_conversation_order (conversation_id, id),
  INDEX idx_messages_listing (listing_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE conversation_reads (
  conversation_id BIGINT UNSIGNED NOT NULL,
  user_id BIGINT UNSIGNED NOT NULL,
  last_read_message_id BIGINT UNSIGNED NULL,
  last_read_at DATETIME(3) NULL,
  PRIMARY KEY (conversation_id, user_id),
  CONSTRAINT chk_conversation_reads_pair CHECK (
    (last_read_message_id IS NULL AND last_read_at IS NULL)
    OR (last_read_message_id IS NOT NULL AND last_read_at IS NOT NULL)),
  CONSTRAINT fk_conversation_reads_conversation FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
  CONSTRAINT fk_conversation_reads_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  -- Cascade: a removed message takes its read row with it. SET NULL would
  -- leave last_read_at set and violate the paired CHECK above, and the whole
  -- delete graph must stay cascade-only so no path can be blocked by a
  -- RESTRICT inside the intentional conversation/message cycle.
  CONSTRAINT fk_conversation_reads_message FOREIGN KEY (last_read_message_id) REFERENCES messages(id) ON DELETE CASCADE,
  INDEX idx_conversation_reads_user (user_id, conversation_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Added last so the intentional conversation/message cycle can be created on an
-- empty schema. Deletion of individual messages is not exposed in this MVP.
ALTER TABLE conversations
  ADD CONSTRAINT fk_conversations_last_message
  FOREIGN KEY (last_message_id) REFERENCES messages(id);
