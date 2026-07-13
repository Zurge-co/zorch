"use client";

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { toast } from "@/components/ui/toast";
import { Layers, Plus, Trash2 } from "lucide-react";

interface MiddlewareConfig {
  id: string;
  name: string;
  phase: string;
  priority: number;
}

interface ApiKeyMiddlewareConfig {
  id: string;
  name: string;
  phase: string;
  priority: number;
}

interface ApiKeyMiddlewareBindingProps {
  apiKeyId: string;
}

const PHASE_LABELS: Record<string, string> = {
  "request.pre_governance": "Pre-Governance",
  "request.pre_upstream": "Pre-Upstream",
};

export function ApiKeyMiddlewareBinding({ apiKeyId }: ApiKeyMiddlewareBindingProps) {
  const [allConfigs, setAllConfigs] = useState<MiddlewareConfig[] | null>(null);
  const [assigned, setAssigned] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    try {
      const [allRes, assignedRes] = await Promise.all([
        fetch("/api/v1/admin/middleware/configs"),
        fetch(`/api/v1/admin/api-keys/${apiKeyId}/middleware-configs`),
      ]);
      if (!allRes.ok) throw new Error("Failed to load middleware configs");
      if (!assignedRes.ok) throw new Error("Failed to load assigned middleware configs");

      const allData = (await allRes.json()) as MiddlewareConfig[];
      const assignedData = (await assignedRes.json()) as ApiKeyMiddlewareConfig[];

      setAllConfigs(allData);
      setAssigned(new Set(assignedData.map((c) => c.id)));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load middleware configs");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, [apiKeyId]);

  const handleAssign = async (configId: string) => {
    setSaving(configId);
    try {
      const res = await fetch(
        `/api/v1/admin/api-keys/${apiKeyId}/middleware-configs/${configId}`,
        { method: "POST" }
      );
      if (!res.ok) throw new Error("Failed to assign middleware config");
      toast.success("Middleware config assigned");
      setAssigned((prev) => new Set(prev).add(configId));
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to assign middleware config");
    } finally {
      setSaving(null);
    }
  };

  const handleUnassign = async (configId: string) => {
    setSaving(configId);
    try {
      const res = await fetch(
        `/api/v1/admin/api-keys/${apiKeyId}/middleware-configs/${configId}`,
        { method: "DELETE" }
      );
      if (!res.ok) throw new Error("Failed to unassign middleware config");
      toast.success("Middleware config unassigned");
      setAssigned((prev) => {
        const next = new Set(prev);
        next.delete(configId);
        return next;
      });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to unassign middleware config");
    } finally {
      setSaving(null);
    }
  };

  if (loading) return <LoadingSpinner />;
  if (error) return <ErrorMessage message={error} />;
  if (!allConfigs) return null;

  return (
    <div className="space-y-4">
      {allConfigs.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No middleware configs available.{" "}
          <a href="/middleware" className="underline">
            Create one first.
          </a>
        </p>
      ) : (
        <div className="space-y-2">
          {allConfigs.map((config) => {
            const isAssigned = assigned.has(config.id);
            return (
              <div
                key={config.id}
                className="flex items-center justify-between border rounded-md p-3"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{config.name || "Unnamed"}</span>
                    {isAssigned && <Badge variant="success">Assigned</Badge>}
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {PHASE_LABELS[config.phase] ?? config.phase} · Priority {config.priority}
                  </p>
                </div>
                <Button
                  variant={isAssigned ? "outline" : "default"}
                  size="sm"
                  disabled={saving === config.id}
                  onClick={() =>
                    isAssigned ? handleUnassign(config.id) : handleAssign(config.id)
                  }
                >
                  {isAssigned ? (
                    <>
                      <Trash2 size={14} className="mr-1" />
                      Unassign
                    </>
                  ) : (
                    <>
                      <Plus size={14} className="mr-1" />
                      Assign
                    </>
                  )}
                </Button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
