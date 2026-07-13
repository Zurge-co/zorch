"use client";

import { useRouter, useParams } from "next/navigation";
import { useState } from "react";
import { PageHeader } from "@/components/PageHeader";
import { ApiKeyForm, ApiKeyFormData } from "@/components/api-keys/ApiKeyForm";
import { ApiKeyMiddlewareBinding } from "@/components/api-keys/ApiKeyMiddlewareBinding";
import { fetchApiKeys, updateApiKey, ApiKey } from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { Layers } from "lucide-react";

function apiKeyToFormDefaults(key: ApiKey): Partial<ApiKeyFormData> {
  return {
    name: key.name,
    tags: [...key.tags],
    allowedHoursStart: key.allowedHoursStart != null ? String(key.allowedHoursStart) : "",
    allowedHoursEnd: key.allowedHoursEnd != null ? String(key.allowedHoursEnd) : "",
    windowTimezone: key.windowTimezone || undefined,
    requestsPerMinute: key.requestsPerMinute != null ? String(key.requestsPerMinute) : "",
    requestsPerDay: key.requestsPerDay != null ? String(key.requestsPerDay) : "",
    maxSpendUsd: key.maxSpendUsd != null ? String(key.maxSpendUsd) : "",
    allowedModels: key.allowedModels?.join(", ") ?? "",
  };
}

export default function EditApiKeyPage() {
  const params = useParams();
  const id = params.id as string;
  const router = useRouter();
  const { data: apiKeys, loading, error } = useFetchData(fetchApiKeys);
  const key = apiKeys?.find((k) => k.id === id) ?? null;
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (data: ApiKeyFormData) => {
    if (!key) return;
    if (!data.name.trim()) {
      toast.error("Key name is required");
      return;
    }

    const payload: Parameters<typeof updateApiKey>[1] = {
      name: data.name.trim(),
      tags: data.tags,
    };
    if (data.allowedHoursStart.trim() && data.allowedHoursEnd.trim()) {
      payload.allowedHoursStart = parseInt(data.allowedHoursStart, 10);
      payload.allowedHoursEnd = parseInt(data.allowedHoursEnd, 10);
      payload.windowTimezone = data.windowTimezone;
    } else {
      payload.allowedHoursStart = null;
      payload.allowedHoursEnd = null;
      payload.windowTimezone = null;
    }
    if (data.requestsPerMinute.trim()) payload.requestsPerMinute = parseInt(data.requestsPerMinute, 10);
    else payload.requestsPerMinute = null;
    if (data.requestsPerDay.trim()) payload.requestsPerDay = parseInt(data.requestsPerDay, 10);
    else payload.requestsPerDay = null;
    if (data.maxSpendUsd.trim()) payload.maxSpendUsd = parseFloat(data.maxSpendUsd);
    else payload.maxSpendUsd = null;
    if (data.allowedModels.trim()) {
      payload.allowedModels = data.allowedModels.split(",").map((m) => m.trim()).filter(Boolean);
    } else {
      payload.allowedModels = null;
    }

    setSaving(true);
    try {
      await updateApiKey(id, payload);
      toast.success("API key updated");
      router.push("/api-keys");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update API key");
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit API Key" description="Loading API key..." />
        <LoadingSpinner />
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit API Key" description="Failed to load API key" />
        <ErrorMessage message={error} />
        <Button variant="outline" onClick={() => router.push("/api-keys")}>
          Back to API Keys
        </Button>
      </div>
    );
  }

  if (!key) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit API Key" description="API key not found" />
        <ErrorMessage message="The requested API key does not exist." />
        <Button variant="outline" onClick={() => router.push("/api-keys")}>
          Back to API Keys
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={`Edit API Key: ${key.name}`}
        description="Update name, tags, and access window for this key."
      />
      <ApiKeyForm
        mode="edit"
        defaultValues={apiKeyToFormDefaults(key)}
        onSubmit={handleSubmit}
        onCancel={() => router.push("/api-keys")}
        submitLabel="Save Changes"
        loading={saving}
      />

      <div className="border rounded-md p-6 space-y-4">
        <div className="flex items-center gap-2">
          <Layers size={18} />
          <h2 className="text-lg font-semibold">Middleware Scripts</h2>
        </div>
        <p className="text-sm text-muted-foreground">
          Assign middleware scripts to this API key. Only assigned scripts will run for requests using this key.
        </p>
        <ApiKeyMiddlewareBinding apiKeyId={id} />
      </div>
    </div>
  );
}
