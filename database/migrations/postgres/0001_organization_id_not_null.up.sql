-- sdkwork:migration
-- id: 0001_organization_id_not_null
-- engine: postgres
-- module: sdkwork-image
-- purpose: Enforce organization_id NOT NULL DEFAULT on all tables in the
--   consolidated baseline. NULL rows (pre-standard data anomalies) are
--   backfilled with the platform sentinel before NOT NULL is set, and
--   NOT NULL columns without an explicit default receive the sentinel
--   default, keeping existing deployments consistent with fresh baseline
--   installs.
-- reversible: false
-- rollback: forward-fix (sentinel backfill is the canonical fix; NULL
--   organization rows are data anomalies)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

UPDATE image_preset SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_preset ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_preset ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_generation_job SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_generation_job ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_generation_job ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_edit_task SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_edit_task ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_edit_task ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_asset SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_asset ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_asset ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_gallery SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_gallery ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_gallery ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_gallery_item SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_gallery_item ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_gallery_item ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_generation_output SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_generation_output ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_generation_output ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_provider_binding SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_provider_binding ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_provider_binding ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_provider_task SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_provider_task ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_provider_task ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_provider_webhook_event SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_provider_webhook_event ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_provider_webhook_event ALTER COLUMN organization_id SET NOT NULL;

UPDATE image_notification_outbox SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE image_notification_outbox ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE image_notification_outbox ALTER COLUMN organization_id SET NOT NULL;

COMMIT;
