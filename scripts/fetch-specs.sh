#!/usr/bin/env bash
# Download the official Atlassian Data Center OpenAPI specs into specs/.
#
# The URLs below are the best-known locations on dac-static.atlassian.com and
# are confirmed for jira, confluence and bitbucket.
# specs/jira-9.12.json is generated separately; see specs/README.md.
#
# If one fails, find the "Download OpenAPI spec" link on the product's REST API
# page on developer.atlassian.com and pass it in:
#
#   BITBUCKET_SPEC_URL=https://.../9.6.swagger.v3.json scripts/fetch-specs.sh
#
# Usage: scripts/fetch-specs.sh [product ...]   (default: all products)
set -euo pipefail

cd "$(dirname "$0")/.."
mkdir -p specs

declare -A URLS=(
  # Jira Software DC spec: covers /rest/api/2 and /rest/agile/1.0 (used by both jira and agile tests).
  [jira]="${JIRA_SPEC_URL:-https://dac-static.atlassian.com/server/jira/platform/jira_software_dc_10000_swagger.v3.json}"
  [confluence]="${CONFLUENCE_SPEC_URL:-https://dac-static.atlassian.com/server/confluence/10.2.15.swagger.v3.json}"
  [bitbucket]="${BITBUCKET_SPEC_URL:-https://dac-static.atlassian.com/server/bitbucket/10.2.swagger.v3.json}"
)

products=("$@")
[ ${#products[@]} -eq 0 ] && products=(jira confluence bitbucket)

failed=0
for product in "${products[@]}"; do
  url="${URLS[$product]:-}"
  if [ -z "$url" ]; then
    echo "unknown product: $product (expected one of: ${!URLS[*]})" >&2
    exit 2
  fi
  out="specs/$product.json"
  tmp="$(mktemp)"
  echo "fetching $product from $url"
  if curl -fsSL --retry 3 -o "$tmp" "$url" && grep -q '"paths"' "$tmp"; then
    mv "$tmp" "$out"
    # Sample tokens in the specs trip GitHub push protection; blank them.
    python3 -I scripts/redact-spec-examples.py "$out"
    echo "  saved $out ($(wc -c <"$out") bytes)"
  else
    rm -f "$tmp"
    echo "  FAILED: $url did not return an OpenAPI document" >&2
    failed=1
  fi
done

if [ "$failed" -ne 0 ]; then
  echo
  echo "Some specs could not be downloaded. Record the working URL and version" >&2
  echo "of each spec in specs/README.md once you have it." >&2
  exit 1
fi
echo
echo "Done. Update the version table in specs/README.md, then run: cargo test"
