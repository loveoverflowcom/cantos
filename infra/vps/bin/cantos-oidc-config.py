#!/usr/bin/env python3
"""Pure, secret-redacting plans for the two existing browser OIDC clients.

Fingerprints cover CLI-visible attributes; a provider's redacted value cannot prove
equality of the underlying secret/key bytes. No command emitted here rotates them.
"""

import hashlib
import json
from pathlib import Path
import re
import sys
from urllib.parse import urlsplit


CLIENTS = {f"cantos-{surface}-web-bff": surface for surface in ("studio", "theatre")}
MUTABLE = {"oauth2_rs_origin_landing", "oauth2_rs_origin", "oauth2_strict_redirect_uri"}
SAFE_KEYS = {"client", "landing_url", "redirect_urls", "strict_redirect", "preserved_fingerprint"}
BROWSER_KEYS = tuple(f"CANTOS_{surface}_{field}" for surface in ("THEATRE", "STUDIO")
                     for field in ("WEB_ORIGIN", "API_ORIGIN", "OIDC_CALLBACK"))
DNS = r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+"
LIMIT = 2 * 1024 * 1024


def require(condition):
    if not condition:
        raise ValueError("configuration refused")


def client_surface(client):
    require(client in CLIENTS)
    return CLIENTS[client]


def origin(value):
    require(isinstance(value, str) and len(value) <= 261
            and re.fullmatch("https://" + DNS, value) is not None)
    require(re.fullmatch(r"(?:[0-9]+|0x[0-9a-f]*)", value.rsplit(".", 1)[-1]) is None)
    return value


def canonical_url(value):
    require(isinstance(value, str) and len(value) <= 8192
            and all(32 < ord(char) < 127 for char in value)
            and "\\" not in value)
    parsed = urlsplit(value)
    origin("https://" + parsed.netloc)
    require(parsed.scheme == "https" and not parsed.query and not parsed.fragment
            and value == "https://" + parsed.netloc + parsed.path)
    return value if parsed.path else value + "/"


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def read_json(text):
    require(len(text) <= LIMIT)
    return json.loads(text, object_pairs_hook=unique_object)


def read_file(path):
    with Path(path).open("rb") as stream:
        raw = stream.read(LIMIT + 1)
    require(len(raw) <= LIMIT)
    return raw.decode("utf-8")


def attrs(raw):
    require(isinstance(raw, dict) and isinstance(raw.get("attrs"), dict))
    value = raw["attrs"]
    require(all(isinstance(key, str) and isinstance(values, list)
                and all(isinstance(item, str) for item in values) for key, values in value.items()))
    return value


def fingerprint(value):
    # Kanidm attributes are sets; ordering in the CLI JSON is not identity.
    canonical = {key: sorted(values) for key, values in value.items()}
    return hashlib.sha256(json.dumps(canonical, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=False).encode("utf-8")).hexdigest()


def snapshot(client, raw):
    client_surface(client)
    value = attrs(raw)
    require(value.get("name") == [client])
    landing = value.get("oauth2_rs_origin_landing", [])
    strict = value.get("oauth2_strict_redirect_uri", ["false"])
    require(len(landing) == 1 and strict in (["true"], ["false"]))
    redirects = [canonical_url(url) for url in value.get("oauth2_rs_origin", [])]
    require(len(redirects) == len(set(redirects)))
    return {"client": client, "landing_url": canonical_url(landing[0]),
            "redirect_urls": sorted(redirects), "strict_redirect": strict == ["true"],
            "preserved_fingerprint": fingerprint({key: values for key, values in value.items()
                                                  if key not in MUTABLE | {"last_modified_cid"}})}


def safe(value):
    require(isinstance(value, dict) and set(value) == SAFE_KEYS)
    client_surface(value["client"])
    require(canonical_url(value["landing_url"]) == value["landing_url"]
            and isinstance(value["strict_redirect"], bool)
            and isinstance(value["redirect_urls"], list))
    redirects = [canonical_url(url) for url in value["redirect_urls"]]
    require(redirects == sorted(set(redirects)) == value["redirect_urls"])
    digest = value["preserved_fingerprint"]
    require(digest is None or isinstance(digest, str) and re.fullmatch("[0-9a-f]{64}", digest))
    return value


def target(client, web, callback):
    surface = client_surface(client)
    landing = origin(web) + "/"
    suffix = f"/api/v1/{surface}/auth/callback"
    require(callback.endswith(suffix))
    origin(callback[:-len(suffix)])
    return {"client": client, "landing_url": landing, "redirect_urls": [callback],
            "strict_redirect": True, "preserved_fingerprint": None}


def plan(client, current, desired):
    client_surface(client)
    current, desired = safe(current), safe(desired)
    require(current["client"] == desired["client"] == client and current["preserved_fingerprint"] is not None)
    require(desired["preserved_fingerprint"] in (None, current["preserved_fingerprint"]))
    # Only a captured baseline can disable strict matching; target() always enables it.
    require(desired["strict_redirect"] or desired["preserved_fingerprint"] is not None)
    actions = []
    if current["landing_url"] != desired["landing_url"]:
        actions.append(("set-landing-url", desired["landing_url"]))
    actions += [("add-redirect-url", url) for url in desired["redirect_urls"] if url not in current["redirect_urls"]]
    actions += [("remove-redirect-url", url) for url in current["redirect_urls"] if url not in desired["redirect_urls"]]
    if current["strict_redirect"] != desired["strict_redirect"]:
        actions.append((("enable" if desired["strict_redirect"] else "disable") + "-strict-redirect-url",))
    return actions


def verify(before, after, desired=None):
    before, after = safe(before), safe(after)
    require(before["client"] == after["client"] and before["preserved_fingerprint"] is not None
            and before["preserved_fingerprint"] == after["preserved_fingerprint"])
    if desired is not None:
        desired = safe(desired)
        require(desired["client"] == after["client"]
                and desired["preserved_fingerprint"] in (None, before["preserved_fingerprint"]))
        require(all(after[key] == desired[key] for key in ("landing_url", "redirect_urls", "strict_redirect")))


def group_fingerprint(raw):
    value = attrs(raw)
    require(len(value.get("name", [])) == 1 and len(value.get("uuid", [])) == 1)
    keys = {"name", "uuid", "spn", "member", "dynmember", "memberof", "directmemberof"}
    return fingerprint({key: values for key, values in value.items() if key in keys})


def secret_fingerprint(text):
    # Accept the CLI's single terminal LF, never trim secret data or multiple lines.
    value = text.removesuffix("\n")
    require("\n" not in value and "\r" not in value)
    if value.startswith('"'):
        value = read_json(value)
    require(isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_-]{16,4096}", value) is not None)
    return hashlib.sha256(value.encode("ascii")).hexdigest()


def env_assignments(text, inputs=False):
    require("\r" not in text and "\x00" not in text and (not inputs or text.isascii()))
    result = {}
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        match = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)=(.*)", line)
        if match is None:
            require(not inputs and not any(key in line for key in (*BROWSER_KEYS, "CANTOS_API_HOST")))
            continue
        key, value = match.groups()
        require(key not in result and (not inputs or key in BROWSER_KEYS))
        result[key] = value
    return result


def merge_browser(inputs, edge):
    updates, existing = env_assignments(inputs, True), env_assignments(edge)
    require(set(updates) == set(BROWSER_KEYS))
    api = origin("https://" + existing.get("CANTOS_API_HOST", ""))
    for surface in ("THEATRE", "STUDIO"):
        web, api_origin, callback = (updates[f"CANTOS_{surface}_{field}"]
                                     for field in ("WEB_ORIGIN", "API_ORIGIN", "OIDC_CALLBACK"))
        target(f"cantos-{surface.lower()}-web-bff", web, callback)
        require(api_origin == api and callback == api + f"/api/v1/{surface.lower()}/auth/callback")
    require(updates["CANTOS_THEATRE_WEB_ORIGIN"] != updates["CANTOS_STUDIO_WEB_ORIGIN"])
    output, replaced = [], set()
    for line in edge.splitlines(keepends=True):
        key = line.partition("=")[0]
        if key in updates:
            output.append(key + "=" + updates[key] + ("\n" if line.endswith("\n") else ""))
            replaced.add(key)
        else:
            output.append(line)
    missing = [key for key in BROWSER_KEYS if key not in replaced]
    if missing and output and not output[-1].endswith("\n"):
        output.append("\n")
    return "".join(output) + "".join(key + "=" + updates[key] + "\n" for key in missing)


def main(arguments):
    try:
        command, *args = arguments
        if command == "snapshot" and len(args) == 1:
            result = snapshot(args[0], read_json(sys.stdin.read(LIMIT + 1)))
        elif command == "target" and len(args) == 3:
            result = target(*args)
        elif command == "plan" and len(args) == 3:
            actions = plan(args[0], *(read_json(read_file(path)) for path in args[1:]))
            sys.stdout.write("".join("\t".join(action) + "\n" for action in actions))
            return 0
        elif command == "verify" and len(args) in (2, 3):
            verify(*(read_json(read_file(path)) for path in args))
            return 0
        elif command == "group-fingerprint" and not args:
            print(group_fingerprint(read_json(sys.stdin.read(LIMIT + 1))))
            return 0
        elif command == "secret-fingerprint" and not args:
            print(secret_fingerprint(sys.stdin.read(LIMIT + 1)))
            return 0
        elif command == "merge-browser" and len(args) == 2:
            sys.stdout.write(merge_browser(*(read_file(path) for path in args)))
            return 0
        else:
            raise ValueError("invalid command")
        print(json.dumps(result, sort_keys=True))
        return 0
    except (ValueError, TypeError, KeyError, OSError, UnicodeError):
        # Parse/library exceptions can contain source values: never forward their text.
        print("cantos-oidc-config: configuration refused", file=sys.stderr)
        return 65


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
