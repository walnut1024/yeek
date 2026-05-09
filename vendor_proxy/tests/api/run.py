#!/usr/bin/env python3
"""vendor_proxy integration test runner.

Usage:
    # 1. Start the proxy in another terminal:
    #    DEEPSEEK_API_KEY=sk-xxx ZHIPU_API_KEY=xxx cargo run -p vendor-proxy vendor_proxy/proxy.toml
    #
    # 2. Run tests:
    #    uv run vendor_proxy/tests/api/run.py
    #
    # Override base URL:
    #    BASE_URL=http://127.0.0.1:9999 uv run vendor_proxy/tests/api/run.py

    # Run specific tests by name:
    #    uv run vendor_proxy/tests/api/run.py health_check anthropic_to_chat_non_stream
"""

import json
import os
import sys
import time
import traceback
from pathlib import Path

import httpx
import yaml

# ── Config ───────────────────────────────────────────────────────────

BASE_URL = os.getenv("BASE_URL", "http://127.0.0.1:8787")
TESTCASES_FILE = Path(__file__).parent / "testcases.yaml"
TIMEOUT = httpx.Timeout(45.0, connect=5.0)

# Env vars available as {{VAR}} in testcases
TEMPLATE_VARS = {
    "BASE_URL": BASE_URL,
    "DEEPSEEK_API_KEY": os.getenv("DEEPSEEK_API_KEY", ""),
    "ZHIPU_API_KEY": os.getenv("ZHIPU_API_KEY", ""),
    "DEEPSEEK_MODEL": "deepseek-v4-pro",
    "ZHIPU_MODEL": "glm-5.1",
}


def resolve_templates(obj):
    """Recursively replace {{VAR}} placeholders in strings."""
    if isinstance(obj, str):
        for key, val in TEMPLATE_VARS.items():
            obj = obj.replace(f"{{{{{key}}}}}", str(val))
        return obj
    if isinstance(obj, dict):
        return {k: resolve_templates(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [resolve_templates(i) for i in obj]
    return obj


# ── Assertions ───────────────────────────────────────────────────────

def check_assertions(expect: dict, resp: httpx.Response, test_name: str) -> list[str]:
    """Run assertions against response. Returns list of error messages."""
    errors = []

    # Status code
    if "status" in expect:
        expected_status = expect["status"]
        if resp.status_code != expected_status:
            errors.append(f"  status: expected {expected_status}, got {resp.status_code}")

    # Latency
    if "latency_ms" in expect:
        # resp.elapsed is a timedelta from httpx
        elapsed_ms = resp.elapsed.total_seconds() * 1000
        limit = expect["latency_ms"]
        if elapsed_ms > limit:
            errors.append(f"  latency: {elapsed_ms:.0f}ms > {limit}ms limit")

    # JSON body assertions
    body_text = resp.text
    try:
        body = json.loads(body_text) if body_text else {}
    except json.JSONDecodeError:
        body = {}

    # json can be a single dict or a list of dicts
    json_assertions = expect.get("json", [])
    if isinstance(json_assertions, dict):
        json_assertions = [json_assertions]

    for assertion in json_assertions:
        path = assertion.get("path", "")
        value = _resolve_path(body, path)

        if "equals" in assertion:
            expected = assertion["equals"]
            if value != expected:
                errors.append(f"  json {path}: expected {expected!r}, got {value!r}")

        if "not_equals" in assertion:
            unexpected = assertion["not_equals"]
            if value == unexpected:
                errors.append(f"  json {path}: should not equal {unexpected!r}")

        if "type" in assertion:
            expected_type = assertion["type"]
            type_ok = (
                (expected_type == "array" and isinstance(value, list))
                or (expected_type == "string" and isinstance(value, str))
                or (expected_type == "number" and isinstance(value, (int, float)))
                or (expected_type == "object" and isinstance(value, dict))
            )
            if not type_ok:
                errors.append(f"  json {path}: expected type {expected_type}, got {type(value).__name__} ({value!r})")

        if "min_length" in assertion:
            if not isinstance(value, list) or len(value) < assertion["min_length"]:
                errors.append(f"  json {path}: expected min_length {assertion['min_length']}, got {len(value) if isinstance(value, list) else 'N/A'}")

    return errors


def check_stream_assertions(expect: dict, lines: list[str], test_name: str) -> list[str]:
    """Run assertions against collected SSE stream lines."""
    errors = []
    full_text = "\n".join(lines)

    for keyword in expect.get("stream_contains", []):
        if keyword not in full_text:
            errors.append(f"  stream: expected keyword '{keyword}' not found in SSE output")

    return errors


def _resolve_path(obj, path: str):
    """Resolve dot-notation path like 'content.0.text'."""
    parts = path.split(".")
    current = obj
    for part in parts:
        if current is None:
            return None
        if isinstance(current, dict):
            current = current.get(part)
        elif isinstance(current, list):
            try:
                current = current[int(part)]
            except (IndexError, ValueError):
                return None
        else:
            return None
    return current


# ── Test Runner ──────────────────────────────────────────────────────

def run_test(tc: dict) -> tuple[bool, str, float]:
    """Execute a single test case. Returns (passed, output, elapsed_ms)."""
    name = tc["name"]
    method = tc.get("method", "POST")
    path = tc.get("path", "/")
    headers = resolve_templates(tc.get("headers", {}))
    expect = tc.get("expect", {})
    is_stream = tc.get("stream", False)
    timeout_ms = expect.get("latency_ms", 30000)
    timeout_s = timeout_ms / 1000.0

    url = f"{BASE_URL}{path}"

    # Build body
    if "body_raw" in tc:
        body = resolve_templates(tc["body_raw"])
    elif "body" in tc:
        body = json.dumps(resolve_templates(tc["body"]))
    else:
        body = None

    output_lines = []
    start = time.monotonic()

    try:
        if is_stream:
            # Streaming request
            with httpx.Client(timeout=httpx.Timeout(timeout_s, connect=5.0)) as client:
                with client.stream(method, url, headers=headers, content=body) as resp:
                    stream_lines = []
                    for line in resp.iter_lines():
                        stream_lines.append(line)
                        output_lines.append(f"    {line[:120]}{'...' if len(line) > 120 else ''}")

                    # Stream body already consumed via iter_lines; skip read
                    elapsed_ms = (time.monotonic() - start) * 1000
                    # Only check status for stream tests (body is in stream_lines)
                    errors = []
                    if "status" in expect:
                        if resp.status_code != expect["status"]:
                            errors.append(f"  status: expected {expect['status']}, got {resp.status_code}")
                    errors.extend(check_stream_assertions(expect, stream_lines, name))

        else:
            # Non-streaming request
            with httpx.Client(timeout=httpx.Timeout(timeout_s, connect=5.0)) as client:
                resp = client.request(method, url, headers=headers, content=body)

            elapsed_ms = (time.monotonic() - start) * 1000
            errors = check_assertions(expect, resp, name)

            # Truncate response body for output
            body_preview = resp.text[:200] + ("..." if len(resp.text) > 200 else "")
            output_lines.append(f"    status={resp.status_code}  body={body_preview}")

    except httpx.ConnectError:
        elapsed_ms = (time.monotonic() - start) * 1000
        errors = ["  CONNECTION FAILED — is the proxy running?"]
    except httpx.ReadTimeout:
        elapsed_ms = (time.monotonic() - start) * 1000
        errors = [f"  TIMEOUT after {timeout_s}s"]
    except Exception as e:
        elapsed_ms = (time.monotonic() - start) * 1000
        errors = [f"  EXCEPTION: {e}"]
        traceback.print_exc()

    passed = len(errors) == 0
    output = "\n".join(output_lines)
    if errors:
        output += "\n" + "\n".join(errors)

    return passed, output, elapsed_ms


# ── Main ─────────────────────────────────────────────────────────────

def main():
    # Filter tests by name if args provided
    filter_names = set(sys.argv[1:]) if len(sys.argv) > 1 else None

    with open(TESTCASES_FILE) as f:
        testcases = yaml.safe_load(f)

    testcases = [tc for tc in testcases if tc.get("name")]
    if filter_names:
        testcases = [tc for tc in testcases if tc["name"] in filter_names]

    if not testcases:
        print("No test cases to run.")
        sys.exit(1)

    print(f"vendor_proxy integration tests — {BASE_URL}")
    print(f"{'=' * 60}")

    passed = 0
    failed = 0
    skipped = 0
    results = []

    for tc in testcases:
        name = tc["name"]
        desc = tc.get("description", "")
        label = f"{name}" + (f"  ({desc})" if desc else "")
        print(f"\n  {label}")
        print(f"  {'-' * len(label)}")

        ok, output, elapsed_ms = run_test(tc)
        results.append((name, ok, elapsed_ms, output))

        if ok:
            passed += 1
            print(f"  PASS  ({elapsed_ms:.0f}ms)")
        else:
            failed += 1
            print(f"  FAIL  ({elapsed_ms:.0f}ms)")
            print(output)

    # Summary
    print(f"\n{'=' * 60}")
    total = passed + failed
    status = "ALL PASSED" if failed == 0 else f"{failed} FAILED"
    print(f"Results: {passed}/{total} passed  [{status}]")

    if failed > 0:
        print("\nFailed tests:")
        for name, ok, ms, output in results:
            if not ok:
                print(f"  - {name} ({ms:.0f}ms)")

    sys.exit(1 if failed > 0 else 0)


if __name__ == "__main__":
    main()
