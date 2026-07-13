"use client";

import { useRouter, useParams } from "next/navigation";
import { useState, useEffect } from "react";
import { PageHeader } from "@/components/PageHeader";
import { MiddlewareForm, MiddlewareFormData, RunPayload, RunResult } from "@/components/middleware/MiddlewareForm";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";

interface MiddlewareConfig {
  id: string;
  name: string;
  enabled: boolean;
  phase: string;
  priority: number;
  failureMode: string;
  config: {
    source?: string;
    max_operations?: number;
    max_string_size?: number;
    max_array_size?: number;
    max_map_size?: number;
    max_call_stack_depth?: number;
  };
}

export default function EditMiddlewarePage() {
  const params = useParams();
  const id = params.id as string;
  const router = useRouter();
  const [config, setConfig] = useState<MiddlewareConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    fetch(`/api/v1/admin/middleware/configs/${id}`)
      .then((r) => {
        if (!r.ok) throw new Error("Config not found");
        return r.json();
      })
      .then((c) => setConfig(c as MiddlewareConfig))
      .catch((err) => setError(err instanceof Error ? err.message : "Failed to load config"))
      .finally(() => setLoading(false));
  }, [id]);

  const handleValidate = async (source: string) => {
    const res = await fetch("/api/v1/admin/middleware/validate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ source }),
    });
    const data = await res.json().catch(() => ({ valid: false, error: "Validation failed" }));
    return { valid: data.valid, error: data.error };
  };

  const handleRun = async (payload: RunPayload): Promise<RunResult> => {
    const context = JSON.parse(payload.contextJson);
    const input = JSON.parse(payload.inputJson);
    const extraConfig = JSON.parse(payload.extraConfigJson);

    const res = await fetch("/api/v1/admin/middleware/run", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        source: payload.source,
        config: {
          maxOperations: payload.maxOperations,
          maxStringSize: payload.maxStringSize,
          maxArraySize: payload.maxArraySize,
          maxMapSize: payload.maxMapSize,
          maxCallStackDepth: payload.maxCallStackDepth,
          ...extraConfig,
        },
        context,
        input,
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.message || "Run request failed");
    }

    const data = await res.json();
    return {
      success: data.success,
      action: data.action,
      body: data.body,
      headers: data.headers,
      metadata: data.metadata,
      bodyChanged: data.bodyChanged,
      message: data.message,
      statusCode: data.statusCode,
      error: data.error,
      durationMs: data.durationMs,
    };
  };

  const handleSubmit = async (data: MiddlewareFormData) => {
    setSaving(true);
    try {
      const res = await fetch(`/api/v1/admin/middleware/configs/${id}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: data.name,
          enabled: data.enabled,
          phase: data.phase,
          priority: data.priority,
          failureMode: data.failureMode,
          config: {
            source: data.source,
            max_operations: data.maxOperations,
            max_string_size: data.maxStringSize,
            max_array_size: data.maxArraySize,
            max_map_size: data.maxMapSize,
            max_call_stack_depth: data.maxCallStackDepth,
          },
        }),
      });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || "Failed to update config");
      }
      toast.success("Middleware config updated");
      router.push("/middleware");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update config");
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader title="Update Middleware Config" description="Loading configuration..." />
        <LoadingSpinner />
      </div>
    );
  }

  if (error || !config) {
    return (
      <div className="space-y-8">
        <PageHeader title="Update Middleware Config" description="Failed to load configuration" />
        <ErrorMessage message={error || "Config not found"} />
        <Button variant="outline" onClick={() => router.push("/middleware")}>
          Back
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={`Update Middleware Config: ${config.name || "Unnamed"}`}
        description="Edit Rhai middleware configuration."
      />
      <MiddlewareForm
        defaultValues={{
          name: config.name,
          phase: config.phase,
          priority: config.priority,
          failureMode: config.failureMode,
          enabled: config.enabled,
          source: config.config.source ?? "",
          maxOperations: config.config.max_operations,
          maxStringSize: config.config.max_string_size,
          maxArraySize: config.config.max_array_size,
          maxMapSize: config.config.max_map_size,
          maxCallStackDepth: config.config.max_call_stack_depth,
        }}
        onSubmit={handleSubmit}
        onCancel={() => router.push("/middleware")}
        submitLabel="Update Config"
        loading={saving}
        onValidate={handleValidate}
        onRun={handleRun}
      />
    </div>
  );
}
