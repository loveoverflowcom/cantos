#!/usr/bin/env python3
"""Tests for skill structure and API architecture checks in check_repository.py.

Run from the repository root:

    python3 -m unittest discover -s scripts -p 'test_*.py'
"""

import os
import tempfile
import unittest
from pathlib import Path

import check_repository as checks


VALID_SKILL = """---
name: cantos-sample
description: >-
  Sample skill used by the checker tests.
  Use when exercising validation.
---

# Sample

See [the technique](references/technique.md).
"""

VALID_OPENAI = """interface:
  display_name: "Cantos Sample"
  short_description: "A sample skill for tests"
  default_prompt: "Use $cantos-sample to exercise the checker."
"""


class SkillTree:
    """A disposable repository root holding one valid skill, its index and its Claude link."""

    def __init__(self, root):
        self.root = Path(root)
        self.skill = self.root / ".agents/skills/cantos-sample"
        (self.skill / "agents").mkdir(parents=True)
        (self.skill / "references").mkdir()
        self.write("SKILL.md", VALID_SKILL)
        self.write("agents/openai.yaml", VALID_OPENAI)
        self.write("references/technique.md", "# Technique\n")
        index = self.root / ".agents/skills/README.md"
        index.write_text("- [`cantos-sample`](cantos-sample/SKILL.md)\n", encoding="utf-8")
        claude = self.root / ".claude/skills"
        claude.mkdir(parents=True)
        os.symlink("../../.agents/skills/cantos-sample", claude / "cantos-sample")

    def write(self, relative, content):
        path = self.skill / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def errors(self):
        return checks.check_skills(self.root)


class ParseFrontmatterTest(unittest.TestCase):
    def test_folded_block_joins_lines_with_spaces(self):
        fields, errors = checks.parse_frontmatter(VALID_SKILL)
        self.assertEqual(errors, [])
        self.assertEqual(fields["name"], "cantos-sample")
        self.assertEqual(
            fields["description"],
            "Sample skill used by the checker tests. Use when exercising validation.",
        )

    def test_quoted_scalar_is_unquoted(self):
        fields, errors = checks.parse_frontmatter('---\nname: "cantos-a"\ndescription: x\n---\n')
        self.assertEqual(errors, [])
        self.assertEqual(fields["name"], "cantos-a")

    def test_missing_opening_delimiter_is_rejected(self):
        _, errors = checks.parse_frontmatter("# No frontmatter\n")
        self.assertEqual(errors, ["frontmatter must start on line 1 with ---"])

    def test_unclosed_frontmatter_is_rejected(self):
        _, errors = checks.parse_frontmatter("---\nname: cantos-a\n")
        self.assertEqual(errors, ["frontmatter is not closed with ---"])

    def test_duplicate_key_is_rejected(self):
        _, errors = checks.parse_frontmatter("---\nname: a\nname: b\n---\n")
        self.assertEqual(errors, ["duplicate frontmatter key: name"])

    def test_nested_mapping_is_outside_the_portable_subset(self):
        _, errors = checks.parse_frontmatter("---\nmetadata:\n  owner: x\n---\n")
        self.assertEqual(len(errors), 1)
        self.assertIn("unsupported frontmatter line 3", errors[0])


class CheckSkillsTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.tree = SkillTree(self.directory.name)

    def tearDown(self):
        self.directory.cleanup()

    def assertSingleError(self, fragment):
        errors = self.tree.errors()
        self.assertEqual(len(errors), 1, errors)
        self.assertIn(fragment, errors[0])

    def test_valid_tree_has_no_errors(self):
        self.assertEqual(self.tree.errors(), [])

    def test_name_must_match_directory(self):
        self.tree.write("SKILL.md", VALID_SKILL.replace("name: cantos-sample", "name: cantos-other"))
        self.assertSingleError("must equal directory name 'cantos-sample'")

    def test_non_portable_frontmatter_key_is_rejected(self):
        self.tree.write("SKILL.md", VALID_SKILL.replace("---\n\n", "allowed-tools: Bash\n---\n\n"))
        self.assertSingleError("non-portable frontmatter keys: allowed-tools")

    def test_description_over_limit_is_rejected(self):
        long_description = "x" * (checks.SKILL_DESCRIPTION_MAX + 1)
        self.tree.write(
            "SKILL.md",
            f"---\nname: cantos-sample\ndescription: {long_description}\n---\n"
            "[t](references/technique.md)\n",
        )
        self.assertSingleError(f"limit is {checks.SKILL_DESCRIPTION_MAX}")

    def test_skill_file_over_line_limit_is_rejected(self):
        padding = "line\n" * checks.SKILL_FILE_MAX_LINES
        self.tree.write("SKILL.md", VALID_SKILL + padding)
        self.assertSingleError("move depth into references/")

    def test_missing_codex_metadata_is_rejected(self):
        (self.tree.skill / "agents/openai.yaml").unlink()
        self.assertSingleError("missing agents/openai.yaml")

    def test_default_prompt_must_invoke_the_skill(self):
        self.tree.write("agents/openai.yaml", VALID_OPENAI.replace("$cantos-sample", "the skill"))
        self.assertSingleError("default_prompt must invoke $cantos-sample")

    def test_unreferenced_reference_is_rejected(self):
        self.tree.write("references/orphan.md", "# Orphan\n")
        self.assertSingleError("references/orphan.md is not linked")

    def test_reference_reached_through_another_reference_is_accepted(self):
        self.tree.write("references/technique.md", "# Technique\n\n[Deeper](deeper.md)\n")
        self.tree.write("references/deeper.md", "# Deeper\n")
        self.assertEqual(self.tree.errors(), [])

    def test_link_inside_code_fence_does_not_count(self):
        self.tree.write("SKILL.md", VALID_SKILL.replace(
            "See [the technique](references/technique.md).",
            "```text\n[the technique](references/technique.md)\n```",
        ))
        self.assertSingleError("references/technique.md is not linked")

    def test_unexpected_entry_is_rejected(self):
        self.tree.write("notes.txt", "scratch\n")
        self.assertSingleError("unexpected entry 'notes.txt'")

    def test_skill_missing_from_index_is_rejected(self):
        (self.tree.root / ".agents/skills/README.md").write_text("# Index\n", encoding="utf-8")
        self.assertSingleError("does not link cantos-sample/SKILL.md")

    def test_missing_claude_symlink_is_rejected(self):
        (self.tree.root / ".claude/skills/cantos-sample").unlink()
        self.assertSingleError("missing symlink to ../../.agents/skills/cantos-sample")

    def test_claude_symlink_with_wrong_target_is_rejected(self):
        link = self.tree.root / ".claude/skills/cantos-sample"
        link.unlink()
        os.symlink("../../elsewhere/cantos-sample", link)
        self.assertSingleError("points to '../../elsewhere/cantos-sample'")

    def test_stale_claude_symlink_is_rejected(self):
        os.symlink("../../.agents/skills/cantos-gone", self.tree.root / ".claude/skills/cantos-gone")
        self.assertSingleError("cantos-gone: no matching skill")

    def test_symlinked_claude_skills_directory_is_rejected(self):
        claude = self.tree.root / ".claude/skills"
        (claude / "cantos-sample").unlink()
        claude.rmdir()
        os.symlink("../.agents/skills", claude)
        self.assertSingleError("must be a real directory of per-skill symlinks")


class RepositorySkillsTest(unittest.TestCase):
    def test_repository_skills_pass(self):
        self.assertEqual(checks.check_skills(checks.ROOT), [])


class ApiArchitectureTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)

    def tearDown(self):
        self.directory.cleanup()

    def errors(self, path, content):
        file = self.root / checks.API_SOURCE / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(content, encoding="utf-8")
        return checks.check_api_architecture(self.root)

    def assertRejected(self, path, content, reason):
        errors = self.errors(path, content)
        self.assertTrue(any(reason in error for error in errors), errors)

    def test_domain_accepts_pure_collections_and_in_memory_byte_readers(self):
        self.assertEqual(self.errors("contexts/ingestion/domain/extraction/docx.rs", """
use std::{collections::BTreeSet, io::{Cursor, Read}};
use crate::{script_ir::read_script, contexts::editorial::domain::EvidenceKind};
"""), [])

    def test_identity_domain_is_pure_and_registry_table_is_platform_owned(self):
        self.assertEqual(self.errors("platform/identity/domain.rs", """
use serde::{Deserialize, Serialize};
use std::fmt;
"""), [])
        self.assertRejected("platform/identity/domain.rs", "use tokio_postgres::Client;",
                            "pure domain must not depend on I/O")
        self.assertRejected("platform/identity/domain.rs", "use super::postgres::Registry;",
                            "pure domain may use only")
        self.assertRejected("contexts/catalog/postgres.rs",
                            'fn query() { "SELECT * FROM actor_external_identities"; }',
                            "SQL table actor_external_identities belongs to platform")

    def test_flat_domain_modules_cannot_bypass_purity_guards(self):
        for path, content, reason in (
            ("contexts/editorial/domain.rs", "use std::fs::File;",
             "pure domain must not depend on I/O"),
            ("script_ir.rs", "use crate::contexts::production::domain::Frozen;",
             "Script IR shared kernel must not depend"),
        ):
            with self.subTest(path=path):
                errors = self.errors(path, content)
                prefix = f"{checks.API_SOURCE / path}:"
                self.assertTrue(any(error.startswith(prefix) and reason in error
                                    for error in errors), errors)

    def test_flat_domain_modules_accept_pure_dependencies(self):
        for path in ("contexts/editorial/domain.rs", "script_ir.rs"):
            with self.subTest(path=path):
                self.assertEqual(self.errors(path, "use std::collections::BTreeSet;"), [])

    def test_nested_import_cannot_hide_filesystem_dependency(self):
        self.assertRejected("contexts/editorial/domain/revisions.rs", """
use std::{collections::BTreeSet, fs::{self as disk, File}};
""", "pure domain must not depend on I/O")

    def test_module_alias_cannot_hide_filesystem_or_downstream_dependency(self):
        self.assertRejected("contexts/editorial/domain/revisions.rs", """
use std as library;
fn f() { library::fs::read(path); }
""", "pure domain must not depend on I/O")
        self.assertRejected("contexts/editorial/application.rs", """
use crate::contexts as capabilities;
fn f() { capabilities::production::current_snapshot(); }
""", "editorial must not depend on production")

    def test_console_macros_and_clock_aliases_are_effects(self):
        self.assertRejected("contexts/editorial/domain/revisions.rs", """
use std::time as clock;
fn f() { println!("side effect"); clock::SystemTime::now(); }
""", "pure domain must not write to the console")

    def test_fully_qualified_provider_and_runtime_calls_are_rejected(self):
        self.assertRejected("contexts/production/domain/frozen.rs",
                            "fn f() { reqwest::get(url); }", "pure domain must not depend on I/O")

    def test_grouped_import_of_adapter_cannot_bypass_pure_domain_boundary(self):
        self.assertRejected("contexts/production/domain/frozen.rs", """
use crate::{contexts::production::{domain::Frozen, postgres::Snapshot}, script_ir::read_script};
""", "without an adapter or facade")

    def test_relative_adapter_import_cannot_bypass_pure_domain_boundary(self):
        self.assertRejected("contexts/production/domain/frozen.rs",
                            "use super::super::postgres::Snapshot;", "without an adapter or facade")

    def test_domain_must_use_pure_context_path_instead_of_facade(self):
        self.assertRejected("contexts/production/domain/frozen.rs",
                            "use crate::contexts::editorial::authorize;", "without an adapter or facade")

    def test_canonical_identifier_validation_is_pure_but_random_generation_is_not(self):
        self.assertEqual(self.errors("contexts/ingestion/domain/receipt.rs",
                                     "use crate::platform::ids::canonical_uuid;"), [])
        self.assertRejected("platform/ids.rs", "fn id() { uuid::Uuid::new_v4(); }",
                            "pure domain must not depend on I/O")

    def test_legacy_crate_alias_cannot_hide_context_direction(self):
        for alias in ("production", "clip_reuse", "tts", "postgres"):
            with self.subTest(alias=alias):
                self.assertRejected("contexts/editorial/application.rs",
                                    "use crate::{" + alias + " as planning};", "legacy crate alias")

    def test_local_postgres_import_is_not_a_legacy_crate_alias(self):
        self.assertEqual(self.errors("contexts/editorial/application.rs", """
use super::postgres;
fn f() { postgres::load_revision(); }
"""), [])

    def test_context_must_not_import_downstream_context(self):
        self.assertRejected("contexts/editorial/application.rs", """
use crate::contexts::{production::current_snapshot};
""", "editorial must not depend on production")

    def test_application_uses_upstream_entrypoint_without_private_adapter(self):
        self.assertEqual(self.errors("contexts/production/application/freeze.rs",
                                     "use crate::contexts::editorial::load_revision;"), [])
        self.assertRejected("contexts/production/application/freeze.rs",
                            "use crate::contexts::editorial::postgres::load_revision;",
                            "owning context's entrypoint")

    def test_media_lifecycle_consumes_owning_content_brief_entrypoints_only(self):
        for source in ["use crate::contexts::editorial::{authorize, find_revision};",
                       "use crate::contexts::production::audio_brief;",
                       "use crate::contexts::catalog::ProductionContent;"]:
            self.assertEqual(self.errors("contexts/media/application.rs", source), [])
        self.assertRejected("contexts/media/application.rs",
                            "use crate::contexts::production::postgres::briefs;",
                            "owning context's entrypoint")
    def test_production_has_only_the_concrete_reverse_credential_inquiry(self):
        self.assertEqual(self.errors("contexts/production/application/briefs.rs",
                                    "use crate::contexts::media::upload_token_registered;"), [])
        self.assertRejected("contexts/production/application/briefs.rs",
                            "use crate::contexts::media::audio_intake;",
                            "production must not depend on media")
        self.assertRejected("contexts/production/domain/brief.rs",
                            "use crate::contexts::media::upload_token_registered;",
                            "production must not depend on media")

    def test_media_intake_owns_only_its_postgres_tables(self):
        self.assertEqual(self.errors("contexts/media/postgres.rs", '''
const QUERY: &str = "SELECT id FROM audio_upload_intents JOIN audio_intake_receipts USING(id)";
'''), [])
        self.assertRejected("contexts/production/postgres.rs",
                            'const QUERY: &str = "SELECT id FROM audio_intake_receipts";',
                            "SQL table audio_intake_receipts belongs to media")
        self.assertRejected("contexts/media/application.rs",
                            'let sql = "INSERT INTO audio_upload_intents VALUES ($1)";',
                            "SQL table access belongs in the PostgreSQL adapter")

    def test_new_context_requires_explicit_dependency_policy(self):
        self.assertRejected("contexts/publishing/mod.rs", "mod domain;",
                            "new context needs an explicit dependency policy")

    def test_catalog_application_uses_editorial_and_ingestion_entrypoints(self):
        self.assertEqual(self.errors("contexts/catalog/application.rs", """
use crate::contexts::{editorial::find_revision, ingestion::imported_source};
"""), [])

    def test_catalog_cannot_import_private_upstream_adapter(self):
        self.assertRejected("contexts/catalog/application.rs",
                            "use crate::contexts::ingestion::postgres::imported_source;",
                            "owning context's entrypoint")

    def test_catalog_is_downstream_of_editorial_and_separate_from_production(self):
        self.assertRejected("contexts/editorial/application.rs",
                            "use crate::contexts::catalog::domain::Work;",
                            "editorial must not depend on catalog")
        self.assertRejected("contexts/catalog/application.rs",
                            "use crate::contexts::production::production_catalog;",
                            "catalog must not depend on production")

    def test_catalog_domain_rejects_database_and_provider_dependencies(self):
        for dependency in ("tokio_postgres::Row", "reqwest::Client"):
            with self.subTest(dependency=dependency):
                self.assertRejected("contexts/catalog/domain/mod.rs",
                                    f"use {dependency};",
                                    "pure domain must not depend on I/O")

    def test_catalog_tables_belong_only_to_catalog_adapter(self):
        self.assertEqual(self.errors("contexts/catalog/postgres.rs", """
const QUERY: &str = "SELECT id FROM catalog_works";
"""), [])
        self.assertRejected("contexts/editorial/postgres.rs",
                            'const QUERY: &str = "SELECT id FROM catalog_works";',
                            "SQL table catalog_works belongs to catalog")

    def test_catalog_adapter_must_use_legacy_context_entrypoints(self):
        self.assertRejected("contexts/catalog/postgres.rs",
                            'const QUERY: &str = "SELECT id FROM source_records";',
                            "SQL table source_records belongs to ingestion")

    def test_catalog_application_and_http_cannot_execute_catalog_sql(self):
        for path in ("contexts/catalog/application.rs", "interfaces/http/studio/catalog.rs"):
            with self.subTest(path=path):
                self.assertRejected(path,
                                    'const QUERY: &str = "UPDATE catalog_works SET head_revision=$1";',
                                    "SQL table access belongs in the PostgreSQL adapter")

    def test_context_adapter_and_use_case_modules_remain_private(self):
        self.assertRejected("contexts/production/mod.rs", "pub(crate) mod postgres;",
                            "postgres must remain private")

    def test_shared_kernel_cannot_depend_on_context(self):
        self.assertRejected("script_ir/validation.rs",
                            "use crate::contexts::production::domain::Frozen;",
                            "Script IR shared kernel must not depend")

    def test_http_allows_wire_admission_and_delegates_business_use_case(self):
        self.assertEqual(self.errors("interfaces/http/studio/mod.rs", """
use crate::platform::postgres::{Store, StoreError};
use crate::contexts::editorial::domain::diagnostics::validation_issues;
fn validate() { crate::script_ir::read_script(bytes); state.store.save(request); }
"""), [])

    def test_catalog_http_allows_pure_wire_admission_and_typed_error_mapping(self):
        self.assertEqual(self.errors("interfaces/http/studio/catalog.rs", """
use crate::contexts::catalog::{wire as input, CatalogCommand, CatalogCursor, CatalogStoreError};
use crate::contexts::catalog::domain::CatalogError;
fn f() { input::create_work(request); CatalogCursor::decode(cursor); }
fn error(e: CatalogStoreError) { match e { CatalogStoreError::Domain(CatalogError::WorkArchived) => {}, _ => {} } }
"""), [])

    def test_catalog_http_cannot_call_domain_transition_or_command_constructor(self):
        for symbol in ("domain::Work::new", "domain::Work::archive", "CatalogCommand::ArchiveWork"):
            with self.subTest(symbol=symbol):
                self.assertRejected("interfaces/http/studio/catalog.rs",
                    f"fn f() {{ crate::contexts::catalog::{symbol}(input); }}",
                    "HTTP must delegate business decisions")

    def test_http_cannot_import_business_policy(self):
        self.assertRejected("interfaces/http/studio/production.rs", """
use crate::contexts::production::{domain::assess_eligibility as assess};
""", "HTTP must delegate business decisions")

    def test_http_cannot_reach_transaction_or_sql_client(self):
        self.assertRejected("interfaces/http/studio/production.rs", """
use crate::platform::postgres::{Store, Tx};
fn f() { state.store.connection(); client.transaction(); tx.query_one(sql, args); }
""", "HTTP must not execute SQL or open transactions")

    def test_context_sql_cannot_read_or_write_another_contexts_table(self):
        self.assertRejected("contexts/ingestion/postgres.rs", """
const QUERY: &str = r#"SELECT document_bytes FROM public.production_snapshots WHERE id=$1"#;
""", "SQL table production_snapshots belongs to production")

    def test_application_cannot_bypass_sql_adapter_even_for_its_own_table(self):
        self.assertRejected("contexts/production/application/freeze.rs",
                            'let sql = "UPDATE production_snapshots SET id=$1";',
                            "SQL table access belongs in the PostgreSQL adapter")

    def test_platform_cannot_depend_on_business_contexts(self):
        self.assertRejected("platform/postgres/mod.rs", """
type View = crate::contexts::synthesis::summary_view::AttachedView;
""", "platform must not depend on a business context")
        self.assertRejected("platform/postgres/session.rs", """
use crate::contexts::production::current_snapshot;
""", "platform must not depend on a business context")

    def test_legacy_link_exception_is_read_only_and_path_specific(self):
        self.assertEqual(self.errors("contexts/ingestion/postgres.rs", """
const QUERY: &str = "SELECT id FROM source_records WHERE EXISTS(SELECT 1 FROM revision_evidence)";
"""), [])
        self.assertRejected("contexts/ingestion/postgres.rs",
                            'const QUERY: &str = "INSERT INTO revision_evidence VALUES ($1)";',
                            "SQL table revision_evidence belongs to editorial")

    def test_inline_test_module_super_resolves_to_its_parent_module(self):
        self.assertEqual(self.errors("platform/ids.rs", """
#[cfg(test)]
mod tests { use super::canonical_uuid; }
"""), [])

    def test_nested_comments_strings_characters_and_lifetimes_do_not_create_dependencies(self):
        self.assertEqual(self.errors("contexts/production/domain/frozen.rs", """
// use axum::Router;
/* std::fs::read(path); /* use tokio::fs; */ */
const DOC: &str = r##"A \"quote\" and reqwest::get are examples"##;
fn f<'a>(value: &'a str) { let quote = '\"'; let letter = 'a'; }
"""), [])


if __name__ == "__main__":
    unittest.main()
