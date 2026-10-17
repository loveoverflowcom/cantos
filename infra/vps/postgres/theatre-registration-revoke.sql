-- Counterpart of theatre-registration-grants.sql, applied as `cantos_owner` while Theatre
-- registration is not enabled, so disabling it also withdraws the privileges. Idempotent.
REVOKE ALL ON theatre_firebase_enrollments, theatre_firebase_send_starts,
    theatre_firebase_enrollment_receipts FROM cantos_app;
REVOKE ALL ON SEQUENCE theatre_firebase_send_starts_receipt_id_seq,
    theatre_firebase_enrollment_receipts_receipt_id_seq FROM cantos_app;
