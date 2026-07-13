"use client";

import React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { EmptyState } from "@/components/ui/empty-state";
import { toast } from "@/components/ui/toast";
import { useFetchData } from "@/lib/useFetchData";
import {
  fetchProviderTargetModels,
  createProviderTargetModel,
  deleteProviderTargetModel,
  syncProviderTargetModels,
  Provider,
  ProviderTargetModel,
} from "@/lib/api";
import { Layers, Plus, RefreshCw, Trash2 } from "lucide-react";

interface ProviderTargetsSectionProps {
  provider: Provider;
}

export function ProviderTargetsSection({ provider }: ProviderTargetsSectionProps) {
  const { data: targetModels, loading, error, refetch } = useFetchData<ProviderTargetModel[]>(
    () => fetchProviderTargetModels(provider.id)
  );
  const [newModel, setNewModel] = React.useState("");
  const [adding, setAdding] = React.useState(false);
  const [syncing, setSyncing] = React.useState(false);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newModel.trim()) return;
    setAdding(true);
    try {
      await createProviderTargetModel(provider.id, { targetModel: newModel.trim(), isActive: true });
      setNewModel("");
      refetch();
      toast.success("Target model added");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to add target model");
    } finally {
      setAdding(false);
    }
  };

  const handleDelete = async (tm: ProviderTargetModel) => {
    if (!window.confirm(`Delete target model "${tm.targetModel}"?`)) return;
    try {
      await deleteProviderTargetModel(provider.id, tm.id);
      refetch();
      toast.success("Target model deleted");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete target model");
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      const res = await syncProviderTargetModels(provider.id);
      refetch();
      toast.success(`Synced upstream models${res.added.length ? `: ${res.added.join(", ")}` : ""}`);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to sync target models");
    } finally {
      setSyncing(false);
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Layers size={18} className="text-muted-foreground" />
          <h2 className="text-lg font-semibold">Target Models</h2>
          <Badge variant="secondary">{targetModels?.length ?? 0}</Badge>
        </div>
        <Button type="button" variant="outline" size="sm" onClick={handleSync} disabled={syncing}>
          <RefreshCw size={14} className={syncing ? "animate-spin mr-2" : "mr-2"} />
          Sync from upstream
        </Button>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      <form onSubmit={handleAdd} className="flex items-center gap-2 max-w-md">
        <Input
          value={newModel}
          onChange={(e) => setNewModel(e.target.value)}
          placeholder="model-name"
          disabled={adding}
        />
        <Button type="submit" disabled={adding || !newModel.trim()}>
          <Plus size={16} className="mr-1" />
          Add
        </Button>
      </form>

      {loading ? (
        <LoadingSpinner />
      ) : targetModels && targetModels.length > 0 ? (
        <div className="border rounded-md divide-y">
          {targetModels.map((tm) => (
            <div key={tm.id} className="flex items-center justify-between px-4 py-3 gap-4">
              <span className="font-mono text-sm truncate" title={tm.targetModel}>
                {tm.targetModel}
              </span>
              <div className="flex items-center gap-2 shrink-0">
                <Badge variant={tm.isActive ? "default" : "secondary"}>{tm.isActive ? "Active" : "Inactive"}</Badge>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => handleDelete(tm)}
                  className="text-muted-foreground hover:text-destructive"
                >
                  <Trash2 size={14} />
                </Button>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <EmptyState
          variant="compact"
          icon={<Layers size={20} />}
          title="No target models configured"
          description="Add a target model or sync from upstream."
        />
      )}
    </section>
  );
}
