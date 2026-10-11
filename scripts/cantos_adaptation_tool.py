#!/usr/bin/env python3
"""Bounded CLI tools for Cantos' authenticated, caller-owned adaptation pipeline.

No inference, shell execution, credential provisioning or automatic retry occurs here.
An agent with an authorized terminal tool invokes one command with JSON on stdin.
"""
import argparse
import http.client
import ipaddress
import json
import math
import os
from pathlib import Path
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

MAX_INPUT_BYTES = 1024 * 1024
MAX_RESPONSE_BYTES = 8 * 1024 * 1024
TIMEOUT_SECONDS = 10
ROOT = Path(__file__).resolve().parents[1]
UUID = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")


class ToolInputError(ValueError):
    pass


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ToolInputError("duplicate_json_key")
        result[key] = value
    return result


def reject_constant(_value):
    raise ToolInputError("non_finite_json_number")


def finite_float(value):
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ToolInputError("non_finite_json_number")
    return parsed


def decode_json(data):
    try:
        value = json.loads(data, object_pairs_hook=unique_object, parse_constant=reject_constant,
                           parse_float=finite_float)
        pending = [value]
        while pending:
            current = pending.pop()
            if isinstance(current, str):
                current.encode("utf-8")  # Escaped lone surrogates must not survive admission.
            elif isinstance(current, dict):
                pending.extend(current.keys())
                pending.extend(current.values())
            elif isinstance(current, list):
                pending.extend(current)
        return value
    except (ValueError, UnicodeError, RecursionError):
        raise ToolInputError("invalid_json") from None


def read_input(stream):
    raw = stream.read(MAX_INPUT_BYTES + 1)
    if len(raw) > MAX_INPUT_BYTES:
        raise ToolInputError("input_too_large")
    value = decode_json(raw)
    if not isinstance(value, dict):
        raise ToolInputError("object_required")
    return value


def local_origin(value):
    try:
        parsed = urllib.parse.urlsplit(value)
        address = ipaddress.ip_address(parsed.hostname or "")
        port = parsed.port
    except ValueError:
        raise ToolInputError("numeric_loopback_http_required") from None
    if (parsed.scheme != "http" or not address.is_loopback or port is None or port == 0
            or parsed.username is not None or parsed.password is not None
            or parsed.path not in ("", "/") or parsed.query or parsed.fragment):
        raise ToolInputError("numeric_loopback_http_required")
    return urllib.parse.urlunsplit(("http", parsed.netloc, "", "", ""))


def route(command, value):
    if command == "context":
        return "POST", "/api/v1/adaptations/contexts", value
    expected_keys = {"run_id", "submission"} if command == "submit" else {"run_id"}
    if set(value) != expected_keys:
        raise ToolInputError("closed_tool_arguments_required")
    run_id = value.get("run_id")
    if not isinstance(run_id, str) or not UUID.fullmatch(run_id):
        raise ToolInputError("canonical_run_uuid_required")
    if command == "submit":
        if not isinstance(value["submission"], dict):
            raise ToolInputError("submission_object_required")
        return "POST", f"/api/v1/adaptations/{run_id}/proposals", value["submission"]
    suffix = {"context-read": "context", "review": "review"}.get(command)
    if suffix is None:
        raise ToolInputError("unknown_tool")
    return "GET", f"/api/v1/adaptations/{run_id}/{suffix}", None


def response_json(response):
    raw = response.read(MAX_RESPONSE_BYTES + 1)
    if len(raw) > MAX_RESPONSE_BYTES:
        raise ToolInputError("response_too_large")
    return decode_json(raw)


def invoke(command, arguments, base_url, token):
    origin = local_origin(base_url)
    # Current operator-issued development sessions are 32 random bytes encoded as hex.
    if not isinstance(token, str) or not re.fullmatch(r"[0-9a-f]{64}", token):
        raise ToolInputError("existing_session_required")
    method, path, body = route(command, arguments)
    try:
        encoded = None if body is None else json.dumps(body, ensure_ascii=False, allow_nan=False).encode()
    except (ValueError, TypeError, RecursionError):
        raise ToolInputError("invalid_json") from None
    if encoded is not None and len(encoded) > MAX_INPUT_BYTES:
        raise ToolInputError("input_too_large")
    request = urllib.request.Request(
        origin + path, data=encoded, method=method,
        headers={"Cookie": "cantos_session=" + token, "Origin": origin,
                 "Content-Type": "application/json", "Accept": "application/json"},
    )
    # Neither environment proxies nor redirects may forward the session or source elsewhere.
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    status = None
    try:
        with opener.open(request, timeout=TIMEOUT_SECONDS) as response:
            status = response.status
            return {"ok": True, "http_status": status, "result": response_json(response)}, 0
    except urllib.error.HTTPError as error:
        try:
            result = response_json(error)
        except (ToolInputError, OSError, http.client.HTTPException):
            code = "write_outcome_unknown_retry_exact_operation" if method == "POST" else "invalid_or_incomplete_response"
            return {"ok": False, "http_status": error.code, "result": {"code": code}}, 2
        return {"ok": False, "http_status": error.code, "result": result}, 1
    except (ToolInputError, OSError, urllib.error.URLError, http.client.HTTPException):
        if method == "POST":
            code = "write_outcome_unknown_retry_exact_operation"
        else:
            code = "transport_unavailable" if status is None else "invalid_or_incomplete_response"
        return {"ok": False, "http_status": status, "result": {"code": code}}, 2


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", help="Explicit numeric loopback development host and port")
    parser.add_argument("command", choices=("schemas", "context", "context-read", "submit", "review"))
    args = parser.parse_args(argv)
    try:
        if args.command == "schemas":
            result = decode_json((ROOT / "packages/contracts/adaptation-tools-v1.json").read_bytes())
            code = 0
        else:
            if args.base_url is None:
                raise ToolInputError("explicit_base_url_required")
            result, code = invoke(args.command, read_input(sys.stdin.buffer), args.base_url,
                                  os.environ.get("CANTOS_SESSION_TOKEN"))
    except ToolInputError as error:
        result = {"ok": False, "http_status": None, "result": {"code": str(error)}}
        code = 2
    print(json.dumps(result, ensure_ascii=False, allow_nan=False))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
