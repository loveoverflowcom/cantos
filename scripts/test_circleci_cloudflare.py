"""Static CI regressions; no CircleCI execution or live Cloudflare deployment."""

import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / '.circleci/config.yml').read_text())
JOBS = CONFIG['jobs']
WORKFLOWS = CONFIG['workflows']
CLOUDFLARE_WORKFLOWS = ('release-cloudflare-theatre-web', 'release-cloudflare-studio-web')


def text(value):
    return json.dumps(value)


def commands(job):
    return '\n'.join(step['run']['command'] for step in JOBS[job]['steps']
                     if isinstance(step, dict) and 'run' in step)


class CloudflareLane(unittest.TestCase):
    def test_ordinary_pushes_start_no_unconditional_workflow(self):
        for workflow in WORKFLOWS.values():
            self.assertIn('when', workflow)
            self.assertIn('pipeline.parameters.', text(workflow['when']))

    def test_provider_selection_keeps_firebase_and_pages_separate(self):
        parameter = CONFIG['parameters']['web-provider']
        self.assertEqual(parameter['enum'], ['firebase', 'cloudflare'])
        self.assertEqual(parameter['default'], 'firebase')
        for name in ('release-theatre-web', 'release-studio-web', 'release-web', 'release-all'):
            self.assertIn({'equal': ['firebase', '<< pipeline.parameters.web-provider >>']},
                          WORKFLOWS[name]['when']['and'])
        for name in CLOUDFLARE_WORKFLOWS:
            self.assertIn({'equal': ['cloudflare', '<< pipeline.parameters.web-provider >>']},
                          WORKFLOWS[name]['when']['and'])

    def test_each_app_keeps_its_own_approval_serial_group_and_context(self):
        for component in ('theatre-web', 'studio-web'):
            jobs = WORKFLOWS['release-cloudflare-' + component]['jobs']
            package, approval, publish = (next(iter(node.values())) for node in jobs)
            self.assertNotIn('context', package)
            self.assertEqual(package['app'], component)
            self.assertEqual(approval, {'type': 'approval', 'requires': ['package-' + component]})
            self.assertEqual(publish['requires'], ['approve-cloudflare-' + component])
            self.assertEqual(publish['context'], ['cloudflare-web'])
            self.assertIn('cloudflare-' + component, publish['serial-group'])
            self.assertIn('pipeline.config.repository.url', publish['filters'])
            self.assertIn('pipeline.git.branch', publish['filters'])

    def test_packaging_uses_one_v2_request_and_explicit_origin_without_credentials(self):
        package = text(JOBS['package-web-release'])
        self.assertNotIn('CLOUDFLARE_', package)
        command = commands('package-web-release')
        self.assertIn('cargo xtask release prepare', command)
        self.assertIn('--api-origin', command)
        self.assertIn('artifacts/cloudflare-transfer/<< parameters.app >>', command)
        self.assertIn('persist_to_workspace', package)
        self.assertNotIn('resolve-target', package)

    def test_publisher_validates_inventory_instead_of_guessing_the_old_sha_layout(self):
        command = commands('deploy-web-cloudflare')
        self.assertIn('cargo xtask release checkout --transfer artifacts/cloudflare-transfer/', command)
        self.assertIn('release.promotion_input', command)
        self.assertIn('str(manifest.parent)', command)
        self.assertNotIn('directories=(', command)
        self.assertIn('attach_workspace', text(JOBS['deploy-web-cloudflare']))

    def test_publisher_never_builds_and_uses_only_the_pinned_wrangler(self):
        command = commands('deploy-web-cloudflare')
        for forbidden in ('cargo xtask release-build', 'trunk', 'cargo install', 'cargo xtask build',
                          'cargo xtask ci', 'npx', 'npm install', 'npm i '):
            self.assertNotIn(forbidden, command)
        self.assertIn('npm ci --ignore-scripts --prefix infra/cloudflare/wrangler', command)
        self.assertIn('args=(deploy cloudflare << parameters.app >> --web', command)
        self.assertIn('cargo xtask "${args[@]}" --dry-run', command)
        self.assertIn('"$major" -ge 22', command)

    def test_pipeline_parameters_reach_shell_only_through_environment(self):
        for job in ('package-web-release', 'deploy-web-cloudflare'):
            self.assertNotIn('pipeline.parameters.', commands(job))
        steps = JOBS['deploy-web-cloudflare']['steps']
        environment = next(s['run']['environment'] for s in steps if isinstance(s, dict)
                           and 'WEB_BRANCH' in s.get('run', {}).get('environment', {}))
        self.assertEqual(environment['WEB_BRANCH'], '<< pipeline.parameters.web-branch >>')
        self.assertEqual(environment['WEB_ENVIRONMENT'], '<< pipeline.parameters.web-environment >>')

    def test_run_records_are_stored_without_credential_directories(self):
        records = [s['store_artifacts'] for s in JOBS['deploy-web-cloudflare']['steps']
                   if isinstance(s, dict) and 'store_artifacts' in s]
        self.assertEqual(records, [{'when': 'always', 'path': 'artifacts/cloudflare',
                                   'destination': 'pages-run-records'}])


if __name__ == '__main__':
    unittest.main()
