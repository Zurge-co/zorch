"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { PageHeader } from "@/components/PageHeader";
import { MiddlewareForm, MiddlewareFormData, RunPayload, RunResult } from "@/components/middleware/MiddlewareForm";
import { toast } from "@/components/ui/toast";

export default function NewMiddlewarePage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);

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
    setLoading(true);
    try {
      const res = await fetch("/api/v1/admin/middleware/configs", {
        method: "POST",
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
        throw new Error(err.message || "Failed to create config");
      }
      toast.success("Middleware config created");
      router.push("/middleware");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create config");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-8">
      <PageHeader
        title="Add Middleware Config"
        description="Configure a Rhai middleware script."
      />
      <MiddlewareForm
        onSubmit={handleSubmit}
        onCancel={() => router.push("/middleware")}
        submitLabel="Save Config"
        loading={loading}
        onValidate={handleValidate}
        onRun={handleRun}
      />
    </div>
  );
}
