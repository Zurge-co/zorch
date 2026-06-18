# Provider Setup

Zorch supports direct connections to AI providers without requiring OpenRouter or LiteLLM intermediaries.

## Supported Protocols

### OpenAI-compatible

Use this protocol for any provider that implements the OpenAI API format, including:
- OpenAI
- OpenRouter
- Groq
- Any custom OpenAI-compatible endpoint

**Setup:**
1. Go to **Providers** in the admin dashboard
2. Click **Add Provider**
3. Select **OpenAI-compatible** as the protocol
4. Set the base URL to the API version root, e.g.:
   - OpenAI: `https://api.openai.com/v1`
   - OpenRouter: `https://openrouter.ai/api/v1`
5. Enter your API key (encrypted at rest)
6. Add model tags manually or use **Fetch from /models**

**Auth headers sent upstream:**
```
Authorization: Bearer <key>
Content-Type: application/json
```

### Anthropic

Use this protocol for Anthropic's Claude API.

**Setup:**
1. Go to **Providers** in the admin dashboard
2. Click **Add Provider**
3. Select **Anthropic** as the protocol
4. Set the base URL to: `https://api.anthropic.com/v1`
5. Enter your API key (encrypted at rest)
6. Add model tags manually, e.g. `claude-3-5-sonnet-latest`

**Auth headers sent upstream:**
```
x-api-key: <key>
anthropic-version: 2023-06-01
Content-Type: application/json
```

## Base URL Convention

The `base_url` must be the API version root (the path that includes `/v1`).

Zorch strips its own `/v1` prefix from incoming gateway paths and appends the remaining path to the configured base URL.

| Protocol | Example base URL | Zorch route | Upstream path |
|----------|-----------------|-------------|---------------|
| `openai_compatible` | `https://api.openai.com/v1` | `/v1/chat/completions` | `/chat/completions` |
| `anthropic` | `https://api.anthropic.com/v1` | `/v1/messages` | `/messages` |
| Either | provider-specific `/v1` root | `/v1/models` | `/models` |

## Model Tags

Model tags are the identifiers your application uses in the `model` field of API requests. Examples:

- `gpt-4o-mini`
- `gpt-4o`
- `claude-3-5-sonnet-latest`

You can add tags manually or fetch them from the provider's `/models` endpoint.

## Pricing Setup

After creating a provider, configure per-model pricing in the **Pricing** admin page:
1. Select the provider
2. Enter the model tag
3. Set input cost per 1M tokens
4. Set output cost per 1M tokens
5. Optionally set markup percentage and max context tokens

## Fallback Keys

Environment variables `ZORCH_OPENAI_API_KEY` and `ZORCH_ANTHROPIC_API_KEY` are used as fallback keys when no database providers are configured. In production, always configure providers through the dashboard so keys are encrypted at rest.
