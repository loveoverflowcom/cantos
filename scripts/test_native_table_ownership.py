"""Negative source controls for the #332 table ownership inventory (no database)."""

import copy
import json
import tempfile
import unittest
from pathlib import Path

import native_table_ownership as ownership


class NativeTableOwnershipTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.sql = self.root / ownership.SCHEMA / "0001_current.sql"
        self.sql.parent.mkdir(parents=True)
        self.sql.write_text(
            "CREATE TABLE public.actors (id text);\n"
            "CREATE TABLE public.bearer_tokens (token_hash text);\n"
            "CREATE TABLE public.sessions (id text);\n", encoding="utf-8",
        )
        self.manifest = {
            "version": 1,
            "dispositions": {
                "server": ["actors"],
                "replace-provider-auth": ["bearer_tokens"],
                "development-only": ["sessions"],
                "replace-registry": ["cantos_migrations", "cantos_schema"],
            },
        }
        self.save()

    def save(self):
        (self.root / ownership.MANIFEST).write_text(json.dumps(self.manifest), encoding="utf-8")

    def errors(self):
        self.save()
        return ownership.repository_failures(self.root)

    def test_current_sources_cover_all_84_tables_without_database_access(self):
        root = Path(__file__).resolve().parents[1]
        self.assertEqual(ownership.repository_failures(root), [])
        self.assertEqual(len(ownership.source_tables(root)), 84)

    def test_disposable_source_fixture_is_valid(self):
        self.assertEqual(self.errors(), [])

    def test_new_migration_table_requires_explicit_disposition(self):
        (self.sql.parent / "0002_new.sql").write_text(
            "CREATE TABLE public.new_receipts (id text);", encoding="utf-8",
        )
        self.assertEqual(self.errors(), [
            "native table ownership: new_receipts: missing native disposition "
            "(services/cantos-server/schema/0002_new.sql)",
        ])

    def test_provider_secrets_cannot_be_classified_as_server_data(self):
        names = [
            "bearer_flows", "bearer_families", "bearer_tokens", "studio_oidc_flows",
            "studio_oidc_sessions", "theatre_identity_flows", "theatre_identity_sessions",
            "theatre_native_flows", "theatre_native_sessions", "theatre_registrations",
            "theatre_registration_mail_starts", "theatre_firebase_enrollments",
            "theatre_firebase_enrollment_receipts", "theatre_firebase_send_starts",
        ]
        self.sql.write_text("\n".join(f"CREATE TABLE public.{name} (id text);" for name in names),
                            encoding="utf-8")
        groups = self.manifest["dispositions"]
        groups["server"] = sorted(names)
        groups["replace-provider-auth"] = []
        groups["development-only"] = []
        errors = self.errors()
        self.assertEqual(len(errors), 14)
        self.assertTrue(all("expected replace-provider-auth, found server" in e for e in errors))

    def test_development_sessions_and_pg_registry_cannot_enter_release_data(self):
        for table, group in [("sessions", "development-only"), ("cantos_schema", "replace-registry")]:
            manifest = copy.deepcopy(self.manifest)
            manifest["dispositions"][group].remove(table)
            manifest["dispositions"]["server"].append(table)
            manifest["dispositions"]["server"].sort()
            self.assertEqual(ownership.validate(manifest, ownership.source_tables(self.root)), [
                f"{table}: expected {group}, found server",
            ])

    def test_duplicate_and_stale_classifications_are_refused(self):
        self.manifest["dispositions"]["server"] = ["actors", "actors", "removed_table"]
        self.assertEqual(self.errors(), [
            "native table ownership: actors: classified 2 times",
            "native table ownership: removed_table: no current source declaration",
        ])

    def test_unrecognized_ddl_fails_instead_of_omitting_a_table(self):
        for declaration in [
            "CREATE TABLE public.\"quoted\" (id text);",
            "CREATE TABLE IF NOT EXISTS public.optional (id text);",
            "CREATE UNLOGGED TABLE public.transient (id text);",
            "CREATE GLOBAL TEMPORARY TABLE public.unmapped_secret (id text);",
        ]:
            self.sql.write_text(declaration, encoding="utf-8")
            self.assertEqual(self.errors(), [
                "native table ownership: services/cantos-server/schema/0001_current.sql: "
                "unsupported CREATE TABLE declaration",
            ])

    def test_duplicate_ddl_is_refused_even_if_inventory_would_hide_it(self):
        self.sql.write_text("CREATE TABLE public.actors (id text);\n" * 2, encoding="utf-8")
        self.assertEqual(self.errors(), ["native table ownership: duplicate source table: actors"])

    def test_unquoted_identifier_case_cannot_bypass_ownership_policy(self):
        for table, required in [("bearer_secrets", "replace-provider-auth"),
                                ("sessions", "development-only"),
                                ("cantos_schema", "replace-registry")]:
            with self.subTest(table=table):
                original = self.sql.read_text(encoding="utf-8")
                if table == "bearer_secrets":
                    self.sql.write_text(original + "CREATE TABLE public.BEARER_SECRETS (id text);",
                                        encoding="utf-8")
                else:
                    saved = self.sql.read_text(encoding="utf-8")
                    self.sql.write_text(saved.replace(f"public.{table}", f"public.{table.upper()}"),
                                        encoding="utf-8")
                manifest = copy.deepcopy(self.manifest)
                if table != "bearer_secrets":
                    manifest["dispositions"][required].remove(table)
                manifest["dispositions"]["server"] = sorted(["actors", table.upper()])
                self.assertEqual(ownership.validate(manifest, ownership.source_tables(self.root)), [
                    f"{table.upper()}: expected canonical lowercase unquoted table name",
                    f"{table}: expected {required}, found server",
                ])
                self.sql.write_text(original, encoding="utf-8")

    def test_unquoted_identifier_case_cannot_hide_duplicate_source_tables(self):
        with self.sql.open("a", encoding="utf-8") as file:
            file.write("CREATE TABLE public.ACTORS (id text);")
        self.assertEqual(self.errors(), ["native table ownership: duplicate source table: actors"])

    def test_manifest_case_alias_is_refused_and_counted_as_a_duplicate(self):
        self.manifest["dispositions"]["server"] = ["ACTORS", "actors"]
        self.assertEqual(self.errors(), [
            "native table ownership: ACTORS: expected canonical lowercase unquoted table name",
            "native table ownership: actors: classified 2 times",
        ])

    def test_retired_registry_tables_need_no_source_but_must_stay_classified(self):
        # The fixture has no registry source at all, exactly like the current tree.
        self.assertFalse((self.root / "services/cantos-server/src/platform/postgres").exists())
        self.assertEqual(self.errors(), [])
        for table in sorted(ownership.RETIRED_REGISTRY_TABLES):
            with self.subTest(table=table):
                manifest = copy.deepcopy(self.manifest)
                manifest["dispositions"]["replace-registry"] = []
                self.assertEqual(ownership.validate(manifest, ownership.source_tables(self.root)), [
                    f"{name}: retired migrator table must stay classified replace-registry"
                    for name in sorted(ownership.RETIRED_REGISTRY_TABLES)
                ])

    def test_only_the_named_retired_tables_may_lack_a_source_declaration(self):
        self.manifest["dispositions"]["server"] = ["actors", "cantos_extra"]
        self.assertEqual(self.errors(), [
            "native table ownership: cantos_extra: no current source declaration",
        ])

    def test_comments_are_not_table_declarations(self):
        with self.sql.open("a", encoding="utf-8") as file:
            file.write("\n-- CREATE TABLE ignored (id text);\n/* /* nested */ CREATE TABLE ignored2 (id int); */")
        self.assertEqual(self.errors(), [])

    def test_comment_markers_in_strings_do_not_hide_later_ddl(self):
        for value in ["'https://example.test'", "'-- not a comment'", "'/* not a comment */'",
                      "'escaped '' quote -- literal'", "$$ -- dollar literal $$"]:
            with self.subTest(value=value):
                original = self.sql.read_text(encoding="utf-8")
                self.sql.write_text(original + f"\nSELECT {value}; CREATE TABLE public.unmapped_secret (id text);",
                                    encoding="utf-8")
                self.assertEqual(self.errors(), [
                    "native table ownership: unmapped_secret: missing native disposition "
                    "(services/cantos-server/schema/0001_current.sql)",
                ])
                self.sql.write_text(original, encoding="utf-8")

    def test_incomplete_sql_lexemes_are_refused(self):
        for text in ["/* open", "SELECT 'open", 'SELECT "open', "SELECT $tag$open"]:
            with self.assertRaises(ValueError):
                ownership.without_sql_comments(text)

    def test_escape_string_quote_cannot_hide_later_ddl(self):
        with self.sql.open("a", encoding="utf-8") as file:
            file.write("\nSELECT E'escaped " + chr(92) + "' -- literal'; "
                       "CREATE TABLE public.unmapped_secret (id text);")
        self.assertEqual(self.errors(), [
            "native table ownership: unsupported backslash-escaped SQL quote",
        ])

    def test_missing_sources_or_manifest_and_invalid_shapes_fail(self):
        self.sql.unlink()
        self.assertEqual(self.errors(), ["native table ownership: no current schema SQL files"])
        for value in [None, {}, {"version": True, "dispositions": {}},
                      {"version": 1, "dispositions": []}]:
            with self.assertRaises(ValueError):
                ownership.validate(value, {})
        (self.root / ownership.MANIFEST).unlink()
        self.assertIn("No such file", ownership.repository_failures(self.root)[0])

    def test_duplicate_manifest_keys_are_refused(self):
        (self.root / ownership.MANIFEST).write_text('{"version":1,"version":1}', encoding="utf-8")
        self.assertEqual(ownership.repository_failures(self.root), [
            "native table ownership: duplicate JSON key: version",
        ])


if __name__ == "__main__":
    unittest.main()
