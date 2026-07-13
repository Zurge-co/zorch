async function readErrorBody(response: Response): Promise<string> {
  try {
    const text = await response.text();
    if (!text) return response.statusText;
    // Backend AppError responses are plain strings like "Provider error: ..."
    return text.replace(/^["']|["']$/g, "");
  } catch {
    return response.statusText;
  }
}

async function handleFetch<T>(url: string, options?: RequestInit): Promise<T> {
  try {
    const response = await fetch(url, {
      headers: {
        "Content-Type": "application/json",
      },
      ...options,
    });

    if (!response.ok) {
      const body = await readErrorBody(response);
      throw new Error(`HTTP ${response.status}: ${body || response.statusText}`);
    }

    return response.json() as Promise<T>;
  } catch (error) {
    console.error(`API fetch failed for ${url}:`, error);
    throw error;
  }
}

async function handleMutation<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    headers: {
      "Content-Type": "application/json",
    },
    ...options,
  });

  if (!response.ok) {
    const body = await readErrorBody(response);
    throw new Error(`HTTP ${response.status}: ${body || response.statusText}`);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

/**
 * Generic list fetcher. Errors are propagated so callers can show real
 * failure states instead of silently displaying mock/empty data.
 */
async function fetchList<T>(
  path: string,
  options?: { key?: string }
): Promise<T[]> {
  if (options?.key) {
    const data = await handleFetch<Record<string, T[]>>(path);
    return data[options.key] ?? [];
  }
  const data = await handleFetch<T[]>(path);
  return data ?? [];
}

async function fetchObject<T>(
  path: string,
  options?: { key?: string }
): Promise<T> {
  if (options?.key) {
    const data = await handleFetch<Record<string, T>>(path);
    const value = data[options.key];
    if (value === undefined) {
      throw new Error(`Missing key '${options.key}' in response from ${path}`);
    }
    return value as T;
  }
  return await handleFetch<T>(path);
}

export interface RecentActivity {
  id: string;
  keyId: string;
  provider: string;
  model: string;
  status: "success" | "error" | "pending";
  timestamp: string;
  latency: string;
}

export interface DashboardStats {
  requestsPerMinute: string;
  tokensPerMinute: string;
  errorRate: string;
  activeProviders: number;
  requestsTrendPercent: number;
  tokensTrendPercent: number;
  errorRateTrendPercent: number;
  requestsLast24h: number;
  tokensLast24h: number;
  errorRequestsLast24h: number;
}

export interface GatewayConfig {
  appPort: number;
  timeoutSecs: number;
  circuitBreakerTimeoutSecs: number;
  rustLog: string;
  inspectorCaptureLevel: string;
}

export interface ApiKey {
  id: string;
  name: string;
  key: string;
  status: "active" | "revoked" | "inactive";
  createdAt: string;
  usage: string;
  tags: ApiKeyTag[];
  allowedHoursStart: number | null;
  allowedHoursEnd: number | null;
  windowTimezone: string | null;
  requestsPerMinute: number | null;
  requestsPerDay: number | null;
  maxSpendUsd: number | null;
  allowedModels: string[] | null;
}

export interface ApiKeyTag {
  key: string;
  value: string;
}

export interface Provider {
  id: string;
  name: string;
  baseUrl: string;
  authType: ProviderAuthType;
  authHeaderName?: string;
  authPrefix?: string;
  status: string;
  models: string[];
  latency: string;
  costPer1m: string;
}

export type ProviderAuthType = "bearer" | "anthropic" | "custom";

export interface ProviderTargetModel {
  id: string;
  providerId: string;
  targetModel: string;
  isActive: boolean;
  createdAt: string;
}

export interface ProviderApiKey {
  id: string;
  providerId: string;
  label?: string;
  maskedKey: string;
  priority: number;
  isActive: boolean;
  createdAt: string;
}

export interface TokenUsagePoint {
  name: string;
  usage: number;
}

export interface CostTrendPoint {
  name: string;
  cost: number;
}

export interface LatencyPoint {
  name: string;
  value: number;
}

export interface LatencyBreakdown {
  avgLatencyMs: number;
  avgProviderLatencyMs: number;
  avgGatewayLatencyMs: number;
  percentileLatencyMs: LatencyPoint[];
}

export interface AnalyticsData {
  tokenUsage: TokenUsagePoint[];
  costTrends: CostTrendPoint[];
  latencyBreakdown: LatencyBreakdown;
  errorRate: number;
  totalRequests24h: number;
  errorRequests24h: number;
}

export interface TagAnalyticsEntry {
  tag: string;
  requests: number;
  inputTokens: number;
  outputTokens: number;
  costCents: number;
  errorRate: number;
}

export interface TagAnalyticsData {
  tags: TagAnalyticsEntry[];
}

export async function fetchDashboardStats(): Promise<{
  stats: DashboardStats;
  recentActivity: RecentActivity[];
}> {
  return fetchObject("/api/v1/admin/dashboard");
}

export async function fetchGatewayConfig(): Promise<GatewayConfig> {
  return fetchObject<GatewayConfig>("/api/v1/admin/config");
}

export async function fetchApiKeys(): Promise<ApiKey[]> {
  return fetchList<ApiKey>("/api/v1/admin/api-keys", { key: "keys" });
}

export async function createApiKey(payload: {
  name: string;
  scopes?: string[];
  expiresInDays?: number;
  tags?: ApiKeyTag[];
  allowedHoursStart?: number;
  allowedHoursEnd?: number;
  windowTimezone?: string;
  requestsPerMinute?: number;
  requestsPerDay?: number;
  maxSpendUsd?: number;
  allowedModels?: string[];
}): Promise<{ key: string; hash: string; message: string }> {
  return handleMutation(`/api/v1/admin/api-keys`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function revokeApiKey(id: string): Promise<void> {
  return handleMutation(`/api/v1/admin/api-keys/${id}`, {
    method: "DELETE",
  });
}

export async function updateApiKey(
  id: string,
  payload: {
    name?: string;
    scopes?: string[];
    tags?: ApiKeyTag[];
    allowedHoursStart?: number | null;
    allowedHoursEnd?: number | null;
    windowTimezone?: string | null;
    requestsPerMinute?: number | null;
    requestsPerDay?: number | null;
    maxSpendUsd?: number | null;
    allowedModels?: string[] | null;
  }
): Promise<ApiKey> {
  return handleMutation(`/api/v1/admin/api-keys/${id}`, {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export async function replaceApiKeyTags(
  id: string,
  tags: ApiKeyTag[]
): Promise<ApiKey> {
  return handleMutation(`/api/v1/admin/api-keys/${id}/tags`, {
    method: "PUT",
    body: JSON.stringify(tags),
  });
}

export async function fetchProviders(): Promise<Provider[]> {
  return fetchList<Provider>("/api/v1/admin/providers", { key: "providers" });
}

export async function createProvider(payload: {
  name: string;
  baseUrl: string;
  authType: ProviderAuthType;
  authHeaderName?: string;
  authPrefix?: string;
  isActive?: boolean;
}): Promise<{ id: string; message: string }> {
  return handleMutation(`/api/v1/admin/providers`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function updateProvider(
  id: string,
  payload: {
    name: string;
    baseUrl: string;
    authType: ProviderAuthType;
    authHeaderName?: string;
    authPrefix?: string;
    isActive: boolean;
  }
): Promise<void> {
  return handleMutation(`/api/v1/admin/providers/${id}`, {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export async function fetchProviderTargetModels(providerId: string): Promise<ProviderTargetModel[]> {
  return fetchList<ProviderTargetModel>(`/api/v1/admin/providers/${providerId}/target-models`, { key: "targetModels" });
}

export async function createProviderTargetModel(
  providerId: string,
  payload: { targetModel: string; isActive?: boolean }
): Promise<{ id: string; message: string }> {
  return handleMutation(`/api/v1/admin/providers/${providerId}/target-models`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function deleteProviderTargetModel(providerId: string, tmId: string): Promise<void> {
  return handleMutation(`/api/v1/admin/providers/${providerId}/target-models/${tmId}`, {
    method: "DELETE",
  });
}

export async function syncProviderTargetModels(providerId: string): Promise<{ added: string[] }> {
  return handleMutation(`/api/v1/admin/providers/${providerId}/target-models/sync`, {
    method: "POST",
  });
}

export async function fetchProviderApiKeys(providerId: string): Promise<ProviderApiKey[]> {
  return fetchList<ProviderApiKey>(`/api/v1/admin/providers/${providerId}/api-keys`, { key: "apiKeys" });
}

export async function createProviderApiKey(
  providerId: string,
  payload: { label?: string; apiKey: string; priority?: number; isActive?: boolean }
): Promise<{ id: string; message: string }> {
  return handleMutation(`/api/v1/admin/providers/${providerId}/api-keys`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function deleteProviderApiKey(providerId: string, keyId: string): Promise<void> {
  return handleMutation(`/api/v1/admin/providers/${providerId}/api-keys/${keyId}`, {
    method: "DELETE",
  });
}

export async function setProviderApiKeyActive(
  providerId: string,
  keyId: string,
  isActive: boolean
): Promise<void> {
  return handleMutation(`/api/v1/admin/providers/${providerId}/api-keys/${keyId}/active`, {
    method: "PUT",
    body: JSON.stringify({ isActive }),
  });
}

export async function deleteProvider(id: string): Promise<void> {
  return handleMutation(`/api/v1/admin/providers/${id}`, {
    method: "DELETE",
  });
}

export async function toggleProviderActive(id: string, isActive: boolean): Promise<void> {
  return handleMutation(`/api/v1/admin/providers/${id}/active`, {
    method: "POST",
    body: JSON.stringify({ isActive }),
  });
}

export async function fetchModels(): Promise<Model[]> {
  return fetchList<Model>("/api/v1/admin/models", { key: "models" });
}

export async function createModel(payload: {
  publicName: string;
  isActive?: boolean;
}): Promise<{ id: string; message: string }> {
  return handleMutation(`/api/v1/admin/models`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function updateModel(
  id: string,
  payload: { publicName: string; isActive: boolean }
): Promise<void> {
  return handleMutation(`/api/v1/admin/models/${id}`, {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export async function deleteModel(id: string): Promise<void> {
  return handleMutation(`/api/v1/admin/models/${id}`, {
    method: "DELETE",
  });
}

export async function fetchModelTargets(modelId: string): Promise<ModelTarget[]> {
  return fetchList<ModelTarget>(`/api/v1/admin/models/${modelId}/targets`, { key: "targets" });
}

export async function fetchProviderTargets(providerId: string): Promise<ModelTarget[]> {
  return fetchList<ModelTarget>(`/api/v1/admin/providers/${providerId}/targets`, { key: "targets" });
}

export async function createModelTarget(
  modelId: string,
  payload: {
    providerTargetModelId: string;
    priority?: number;
    isActive?: boolean;
  }
): Promise<{ id: string; message: string }> {
  return handleMutation(`/api/v1/admin/models/${modelId}/targets`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function updateModelTarget(
  modelId: string,
  targetId: string,
  payload: {
    providerTargetModelId: string;
    priority: number;
    isActive: boolean;
  }
): Promise<void> {
  return handleMutation(`/api/v1/admin/models/${modelId}/targets/${targetId}`, {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export async function deleteModelTarget(modelId: string, targetId: string): Promise<void> {
  return handleMutation(`/api/v1/admin/models/${modelId}/targets/${targetId}`, {
    method: "DELETE",
  });
}

export interface PricingEntry {
  id: string;
  providerId: string;
  provider: string;
  model: string;
  inputCostPer1m: number;
  outputCostPer1m: number;
  markupPercent: number;
  maxContextTokens: number;  // 0 = unspecified
  createdAt: string;
  updatedAt: string;
}

export interface Model {
  id: string;
  publicName: string;
  isActive: boolean;
  createdAt: string;
}

export interface ModelTarget {
  id: string;
  modelId: string;
  providerTargetModelId: string;
  providerId: string;
  providerName: string;
  targetModel: string;
  priority: number;
  isActive: boolean;
}

export async function fetchAnalytics(tag?: string, range?: string): Promise<AnalyticsData> {
  const params = new URLSearchParams();
  if (tag) params.set("tag", tag);
  if (range) params.set("range", range);
  const qs = params.toString();
  const path = qs ? `/api/v1/admin/analytics?${qs}` : "/api/v1/admin/analytics";
  return fetchObject<AnalyticsData>(path);
}

export async function fetchTagAnalytics(range?: string): Promise<TagAnalyticsData> {
  const params = new URLSearchParams();
  if (range) params.set("range", range);
  const qs = params.toString();
  const path = qs ? `/api/v1/admin/analytics/by-tag?${qs}` : "/api/v1/admin/analytics/by-tag";
  return fetchObject<TagAnalyticsData>(path);
}

export async function fetchPricing(): Promise<PricingEntry[]> {
  return fetchList<PricingEntry>("/api/v1/admin/pricing");
}

export async function setPricing(payload: {
  provider: string;
  model: string;
  inputCostPer1m: number;
  outputCostPer1m: number;
  markupPercent: number;
  maxContextTokens?: number;
}): Promise<{ id: string; message: string }> {
  return handleMutation(`/api/v1/admin/pricing`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function deletePricing(id: string): Promise<void> {
  return handleMutation(`/api/v1/admin/pricing/${id}`, {
    method: "DELETE",
  });
}

export async function previewProviderModels(payload: {
  baseUrl: string;
  authType: ProviderAuthType;
  authHeaderName?: string;
  authPrefix?: string;
  apiKey?: string;
}): Promise<{ models: string[] }> {
  return handleMutation(`/api/v1/admin/providers/preview-models`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
