"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { PageHeader } from "@/components/PageHeader";
import { ApiKeyForm, ApiKeyFormData } from "@/components/api-keys/ApiKeyForm";
import { createApiKey } from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Copy, Check } from "lucide-react";

export default function NewApiKeyPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const handleSubmit = async (data: ApiKeyFormData) => {
    if (!data.name) {
      toast.error("Key name is required");
      return;
    }

    const payload: Parameters<typeof createApiKey>[0] = { name: data.name };
    if (data.scopes) {
      payload.scopes = data.scopes.split(",").map((s) => s.trim()).filter(Boolean);
    }
    if (data.expiresInDays) {
      const days = parseInt(data.expiresInDays, 10);
      if (Number.isFinite(days) && days > 0) payload.expiresInDays = days;
    }
    if (data.tags.length > 0) payload.tags = data.tags;
    if (data.allowedHoursStart && data.allowedHoursEnd) {
      payload.allowedHoursStart = parseInt(data.allowedHoursStart, 10);
      payload.allowedHoursEnd = parseInt(data.allowedHoursEnd, 10);
      payload.windowTimezone = data.windowTimezone;
    }
    if (data.requestsPerMinute) payload.requestsPerMinute = parseInt(data.requestsPerMinute, 10);
    if (data.requestsPerDay) payload.requestsPerDay = parseInt(data.requestsPerDay, 10);
    if (data.maxSpendUsd) payload.maxSpendUsd = parseFloat(data.maxSpendUsd);
    if (data.allowedModels) {
      payload.allowedModels = data.allowedModels.split(",").map((m) => m.trim()).filter(Boolean);
    }

    setLoading(true);
    try {
      const res = await createApiKey(payload);
      setCreatedKey(res.key);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create API key");
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = async (text: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (createdKey) {
    return (
      <div className="space-y-8 max-w-3xl">
        <PageHeader
          title="API Key Created"
          description="Copy this key now. You won't be able to see it again."
        />
        <div className="border rounded-md p-4 space-y-4">
          <div className="space-y-2">
            <Label>Your new API key</Label>
            <div className="flex items-center gap-2">
              <code className="flex-1 break-all text-sm font-mono bg-muted p-2 rounded">{createdKey}</code>
              <Button size="sm" variant="outline" onClick={() => copyToClipboard(createdKey)}>
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </Button>
            </div>
          </div>
          <Button onClick={() => router.push("/api-keys")}>Done</Button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title="Create New API Key"
        description="Generate a new API key for accessing the Zorch platform."
      />
      <ApiKeyForm
        mode="create"
        onSubmit={handleSubmit}
        onCancel={() => router.push("/api-keys")}
        submitLabel="Create Key"
        loading={loading}
      />
    </div>
  );
}
