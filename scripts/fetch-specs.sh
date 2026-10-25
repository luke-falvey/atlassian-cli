#!/usr/bin/env bash
# Download the official Atlassian Data Center OpenAPI specs into specs/.
#
# The URLs below are the best-known locations on dac-static.atlassian.com and
# have not all been confirmed (the host was unreachable when this script was
# written). If one fails, find the "Download OpenAPI spec" link on the
# product's REST API page on developer.atlassian.com and pass it in:
#
#   BITBUCKET_SPEC_URL=https://.../9.6.swagger.v3.json scripts/fetch-specs.sh
#
# Usage: scripts/fetch-specs.sh [product ...]   (default: all products)
set -euo pipefail

cd "$(dirname "$0")/.."
mkdir -p specs

declare -A URLS=(
  [jira]="${JIRA_SPEC_URL:-https://dac-static.atlassian.com/server/jira/platform/swagger-v3.v3.json}"
  [jira-software]="${JIRA_SOFTWARE_SPEC_URL:-https://dac-static.atlassian.com/server/jira-software/swagger.v3.json}"
  [confluence]="${CONFLUENCE_SPEC_URL:-https://dac-static.atlassian.com/server/confluence/swagger.v3.json}"
  [bitbucket]="${BITBUCKET_SPEC_URL:-https://dac-static.atlassian.com/server/bitbucket/latest.swagger.v3.json}"
  [bamboo]="${BAMBOO_SPEC_URL:-https://dac-static.atlassian.com/server/bamboo/swagger.v3.json}"
)

products=("$@")
[ ${#products[@]} -eq 0 ] && products=(jira jira-software confluence bitbucket bamboo)

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
