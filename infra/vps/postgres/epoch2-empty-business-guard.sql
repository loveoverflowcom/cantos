-- One deliberately narrow import: the observed 28-migration installation with no business
-- or session rows. Exact schema and registry comparison happen before this data guard.
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout = '60s';
SET LOCAL lock_timeout = '2s';
DO $$
DECLARE relation record; actual bigint; expected bigint;
BEGIN
    IF EXISTS (SELECT FROM pg_prepared_xacts WHERE database = current_database())
       OR EXISTS (SELECT FROM pg_subscription)
       OR EXISTS (SELECT FROM pg_replication_slots WHERE database = current_database()) THEN
        RAISE EXCEPTION 'epoch2 cutover refuses prepared transactions or replication';
    END IF;
    IF EXISTS (SELECT FROM pg_extension WHERE extname <> 'plpgsql') THEN
        RAISE EXCEPTION 'epoch2 cutover refuses unknown extensions';
    END IF;
    IF EXISTS (
        SELECT FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'
          AND pg_get_userbyid(c.relowner) <> 'cantos_owner'
    ) OR EXISTS (
        SELECT FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
        WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'
          AND pg_get_userbyid(p.proowner) <> 'cantos_owner'
    ) THEN
        RAISE EXCEPTION 'epoch2 cutover refuses unexpected object ownership';
    END IF;
    FOR relation IN
        SELECT n.nspname, c.relname FROM pg_class c
        JOIN pg_namespace n ON n.oid=c.relnamespace
        WHERE c.relkind IN ('r','p') AND n.nspname !~ '^pg_'
          AND n.nspname <> 'information_schema' AND NOT c.relispartition
        ORDER BY n.nspname,c.relname
    LOOP
        expected := CASE relation.nspname || '.' || relation.relname
            WHEN 'public.cantos_migrations' THEN 28
            WHEN 'public.catalog_clock' THEN 1
            WHEN 'public.studio_oidc_flows' THEN 2
            WHEN 'public.theatre_identity_flows' THEN 3
            ELSE 0 END;
        EXECUTE format('SELECT count(*) FROM %I.%I', relation.nspname,relation.relname) INTO actual;
        IF actual <> expected THEN
            RAISE EXCEPTION 'epoch2 cutover refuses changed row count in %.%', relation.nspname,relation.relname;
        END IF;
    END LOOP;
END $$;
COMMIT;
