#!/usr/bin/env python3
"""Blank out example credentials in a vendored OpenAPI spec, in place.

Atlassian's specs contain sample access tokens (e.g. Bitbucket's
RestRawAccessToken and RestBearerTokenCredentials). They are documentation
examples, but they match real token formats, so secret scanners (GitHub push
protection) reject commits containing them. This replaces the `example` of any
property whose name suggests a credential.

Usage: redact-spec-examples.py <spec.json>
"""

import json
import sys

SENSITIVE = ("token", "password", "secret", "apikey", "api_key")
PLACEHOLDER = "REDACTED-EXAMPLE"


def redact(node, key=""):
    count = 0
    if isinstance(node, dict):
        if any(s in key.lower() for s in SENSITIVE) and isinstance(node.get("example"), str):
            node["example"] = PLACEHOLDER
            count += 1
        for k, v in node.items():
            count += redact(v, k)
    elif isinstance(node, list):
        for v in node:
            count += redact(v, key)
    return count


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    path = sys.argv[1]
    with open(path, encoding="utf-8") as f:
        spec = json.load(f)
    n = redact(spec)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(spec, f, separators=(",", ":"))
        f.write("\n")
    print(f"  redacted {n} example credential(s) in {path}")


if __name__ == "__main__":
    main()
