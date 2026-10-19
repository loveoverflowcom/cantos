"""Static regressions for the opt-in Cloudflare Pages lane of .circleci/config.yml.

These read the config text only. They establish that the wiring is opt-in, per-app and credential
separated; they do not run CircleCI, Cloudflare or any deployment (see docs/deployment/cloudflare-pages.md).
"""

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
CONFIG = (ROOT / ".circleci/config.yml").read_text()


def section(name):
    """The text of one top-level mapping (`jobs`, `workflows`, `parameters`)."""
    match = re.search(rf"^{name}:\n(.*?)(?=^\S|\Z)", CONFIG, re.S | re.M)
    assert match, name
    return match.group(1)


def entries(name):
    """Two-space-indented entries of a top-level mapping, keyed by name."""
    body = section(name)
    parts = re.split(r"^  ([A-Za-z0-9_-]+):\n", body, flags=re.M)
    return {parts[i]: parts[i + 1] for i in range(1, len(parts), 2)}


JOBS = entries("jobs")
WORKFLOWS = entries("workflows")
CLOUDFLARE_WORKFLOWS = ("release-cloudflare-theatre-web", "release-cloudflare-studio-web")


class CloudflareLane(unittest.TestCase):
    def test_ordinary_pushes_and_pull_requests_start_no_workflow(self):
        for name, body in WORKFLOWS.items():
            with self.subTest(workflow=name):
                self.assertRegex(body, r"^    when:\n", "an unconditional workflow would run on every push")
                self.assertTrue(
                    "<< pipeline.parameters.target >>" in body or "<< pipeline.parameters.build >>" in body
                )

    def test_firebase_stays_the_default_and_cloudflare_is_an_explicit_choice(self):
        parameters = section("parameters")
        self.assertRegex(parameters, r"web-provider:\n    type: enum\n    enum: \[firebase, cloudflare\]\n    default: firebase")
        for name in ("deploy-theatre-web", "deploy-studio-web"):
            self.assertIn("equal: [firebase, << pipeline.parameters.web-provider >>]", WORKFLOWS[name])
        for name in CLOUDFLARE_WORKFLOWS:
            self.assertIn("equal: [cloudflare, << pipeline.parameters.web-provider >>]", WORKFLOWS[name])
            self.assertIn("not: << pipeline.parameters.build >>", WORKFLOWS[name])

    def test_each_app_has_its_own_workflow_serial_group_and_never_names_the_other(self):
        theatre, studio = (WORKFLOWS[name] for name in CLOUDFLARE_WORKFLOWS)
        self.assertIn("app: cantos-theatre", theatre)
        self.assertNotIn("studio", theatre.replace("studio-web", "").lower().replace("equal: [web", ""))
        self.assertIn("app: cantos-studio", studio)
        self.assertNotIn("theatre", studio.replace("theatre-web", "").lower())
        self.assertIn("cloudflare-theatre-web", theatre)
        self.assertIn("cloudflare-studio-web", studio)
        for body in (theatre, studio):
            self.assertIn("serial-group:", body)

    def test_publishing_needs_a_manual_approval_after_packaging(self):
        for name in CLOUDFLARE_WORKFLOWS:
            body = WORKFLOWS[name]
            with self.subTest(workflow=name):
                self.assertIn("type: approval", body)
                self.assertRegex(body, r"package-(theatre|studio)-web")
                self.assertRegex(body, r"requires: \[approve-cloudflare-(theatre|studio)-web\]")

    def test_only_the_publishing_job_receives_the_cloudflare_context_and_never_production(self):
        for name, body in WORKFLOWS.items():
            with self.subTest(workflow=name):
                if name in CLOUDFLARE_WORKFLOWS:
                    self.assertEqual(body.count("context: [cloudflare-web]"), 1)
                    self.assertNotIn("production", body.replace("production-", ""))
                    package = re.search(r"- package-web-release:\n(.*?)(?=\n      - )", body, re.S).group(1)
                    self.assertNotIn("context", package)
                else:
                    self.assertNotIn("cloudflare-web", body)

    def test_the_packaging_job_has_no_credentials_and_the_publisher_never_builds(self):
        package, deploy = JOBS["package-web-release"], JOBS["deploy-web-cloudflare"]
        self.assertNotIn("CLOUDFLARE_", package)
        self.assertIn("resolve-target", package)
        self.assertIn("cargo xtask release-build << parameters.app >>", package)
        self.assertIn("persist_to_workspace", package)
        for forbidden in ("cargo xtask release-build", "trunk", "cargo install", "cargo xtask build", "cargo xtask ci", "npx", "npm install", "npm i "):
            self.assertNotIn(forbidden, deploy, forbidden)
        self.assertIn("attach_workspace", deploy)
        self.assertIn("npm ci --ignore-scripts --prefix infra/cloudflare/wrangler", deploy)
        self.assertIn("args=(deploy cloudflare << parameters.app >> --web", deploy)
        self.assertIn('cargo xtask "${args[@]}" --dry-run', deploy)

    def test_the_publisher_pins_the_artifact_sha_inside_origin_develop(self):
        deploy = JOBS["deploy-web-cloudflare"]
        self.assertIn("merge-base --is-ancestor", deploy)
        self.assertIn("refs/remotes/origin/develop", deploy)
        self.assertIn("git checkout --detach", deploy)
        self.assertRegex(deploy, r"\[0-9a-f\]\{40\}")

    def test_node_is_checked_for_the_pinned_wrangler_instead_of_copied_from_an_old_example(self):
        deploy = JOBS["deploy-web-cloudflare"]
        self.assertIn("-ge 22", deploy)
        self.assertNotRegex(deploy, r"node-v(1\d|20|21)\.")

    def test_pipeline_parameters_reach_shell_only_as_environment_data(self):
        for job in ("package-web-release", "deploy-web-cloudflare"):
            commands = re.findall(r"command: \|\n((?:          .*\n|\n)+)", JOBS[job])
            for command in commands:
                self.assertNotIn("pipeline.parameters", command, job)
        deploy = JOBS["deploy-web-cloudflare"]
        self.assertIn("WEB_BRANCH: << pipeline.parameters.web-branch >>", deploy)
        self.assertIn("WEB_ENVIRONMENT: << pipeline.parameters.web-environment >>", deploy)

    def test_the_token_scope_and_isolation_are_documented_in_the_config_header(self):
        header = CONFIG.split("\nparameters:\n")[0]
        self.assertIn("Account / Cloudflare Pages / Edit", header)
        self.assertIn("no DNS", header)
        self.assertIn("never", header.split("Cloudflare Pages")[1])

    def test_run_records_are_stored_but_never_the_credential_directories(self):
        deploy = JOBS["deploy-web-cloudflare"]
        self.assertIn("path: artifacts/cloudflare", deploy)
        self.assertNotIn("~/.config", deploy)


if __name__ == "__main__":
    unittest.main()
