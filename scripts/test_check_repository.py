#!/usr/bin/env python3
"""Tests for skill structure and API/Theatre architecture checks in check_repository.py.

Run from the repository root:

    python3 -m unittest discover -s scripts -p 'test_*.py'
"""

import os
import shutil
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

    def test_oidc_core_is_pure_and_session_facts_are_platform_owned(self):
        self.assertEqual(self.errors("platform/oidc/core.rs", "use crate::platform::identity::domain::ExternalIdentityKey;"), [])
        self.assertRejected("platform/oidc/core.rs", "use reqwest::Client;", "pure domain must not depend on I/O")
        self.assertRejected("platform/oidc/core.rs", "use super::provider::Provider;", "pure domain may use only")
        self.assertRejected("contexts/editorial/postgres/read.rs", 'const Q: &str = "SELECT * FROM studio_oidc_sessions";', "SQL table studio_oidc_sessions belongs to platform")
        self.assertRejected("contexts/editorial/postgres/read.rs", 'const Q: &str = "SELECT * FROM theatre_identity_sessions";', "SQL table theatre_identity_sessions belongs to platform")

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

    def test_admin_domain_is_pure_and_audit_tables_are_platform_owned(self):
        self.assertRejected("platform/admin/domain.rs", "use tokio_postgres::Client;",
                            "pure domain must not depend on I/O")
        self.assertRejected("platform/admin/domain.rs", "use super::provider::AdminProvider;",
                            "pure domain may use only")
        for table in ("admin_audit", "admin_operations", "admin_access_revocations"):
            self.assertRejected("contexts/catalog/postgres.rs",
                                f'fn query() {{ "SELECT * FROM {table}"; }}',
                                f"SQL table {table} belongs to platform")

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

    def test_domain_cannot_build_theatre_wire_dtos_in_any_import_form(self):
        # Persisted canonical documents stay admitted; the application layer owns projection.
        self.assertEqual(self.errors("contexts/media/domain/publication.rs",
                                     "use cantos_contracts as dto;"), [])
        self.assertEqual(self.errors("contexts/media/listener_projection.rs",
                                     "use cantos_contracts::theatre as dto;"), [])
        for content in (
            "use cantos_contracts::theatre as dto;",
            "use cantos_contracts::{theatre::WorkSummary, FieldIssue};",
            "fn f() -> cantos_contracts::theatre::PublicationManifest { todo!() }",
        ):
            with self.subTest(content=content):
                self.assertRejected("contexts/media/domain/listening.rs", content,
                                    "pure domain must not build Theatre wire DTOs")

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

    def test_media_http_allows_the_reviewed_private_read_facade(self):
        self.assertEqual(self.errors("interfaces/http/studio/audio_reads.rs", """
use crate::contexts::media::read::{
    AudioReadScope, ByteRange, NormalizedByteRange, RangeError, StorageCapability, StorageError, StorageStream,
};
fn ranges() { ByteRange::parse(raw); NormalizedByteRange::byte_length(range); }
fn errors(e: StorageError) { match e { StorageError::NotFound => {}, StorageError::UnsupportedCapability(StorageCapability::RangeRead) => {}, _ => {} } }
fn read() { AudioReadScope::ScriptIntent { script, intent }; state.store.authorize_audio_read(scope); }
"""), [])

    def test_media_http_facade_keeps_domain_adapters_and_unreviewed_types_private(self):
        for dependency in (
            "domain::object_storage::ByteRange",
            "domain::object_storage::StorageError",
            "domain::object_storage::StorageCapability",
            "read_application::AudioReadScope",
            "storage::port::StorageStream",
            "storage::s3::S3Storage",
            "read::StorageRegistry",
            "read::PrivateObjectRef",
        ):
            with self.subTest(dependency=dependency):
                self.assertRejected("interfaces/http/studio/audio_reads.rs",
                    f"use crate::contexts::media::{dependency};",
                    "HTTP must delegate business decisions through application use cases")

    def test_publication_http_allows_only_typed_errors_and_host_composition(self):
        self.assertEqual(self.errors("interfaces/http/studio/publication.rs", """
use crate::contexts::media::{PublicationService, PublicationDelivery, PublicationStoreError, PublicationDependencyError};
use crate::contexts::media::domain::publication::PublicationError;
fn f(store: Store) { PublicationService::new(store); }
fn error(e: PublicationStoreError) { match e { PublicationStoreError::Domain(PublicationError::InvalidTime) => {}, _ => {} } }
"""), [])

    def test_publication_http_cannot_evaluate_or_publish_domain_rules(self):
        for symbol in ("evaluate", "decide", "AudioRelease::publish", "retained_release_eligible", "DeliveryEvidence::new"):
            with self.subTest(symbol=symbol):
                self.assertRejected("interfaces/http/studio/publication.rs",
                    f"fn f() {{ crate::contexts::media::domain::publication::{symbol}(input); }}",
                    "HTTP must delegate business decisions")

    def test_artwork_http_allows_only_composition_receipts_and_typed_errors(self):
        self.assertEqual(self.errors("interfaces/http/studio/artwork.rs", """
use crate::contexts::media::{ArtworkService, ArtworkReceipt, ArtworkApplicationError, ImageObjectError};
use crate::contexts::media::domain::{image::ImageError, artwork::ArtworkError};
use crate::contexts::media::image_intake::ImageIntakeError;
fn f(store: Store) { ArtworkService::new(store); }
fn error(e: ArtworkApplicationError) { match e { ArtworkApplicationError::Image(ImageError::IntentExpired) => {}, _ => {} } }
"""), [])
        for symbol in (
            "domain::image::ImageAsset::create",
            "domain::image::decide_image_prepare",
            "domain::artwork::decide_artwork_change",
            "domain::artwork::SetWorkArtwork::admit",
            "image_intake::ImageAdmission::try_acquire",
            "postgres::image::insert_asset",
        ):
            with self.subTest(symbol=symbol):
                self.assertRejected("interfaces/http/studio/artwork.rs",
                    f"fn f() {{ crate::contexts::media::{symbol}(input); }}",
                    "HTTP must delegate business decisions")

    def test_platform_cannot_own_publication_delivery_port(self):
        for symbol in ("PublicationDelivery", "PublicationService"):
            with self.subTest(symbol=symbol):
                self.assertRejected("platform/postgres/mod.rs",
                    f"use crate::contexts::media::{symbol};",
                    "platform must not depend on a business context")

    def test_listener_http_admits_only_composition_errors_and_credentials(self):
        self.assertEqual(self.errors("interfaces/http/theatre_library.rs", """
use crate::contexts::listening::{LibraryService, LibraryError};
use crate::contexts::listening::auth::{ListenerAuthenticator, ListenerCredentials, AuthenticatedListener};
fn f() { crate::contexts::listening::LibraryService::new(store); crate::contexts::listening::auth::TheatreListenerAuthenticator::new(store); }
"""), [])
        for symbol in ("reconcile_progress", "projection::continue_records"):
            self.assertRejected("interfaces/http/theatre_library.rs",
                f"fn f() {{ crate::contexts::listening::domain::{symbol}(input); }}",
                "HTTP must delegate business decisions")
        self.assertRejected("interfaces/http/theatre_library.rs",
            "fn f() { crate::contexts::listening::auth::AuthenticatedListener::from_verified_actor(actor); }",
            "HTTP must delegate business decisions")

    def test_listener_dependency_direction_and_test_only_router_composition(self):
        self.assertEqual(self.errors("contexts/listening/postgres.rs", """
use crate::contexts::media::library_publications;
use crate::contexts::catalog::lock_listener_catalog;
"""), [])
        fixture="use crate::interfaces::http::{router_with_listener_identity,AppState};"
        self.assertEqual(self.errors("contexts/listening/postgres/tests.rs",fixture), [])
        self.assertRejected("contexts/listening/postgres.rs",fixture,"context must not depend on an interface")
        self.assertRejected("contexts/listening/postgres/tests.rs",
            "use crate::interfaces::http::theatre_library::routes;", "context must not depend on an interface")
        self.assertRejected("contexts/media/postgres.rs",
            "use crate::contexts::listening::LibraryService;", "media must not depend on listening")

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


class TheatreArchitectureTest(unittest.TestCase):
    PREFIX = "com/cantos/theatre"

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)

    def tearDown(self):
        self.directory.cleanup()

    def write(self, location, content, root=None):
        file = (root or self.root) / checks.THEATRE_SOURCE / location
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(content, encoding="utf-8")

    def errors(self, location, body, package=None, root=None):
        """Write one Kotlin file under <source set>/<layer path>; the package follows the path."""
        if root is None:
            shutil.rmtree(self.root / checks.THEATRE_SOURCE, ignore_errors=True)
        source_set, _, layer_path = location.partition("/")
        directory = layer_path.rsplit("/", 1)[0]
        package = package or f"com.cantos.theatre.{directory.replace('/', '.')}"
        self.write(f"{source_set}/kotlin/{self.PREFIX}/{layer_path}", f"package {package}\n\n{body}", root)
        return checks.check_theatre_architecture(root or self.root)

    def assertRejected(self, location, body, reason, package=None):
        errors = self.errors(location, body, package)
        label = f"{checks.THEATRE_SOURCE}/{location.partition('/')[0]}/kotlin/{self.PREFIX}/{location.partition('/')[2]}:"
        self.assertEqual(len(errors), 1, errors)
        self.assertTrue(errors[0].startswith(label) and reason in errors[0], errors)

    def test_domain_rejects_other_layers_and_platform_imports(self):
        for body, reason in (
            ("import com.cantos.theatre.application.playback.PlaybackController", "domain may import only"),
            ("import com.cantos.theatre.ports.MediaEngine", "domain may import only"),
            ("import com.cantos.theatre.adapters.mock.MockCatalog", "domain may import only"),
            ("import androidx.compose.runtime.Composable", "domain may import only"),
            ("import android.media.MediaPlayer", "domain may import only"),
            ("import java.time.Instant", "domain may import only"),
            ("import kotlinx.coroutines.flow.Flow", "domain may import only"),
            ("import com.cantos.ui.CantosColors", "domain may import only"),
        ):
            with self.subTest(body=body):
                self.assertRejected("commonMain/domain/playback/Reducer.kt", body, reason)

    def test_domain_rejects_ios_platform_import_in_ios_source_set(self):
        self.assertRejected("iosMain/domain/playback/Reducer.kt", "import platform.AVFoundation.AVPlayer",
                            "domain may import only")

    def test_import_forms_cannot_hide_a_forbidden_namespace(self):
        for body in (
            "import com.cantos.theatre.application.playback.PlaybackController as Controller",
            "import com.cantos.theatre.application.playback.*",
            "import com.cantos.theatre.`application`.playback.PlaybackController",
            "import com.cantos.theatre . application . playback . PlaybackController",
            "import com.cantos.theatre\n    .application\n    .playback.PlaybackController",
            "val unused = 1;import com.cantos.theatre.application.X",
        ):
            with self.subTest(body=body):
                self.assertRejected("commonMain/domain/playback/Reducer.kt", body, "domain may import only")

    def test_fully_qualified_names_need_no_import_to_be_caught(self):
        for body in (
            "@androidx.compose.runtime.Composable\nfun f() {}",
            "val player: android.media.MediaPlayer? = null",
            "val player = platform.AVFoundation.AVPlayer()",
            "val controller: com.cantos.theatre.application.playback.PlaybackController? = null",
            "val x = androidx . compose . runtime . Composable",
            "val x = `androidx`.compose.runtime.Composable",
            'val text = "${java.time.Instant.now()}"',
        ):
            with self.subTest(body=body):
                self.assertRejected("commonMain/domain/playback/Reducer.kt", body, "domain may import only")

    def test_ports_and_application_layers(self):
        self.assertRejected("commonMain/ports/MediaEngine.kt",
                            "import com.cantos.theatre.adapters.mock.MockCatalog", "ports may import only")
        self.assertRejected("commonMain/ports/MediaEngine.kt",
                            "import com.cantos.theatre.application.playback.PlaybackController",
                            "ports may import only")
        self.assertRejected("commonMain/ports/MediaEngine.kt", "import androidx.compose.runtime.Composable",
                            "ports may import only")
        self.assertRejected("commonMain/application/playback/Controller.kt",
                            "import com.cantos.theatre.adapters.mock.MockCatalog", "application may import only")
        self.assertRejected("commonMain/application/playback/Controller.kt",
                            "import com.cantos.theatre.features.shell.presentation.Root",
                            "application may import only")
        self.assertRejected("commonMain/application/playback/Controller.kt",
                            "import androidx.compose.runtime.mutableStateOf", "application may import only")

    def test_features_cannot_reach_app_adapters_ports_or_other_features(self):
        base = "commonMain/features/shell/presentation/Root.kt"
        for body, reason in (
            ("import com.cantos.theatre.app.CopyKey", "features must not depend on app"),
            ("import com.cantos.theatre.adapters.mock.MockCatalog", "features must not depend on adapters"),
            ("import com.cantos.theatre.ports.MediaEngine", "features must not depend on ports"),
            ("import com.cantos.theatre.features.catalog.presentation.Shelf",
             "features must not import another feature"),
            ("import android.content.Context", "must not use platform namespace android"),
            ("import platform.UIKit.UIView", "must not use platform namespace platform"),
            ("import java.util.UUID", "must not use platform namespace java"),
            ("import javax.crypto.Cipher", "must not use platform namespace javax"),
            ("val x: com.cantos.theatre.app.CopyKey? = null", "features must not depend on app"),
        ):
            with self.subTest(body=body):
                self.assertRejected(base, body, reason)

    def test_demo_is_a_leaf_package(self):
        self.assertRejected("commonMain/demo/DemoCopy.kt", "import com.cantos.theatre.app.Navigation",
                            "demo may import only")
        self.assertRejected("commonMain/demo/DemoCopy.kt", "import com.cantos.theatre.domain.playback.X",
                            "demo may import only")

    def test_package_must_match_the_directory_in_both_directions(self):
        self.assertRejected("commonMain/domain/playback/Reducer.kt", "internal val x = 1",
                            "does not match directory com.cantos.theatre.domain.playback",
                            package="com.cantos.theatre.application.playback")
        self.assertRejected("commonMain/application/playback/Controller.kt", "internal val x = 1",
                            "does not match directory com.cantos.theatre.application.playback",
                            package="com.cantos.theatre.domain.playback")
        self.write(f"commonMain/kotlin/{self.PREFIX}/domain/NoPackage.kt", "internal val x = 1\n")
        self.assertTrue(any("package (none) does not match" in error
                            for error in checks.check_theatre_architecture(self.root)))

    def test_unknown_layer_and_misplaced_sources_need_a_policy(self):
        self.write(f"commonMain/kotlin/{self.PREFIX}/newlayer/X.kt", "package com.cantos.theatre.newlayer\n")
        self.write(f"commonMain/kotlin/{self.PREFIX}/Top.kt", "package com.cantos.theatre\n")
        self.write("commonMain/kotlin/com/other/X.kt", "package com.other\n")
        self.write(f"commonMain/kotlin/{self.PREFIX}/features/Loose.kt", "package com.cantos.theatre.features\n")
        errors = checks.check_theatre_architecture(self.root)
        self.assertEqual(sum("explicit layer policy" in error for error in errors), 2, errors)
        self.assertEqual(sum("must live under commonMain/kotlin/com/cantos/theatre" in error for error in errors), 1, errors)
        self.assertEqual(sum("belongs in features/<feature>/" in error for error in errors), 1, errors)

    def test_valid_dependencies_for_every_layer_pass(self):
        for location, body in (
            ("commonMain/domain/playback/Reducer.kt",
             "import com.cantos.theatre.domain.publication.WorkKey\nimport kotlin.math.max\nimport kotlin.collections.*"),
            ("commonMain/ports/MediaEngine.kt",
             "import com.cantos.theatre.domain.playback.Phase\nimport com.cantos.theatre.ports.Other\n"
             "import kotlinx.coroutines.flow.Flow\nimport kotlin.Result"),
            ("commonMain/application/playback/Controller.kt",
             "import com.cantos.theatre.domain.playback.Phase\nimport com.cantos.theatre.ports.MediaEngine\n"
             "import com.cantos.theatre.application.catalog.Catalog\nimport kotlinx.coroutines.CoroutineScope\n"
             "import kotlinx.coroutines.channels.Channel"),
            ("commonMain/features/shell/presentation/Root.kt",
             "import androidx.compose.runtime.Composable\nimport com.cantos.theatre.demo.DemoState\n"
             "import com.cantos.theatre.features.shell.presentation.Other\nimport com.cantos.ui.CantosColors\n"
             "import com.cantos.theatre.resources.Res\nimport org.jetbrains.compose.resources.stringResource"),
            ("androidMain/features/shell/presentation/Native.kt", "import android.content.Context"),
            ("commonMain/demo/DemoCopy.kt", "import com.cantos.theatre.demo.DemoState\nimport kotlin.math.max"),
            ("commonMain/adapters/mock/Mock.kt", "import com.cantos.theatre.app.X\nimport android.os.Build"),
            ("commonMain/app/App.kt", "import com.cantos.theatre.adapters.mock.Mock\nimport java.util.UUID"),
        ):
            with self.subTest(location=location):
                self.assertEqual(self.errors(location, body), [])

    def test_inert_text_and_member_access_are_not_dependencies(self):
        self.assertEqual(self.errors("commonMain/domain/playback/Reducer.kt", """
// import com.cantos.theatre.application.X
/* import android.os.Build
   val x: androidx.compose.runtime.Composable */
val doc = "import com.cantos.theatre.adapters.X and android.os.Build"
val raw = \"\"\"
import platform.AVFoundation.AVPlayer
\"\"\"
val a = 'x'
fun f(platform: Platform) = platform.name.length + this.platform.android.os
fun g(x: kotlin.collections.List<kotlin.Int>) = x
"""), [])

    def test_test_source_sets_are_not_scanned(self):
        for source_set in ("commonTest", "desktopTest"):
            self.write(f"{source_set}/kotlin/{self.PREFIX}/domain/playback/ReducerTest.kt",
                       "package com.cantos.theatre.domain.playback\nimport androidx.compose.runtime.Composable\n")
        self.assertEqual(checks.check_theatre_architecture(self.root), [])

    def test_reported_line_follows_comments_and_multiline_strings(self):
        errors = self.errors("commonMain/domain/playback/Reducer.kt", """/* header
 * two lines */
import com.cantos.theatre.domain.publication.WorkKey
val text = \"\"\"a
b ${1 +
2}\"\"\"
import com.cantos.theatre.application.X
""")
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("Reducer.kt:9: com.cantos.theatre.application.X", errors[0])

    def test_real_tree_is_clean_and_each_layer_mutant_fails_on_its_own_file(self):
        self.assertEqual(checks.check_theatre_architecture(checks.ROOT), [])
        source = checks.ROOT / checks.THEATRE_SOURCE
        mutants = (
            ("domain/playback/PlaybackReducer.kt", "import com.cantos.theatre.application.playback.PlaybackController", "domain"),
            ("ports/MediaEngine.kt", "import com.cantos.theatre.adapters.mock.MockCatalogAdapter", "ports"),
            ("application/playback/PlaybackController.kt", "import androidx.compose.runtime.Composable", "application"),
            ("features/shell/presentation/PlayerSection.kt", "import com.cantos.theatre.app.CopyKey", "features"),
            ("demo/DemoCopy.kt", "import com.cantos.theatre.app.Navigation", "demo"),
        )
        for location, forbidden, layer in mutants:
            with self.subTest(layer=layer), tempfile.TemporaryDirectory() as directory:
                copy = Path(directory)
                shutil.copytree(source, copy / checks.THEATRE_SOURCE)
                target = copy / checks.THEATRE_SOURCE / "commonMain/kotlin/com/cantos/theatre" / location
                text = target.read_text(encoding="utf-8")
                package_end = text.index("\n", text.index("package "))
                target.write_text(text[:package_end + 1] + forbidden + "\n" + text[package_end + 1:], encoding="utf-8")
                errors = checks.check_theatre_architecture(copy)
                self.assertEqual(len(errors), 1, errors)
                self.assertIn(location, errors[0])
                self.assertIn(forbidden.split()[1], errors[0])


if __name__ == "__main__":
    unittest.main()
