"use client";

import { useRouter } from "next/navigation";
import { useState, useEffect } from "react";
import { PageHeader } from "@/components/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { EmptyState } from "@/components/ui/empty-state";
import { toast } from "@/components/ui/toast";
import { Layers, Plus, Trash2, Pencil, Play } from "lucide-react";

interface MiddlewareConfig {
  id: string;
  name: string;
  enabled: boolean;
  phase: string;
  priority: number;
  failureMode: string;
  config: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

const PHASE_LABELS: Record<string, string> = {
  "request.pre_governance": "Pre-Governance",
  "request.pre_upstream": "Pre-Upstream",
};

export default function MiddlewarePage() {
  const router = useRouter();
  const [configs, setConfigs] = useState<MiddlewareConfig[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    setLoading(true);
    fetch("/api/v1/admin/middleware/configs")
      .then((r) => r.json())
      .then((data) => setConfigs(data as MiddlewareConfig[]))
      .catch((err) => setError(err instanceof Error ? err.message : "Failed to load configs"))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, []);

  const handleDelete = async (id: string) => {
    if (!window.confirm("Delete this middleware config?")) return;
    try {
      const res = await fetch(`/api/v1/admin/middleware/configs/${id}`, { method: "DELETE" });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || "Failed to delete config");
      }
      toast.success("Config deleted");
      load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete config");
    }
  };

  return (
    <div className="space-y-8">
      <PageHeader
        title="Middleware"
        description="Configure Rhai middleware scripts and assign them to API keys."
        actions={
          <div className="flex gap-2">
            <Button onClick={() => router.push("/middleware/new")} className="gap-2">
              <Plus size={16} />
              Add Config
            </Button>
            <Button variant="outline" onClick={() => router.push("/middleware/runs")} className="gap-2">
              <Play size={16} />
              Runs
            </Button>
          </div>
        }
      />

      {loading && <LoadingSpinner />}
      {error && <ErrorMessage message={error} />}

      {!loading && !error && (!configs || configs.length === 0) && (
        <EmptyState
          icon={<Layers size={24} />}
          title="No middleware configured"
          description="Add a middleware config to start transforming and blocking requests."
        />
      )}

      {!loading && configs && configs.length > 0 && (
        <div className="space-y-4">
          {configs.map((config) => (
            <div key={config.id} className="border rounded-md p-4 space-y-3">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <div className="flex items-center gap-2">
                    <h2 className="text-lg font-semibold">{config.name || "Unnamed"}</h2>
                    <Badge variant={config.enabled ? "success" : "secondary"}>
                      {config.enabled ? "enabled" : "disabled"}
                    </Badge>
                  </div>
                  <p className="text-sm text-muted-foreground mt-1">
                    Phase: {PHASE_LABELS[config.phase] ?? config.phase} · Priority: {config.priority} · Failure: {config.failureMode}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => router.push(`/middleware/${config.id}/edit`)}
                  >
                    <Pencil size={14} className="mr-1" />
                    Edit
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-destructive"
                    onClick={() => handleDelete(config.id)}
                  >
                    <Trash2 size={14} className="mr-1" />
                    Delete
                  </Button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
