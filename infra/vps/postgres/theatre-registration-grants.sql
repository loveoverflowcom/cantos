-- Optional runtime grants for Theatre registration in Firebase mode (migration 0028), applied
-- by `cantos-vps deploy` as `cantos_owner` ONLY when the Theatre API configuration contains
-- CANTOS_THEATRE_REGISTRATION_ENABLED=true; otherwise theatre-registration-revoke.sql applies.
-- Every statement is idempotent. Source of truth for the set: the repository's SQL
-- (firebase_enrollment/postgres.rs), proven by its PostgreSQL tests which run against THIS file.
--
-- Deliberately absent: DELETE/TRUNCATE (history is retained), UPDATE of the binding columns
-- (namespace, id, project, uid, email, accepted_at, expires_at), any access to the
-- transition-receipt table (the SECURITY DEFINER trigger writes it), the SMTP-mode tables
-- (theatre_registrations*), DDL and the guard/receipt trigger functions.
-- The grants confer capability to run the flow only; they do not accept activation.

GRANT SELECT, INSERT ON theatre_firebase_enrollments, theatre_firebase_send_starts TO cantos_app;
GRANT UPDATE (phase, last_sent_at, person_id, block_reason)
    ON theatre_firebase_enrollments TO cantos_app;
GRANT USAGE ON SEQUENCE theatre_firebase_send_starts_receipt_id_seq TO cantos_app;
