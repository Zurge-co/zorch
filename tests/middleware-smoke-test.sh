#!/bin/bash
set -euo pipefail

# Middleware smoke tests for the Zorch proxy.
#
# Prerequisites:
#   - Zorch API is running at $BASE_URL
#   - An API key exists and is assigned to a middleware config in the admin dashboard
#   - The assigned middleware is enabled and bound to the request phase being tested
#
# Configure via environment variables or edit defaults below.

BASE_URL="${BASE_URL:-http://localhost:8081}"
API_KEY="${API_KEY:-}"  # required
MODEL="${MODEL:-chat}" # public model name configured in Zorch

if [ -z "$API_KEY" ]; then
    echo "Error: API_KEY environment variable is required."
    exit 1
fi

AUTH_HEADER="Authorization: Bearer $API_KEY"

echo "--- Base proxy request (should succeed or be modified by middleware) ---"
curl -s -X POST "$BASE_URL/v1/chat/completions" \
    -H "$AUTH_HEADER" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}" | jq .

echo -e "\n\n--- Test middleware body modification ---"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$BASE_URL/v1/chat/completions" \
    -H "$AUTH_HEADER" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"system\",\"content\":\"You are helpful.\"},{\"role\":\"user\",\"content\":\"Hello\"}]}" | jq .

echo -e "\n\n--- Test middleware block with custom status code ---"
# If your middleware blocks a forbidden model, this request should return the configured status code
# (e.g. 403 or 400) instead of the default 400.
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$BASE_URL/v1/chat/completions" \
    -H "$AUTH_HEADER" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"blocked-model\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}" | jq .

echo -e "\n\n--- Test middleware header passthrough ---"
# Inspect response headers to verify middleware-added headers are not stripped.
curl -s -D - -X POST "$BASE_URL/v1/chat/completions" \
    -H "$AUTH_HEADER" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}" -o /dev/null

echo -e "\n\nMiddleware smoke tests complete."
