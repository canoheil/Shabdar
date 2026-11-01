-- 0011: exact human image-review attribution (Phase 11).
-- Legacy rows deliberately stay NULL: an old listing approval is not proof
-- that a manager saw its images. Public reads and the admin backlog use these
-- fields; normal owner mutations cannot edit an active listing's image set.

ALTER TABLE listings
  ADD COLUMN image_reviewed_by BIGINT UNSIGNED NULL,
  ADD COLUMN image_reviewed_at DATETIME(3) NULL,
  ADD COLUMN image_reviewed_ids JSON NULL,
  ADD CONSTRAINT fk_listings_image_reviewer
    FOREIGN KEY (image_reviewed_by) REFERENCES users(id),
  ADD CONSTRAINT chk_listings_image_review CHECK (
    (image_reviewed_by IS NULL AND image_reviewed_at IS NULL AND image_reviewed_ids IS NULL)
    OR
    (image_reviewed_by IS NOT NULL AND image_reviewed_at IS NOT NULL AND image_reviewed_ids IS NOT NULL)
  ),
  ADD INDEX idx_listings_image_review (status, image_reviewed_at, published_at, id);
