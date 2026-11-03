-- One admin-managed house-ad slot and private, internal advertising enquiries.
-- Advertising content is deliberately separate from listings and feed cursors.

CREATE TABLE advertising_slots (
    id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    public_id CHAR(26) NOT NULL UNIQUE,
    title VARCHAR(100) NOT NULL,
    body VARCHAR(500) NOT NULL,
    cta_label VARCHAR(80) NOT NULL,
    cta_url VARCHAR(500) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME(3) NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    CONSTRAINT chk_advertising_slot_singleton CHECK (id = 1)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE advertising_requests (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    public_id CHAR(26) NOT NULL UNIQUE,
    submission_id CHAR(36) NOT NULL UNIQUE,
    contact_name VARCHAR(80) NOT NULL,
    phone_e164 VARCHAR(16) NOT NULL,
    details VARCHAR(1000) NULL,
    status ENUM('new', 'in_progress', 'closed') NOT NULL DEFAULT 'new',
    handled_by BIGINT UNSIGNED NULL,
    handled_at DATETIME(3) NULL,
    created_at DATETIME(3) NOT NULL,
    updated_at DATETIME(3) NOT NULL,
    CONSTRAINT fk_advertising_request_handler
        FOREIGN KEY (handled_by) REFERENCES users(id) ON DELETE SET NULL,
    INDEX idx_advertising_requests_status_created (status, created_at, id),
    INDEX idx_advertising_requests_created (created_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- A useful, restrained house-ad default. The idempotent reset used by browser
-- fixtures removes it so existing listing scenarios stay independent.
INSERT INTO advertising_slots
    (id, public_id, title, body, cta_label, cta_url, enabled, created_at, updated_at)
VALUES
    (1, '01K9Z6H8M4F2G0J1P3Q5R7T9V0', 'کسب‌وکارتان را به همسایه‌ها معرفی کنید',
     'این جایگاه برای معرفی کسب‌وکارهای محلی در شبدر است؛ بدون بنر شلوغ و مزاحمت.',
     'ثبت تبلیغ در شبدر', '/advertising', TRUE, NOW(3), NOW(3));
