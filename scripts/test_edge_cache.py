"""Static edge-policy regressions, not Caddy parsing or HTTP/runtime acceptance.

Caddy normally orders header before try_files. Its documented SPA pattern uses
route to classify the rewritten path instead:
https://caddyserver.com/docs/caddyfile/patterns#single-page-apps-spas
"""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
CADDYFILE = ROOT / "services/edge/Caddyfile"
IMMUTABLE = 'header @hashed Cache-Control "public, max-age=31536000, immutable"'
REVALIDATE = 'header Cache-Control "no-cache"'


def statements(source):
    """Read this file's line-oriented blocks without treating placeholders as braces.

    This deliberately is not a general Caddyfile parser or a syntax validator.
    Only standalone closing braces and a trailing space-plus-opening-brace
    delimit blocks in this checked-in configuration.
    """
    root = []
    stack = [root]
    for raw in source.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line == "}":
            if len(stack) == 1:
                raise AssertionError("unexpected block end")
            stack.pop()
        elif line.endswith(" {"):
            body = []
            stack[-1].append((line[:-2], body))
            stack.append(body)
        else:
            stack[-1].append((line, None))
    if len(stack) != 1:
        raise AssertionError("unclosed block")
    return root


def block(nodes, heading):
    found = [body for line, body in nodes if line == heading and body is not None]
    if len(found) != 1:
        raise AssertionError(f"expected one {heading!r} block, found {len(found)}")
    return found[0]


def lines(nodes):
    return [line for line, _ in nodes]


class EdgeCachePolicy(unittest.TestCase):
    def setUp(self):
        self.config = statements(CADDYFILE.read_text())
        self.spa = block(self.config, "(cantos_spa)")

    def test_fallback_rewrite_precedes_both_header_writes_in_one_literal_route(self):
        # Order within route is literal. Outside route, merely moving the header
        # text below try_files does not fix the default directive-order bug.
        route = block(self.spa, "route")
        self.assertEqual(route, [
            ("try_files {path} /index.html", None),
            (REVALIDATE, None),
            (IMMUTABLE, None),
            ("file_server", None),
        ])
        self.assertFalse(any("Cache-Control" in line for line in lines(self.spa)),
                         "no cache header may run before the literal route")
        self.assertEqual([line for line in lines(self.spa) if line.startswith("root ")],
                         ["root * {args[0]}"])

    def test_immutable_requires_an_existing_file_not_an_alternate_shell_match(self):
        matcher = block(self.spa, "@hashed")
        self.assertEqual([line for line in lines(matcher) if line.startswith("file")],
                         ["file {path}"],
                         "missing assets and a missing shell must not match immutable")
        self.assertEqual(len(matcher), 2)
        self.assertTrue(all(body is None for _, body in matcher))

    def test_hash_matcher_accepts_only_supported_content_hashed_asset_paths(self):
        matcher = block(self.spa, "@hashed")
        patterns = [line.removeprefix("path_regexp ") for line in lines(matcher)
                    if line.startswith("path_regexp ")]
        self.assertEqual(len(patterns), 1)
        pattern = re.compile(patterns[0])
        accepted = [
            "/app-01234567.js",
            "/assets/app-0123456789abcdef.js",
            "/app-0123456789abcdef_bg.wasm",
            "/style-0123456789abcdef.css",
            "/app-" + "a" * 64 + ".wasm",
        ]
        rejected = [
            "/index.html",
            "/index-0123456789abcdef.html",
            "/library/episode/123",
            "/app.js",
            "/app.wasm",
            "/style.css",
            "/font-0123456789abcdef.woff2",
            "/art-0123456789abcdef.webp",
            "/app-0123456.js",
            "/app-0123456g.js",
            "/app-0123456789abcdef.js.map",
            "/app-0123456789abcdef.js/index.html",
        ]
        for path in accepted:
            with self.subTest(path=path):
                self.assertIsNotNone(pattern.search(path))
        for path in rejected:
            with self.subTest(path=path):
                self.assertIsNone(pattern.search(path))

    def test_both_web_hosts_share_the_policy_only_in_the_non_api_fallback(self):
        for host, root in [("cantoss.duckdns.org", "/srv/theatre"),
                           ("studio.cantoss.duckdns.org", "/srv/studio")]:
            with self.subTest(host=host):
                site = block(self.config, host)
                self.assertEqual(block(site, "handle /api/*"), [("import cantos_api", None)])
                self.assertEqual(block(site, "handle"), [(f"import cantos_spa {root}", None)])
                self.assertFalse(any("Cache-Control" in line for line in lines(site)))
        api = block(self.config, "api.cantoss.duckdns.org")
        self.assertEqual(block(api, "handle /api/*"), [("import cantos_api", None)])
        self.assertEqual(block(api, "handle"), [('respond "Cantos API" 404', None)])


if __name__ == "__main__":
    unittest.main()
