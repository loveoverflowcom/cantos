"""Static edge-policy regressions, not Caddy parsing or HTTP/runtime acceptance.

ADR 0039 retired local SPA/file-server caching; ADR 0041 retains that proxy only as a
transition. These checks preserve API separation and code-bearing response protection.
The independent pinned-Caddy adapter check supplies syntax/route-order evidence.
"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
CADDYFILE = ROOT / "infra/edge/Caddyfile"


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
        elif line == "{" or line.endswith(" {"):
            body = []
            stack[-1].append((line[:-2] if line != "{" else "global_options", body))
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
        self.source = CADDYFILE.read_text()
        self.config = statements(self.source)

    def test_api_ingress_keeps_the_studio_process_separate_from_theatre(self):
        api = block(self.config, "{$CANTOS_API_HOST}")
        self.assertEqual(block(api, "handle /api/v1/studio/*"),
                         [("import cantos_api cantos-api-studio", None)])
        self.assertIn("import cantos_theatre_api", lines(api))
        theatre = block(self.config, "(cantos_theatre_api)")
        self.assertEqual(block(theatre, "handle /api/*"),
                         [("import cantos_api cantos-api-theatre", None)])
        self.assertEqual(block(api, "handle"), [('respond "Cantos API" 404', None)])

    def test_transitional_static_proxy_cannot_route_api_requests_to_firebase(self):
        studio = block(self.config, "{$CANTOS_STUDIO_HOST}")
        self.assertEqual(block(studio, "handle /api/*"),
                         [("import cantos_api cantos-api-studio", None)])
        self.assertEqual(block(studio, "handle"),
                         [("import firebase_site {$CANTOS_STUDIO_FIREBASE_SITE}", None)])
        theatre = block(self.config, "{$CANTOS_THEATRE_HOST}")
        self.assertIn("import cantos_theatre_api", lines(theatre))
        self.assertNotIn("file_server", self.source)
        self.assertNotIn("immutable", self.source)
        proxy = block(self.config, "(firebase_site)")
        upstream = block(proxy, "reverse_proxy https://{args[0]}.web.app")
        self.assertIn("header_up Host {upstream_hostport}", lines(upstream))
        self.assertFalse(any("Cache-Control" in line for line in lines(proxy)))

    def test_email_action_headers_and_query_scrub_precede_static_upstream(self):
        theatre = block(self.config, "{$CANTOS_THEATRE_HOST}")
        fallback = block(theatre, "handle")
        self.assertIn("@action_code query oobCode=*", lines(fallback))
        self.assertEqual(lines(block(fallback, "route")), [
            'header @action_code >Cache-Control "no-store"',
            'header @action_code >Referrer-Policy "no-referrer"',
            'uri @action_code query -oobCode',
            "import firebase_site {$CANTOS_THEATRE_FIREBASE_SITE}",
        ])

    def test_default_security_headers_do_not_replace_stricter_api_policy(self):
        headers = block(block(self.config, "(cantos_security)"), "header")
        self.assertIn("?Referrer-Policy strict-origin-when-cross-origin", lines(headers))
        self.assertIn("?X-Content-Type-Options nosniff", lines(headers))
        self.assertFalse(any("Cache-Control" in line for line in lines(headers)))


if __name__ == "__main__":
    unittest.main()
