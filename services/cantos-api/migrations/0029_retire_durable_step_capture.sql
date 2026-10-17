-- ADR 0018 retired the backend synthesis runtime and its JobSummary consumer.
-- Stop maintenance of that unused projection without changing historical migrations,
-- captured rows, legacy source rows, privileges or their integrity triggers.
--
-- The retained outbox is historical after this boundary, not a complete change feed.
-- Re-enabling this trigger alone cannot recover changes made while capture was disabled;
-- any future projection needs its own reviewed adapter and a new authoritative snapshot.
ALTER TABLE public.durable_steps DISABLE TRIGGER durable_steps_change_capture;
