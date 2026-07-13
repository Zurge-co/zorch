# Provider Setup

Zorch supports direct connections to AI providers without requiring OpenRouter or LiteLLM intermediaries.

## Provider Structure

A provider in Zorch is a logical upstream endpoint. It consists of:

1. **Provider** — name, base URL, and authentication style.
2. **Target Models** — upstream model names the provider supports, managed in `provider_target_models`.
3. **Target API Keys** — one or more encrypted API keys Zorch uses to call the provider, managed in `provider_api_keys`.

## Supported Auth Types

### Bearer

Use this for any provider that accepts an `Authorization: Bearer` header, including:
- OpenAI
- OpenRouter
- Groq
- Any custom OpenAI-compatible endpoint

**Setup:**
1. Go to **Providers** in the admin dashboard
2. Click **Add Provider**
3. Select **Bearer** as the auth type
4. Set the base URL to the API version root, e.g.:
   - OpenAI: `https://api.openai.com/v1`
   - OpenRouter: `https://openrouter.ai/api/v1`
5. Add one or more target API keys in the **API Keys** section (encrypted at rest)
6. Add target models manually or use **Sync from upstream**

**Auth headers sent upstream:**
```
Authorization: Bearer <key>
Content-Type: application/json
```

### Anthropic

Use this for Anthropic's Claude API.

**Setup:**
1. Go to **Providers** in the admin dashboard
2. Click **Add Provider**
3. Select **Anthropic** as the auth type
4. Set the base URL to: `https://api.anthropic.com/v1`
5. Add one or more target API keys in the **API Keys** section
6. Add target models manually, e.g. `claude-3-5-sonnet-latest`, or sync from upstream

**Auth headers sent upstream:**
```
x-api-key: <key>
anthropic-version: 2023-06-01
Content-Type: application/json
```

### Custom

Use this for providers that expect the key in a custom header or with a custom prefix.

**Setup:**
1. Go to **Providers** in the admin dashboard
2. Click **Add Provider**
3. Select **Custom** as the auth type
4. Fill in **Auth Header Name** (e.g. `X-Api-Key`) and optional **Auth Prefix** (e.g. `Token`)
5. Set the base URL and add API keys as usual

**Example auth header with prefix:**
```
X-Api-Key: Token <key>
Content-Type: application/json
```

## Base URL Convention

The `base_url` must be the API version root (the path that includes `/v1`).

Zorch strips its own `/v1` prefix from incoming gateway paths and appends the remaining path to the configured base URL.

| Auth type | Example base URL | Zorch route | Upstream path |
|-----------|-----------------|-------------|---------------|
| `bearer` | `https://api.openai.com/v1` | `/v1/chat/completions` | `/chat/completions` |
| `anthropic` | `https://api.anthropic.com/v1` | `/v1/messages` | `/messages` |
| `custom` | provider-specific `/v1` root | `/v1/models` | `/models` |

## Target Models

Target models are the upstream identifiers the provider expects in the `model` field. Examples:

- `gpt-4o-mini`
- `gpt-4o`
- `claude-3-5-sonnet-latest`

You can add them manually in the provider's **Target Models** section or click **Sync from upstream** to fetch them from the provider's `/models` endpoint.

## API Keys

Each provider can have multiple target API keys. Zorch will:

- Use sticky routing so repeated requests from the same client API key go to the same target API key (improves upstream cache hits).
- Fall over to the next key if the chosen key fails (network error or 5xx/429).

Keys are encrypted at rest using `ZORCH_ENCRYPTION_KEY`.

## Alias Models

Public model names (aliases) are configured separately in **Models**. An alias maps to a provider plus one of its target models. Client requests use the alias; Zorch rewrites the upstream request to use the target model name.

## Pricing Setup

After creating a provider and its target models, configure per-model pricing in the **Pricing** admin page:
1. Select the provider
2. Enter the target model name
3. Set input cost per 1M tokens
4. Set output cost per 1M tokens
5. Optionally set markup percentage and max context tokens

## Fallback Keys

Environment variables `ZORCH_OPENAI_API_KEY` and `ZORCH_ANTHROPIC_API_KEY` are used as fallback keys when no database providers are configured. In production, always configure providers through the dashboard so keys are encrypted at rest.
