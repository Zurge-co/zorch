#!/bin/bash

# Configuration
BASE_URL="${BASE_URL:-http://localhost:8081}"
API_KEY="${API_KEY:-sk-zorch-I4NrYo57cx6PHx05RvsWKhTVv4SPJFYo}" # Replace with a valid Zorch API key
MODEL="${MODEL:-chat}"   # Replace with a model configured in your Zorch admin

echo "--- Testing Zorch Model Listing ---"
curl -s -X GET "$BASE_URL/v1/models" \
     -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" | jq .

echo -e "\n\n--- Testing /chat/completions Proxy ---"
curl -s -X POST "$BASE_URL/v1/chat/completions" \
     -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" \
     -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello, are you working?\"}]}"

echo -e "\n\n--- Testing /chat/completions Stream Proxy ---"
curl -s -X POST "$BASE_URL/v1/chat/completions" \
     -H "Authorization: Bearer $API_KEY" \
     -H "Content-Type: application/json" \
     -d "{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"Say hello in one word.\"}],\"stream\":true}"
