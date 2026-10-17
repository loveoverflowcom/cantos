-- Cantos production roles and database. Idempotent; run by `cantos-vps bootstrap` as the
-- container's `postgres` superuser over its Unix socket. Passwords are set separately from
-- root-only secret files through stdin, never from this tracked file or a command line.
--
--   cantos_owner  owns the `cantos` database and every migrated object; runs migrations only.
--   cantos_app    the API runtime login: no ownership, no DDL, only runtime-grants.sql.
SELECT 'CREATE ROLE cantos_owner LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION'
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'cantos_owner')\gexec
SELECT 'CREATE ROLE cantos_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOINHERIT'
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'cantos_app')\gexec
SELECT 'CREATE DATABASE cantos OWNER cantos_owner ENCODING ''UTF8'' TEMPLATE template0'
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'cantos')\gexec
REVOKE ALL ON DATABASE cantos FROM PUBLIC;
GRANT CONNECT, TEMPORARY ON DATABASE cantos TO cantos_owner;
GRANT CONNECT ON DATABASE cantos TO cantos_app;
