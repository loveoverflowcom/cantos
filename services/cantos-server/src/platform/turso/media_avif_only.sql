-- Version 30: AVIF-only active admission/selection; historical receipts stay byte-identical.
CREATE TRIGGER media_image_intents_avif_only BEFORE INSERT ON media_image_intents
WHEN NEW.content_type <> 'image/avif'
BEGIN SELECT RAISE(ABORT,'cantos:policy.avif_only'); END;
CREATE TRIGGER media_image_assets_avif_only BEFORE INSERT ON media_image_assets
WHEN NEW.content_type <> 'image/avif'
BEGIN SELECT RAISE(ABORT,'cantos:policy.avif_only'); END;
CREATE TRIGGER media_artwork_changes_avif_only BEFORE INSERT ON media_work_artwork_changes
WHEN NEW.image_asset_id IS NOT NULL AND EXISTS(
 SELECT 1 FROM media_image_assets a WHERE a.id=NEW.image_asset_id AND a.content_type <> 'image/avif')
BEGIN SELECT RAISE(ABORT,'cantos:policy.avif_only'); END;
