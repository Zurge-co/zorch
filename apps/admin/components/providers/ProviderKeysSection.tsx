"use client";

import React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { EmptyState } from "@/components/ui/empty-state";
import { toast } from "@/components/ui/toast";
import { useFetchData } from "@/lib/useFetchData";
import {
  fetchProviderApiKeys,
  createProviderApiKey,
  deleteProviderApiKey,
  setProviderApiKeyActive,
  Provider,
  ProviderApiKey,
} from "@/lib/api";
import { Key, Plus, Trash2 } from "lucide-react";

interface ProviderKeysSectionProps {
  provider: Provider;
}

export function ProviderKeysSection({ provider }: ProviderKeysSectionProps) {
  const { data: apiKeys, loading, error, refetch } = useFetchData<ProviderApiKey[]>(
    () => fetchProviderApiKeys(provider.id)
  );
  const [newKey, setNewKey] = React.useState("");
  const [newLabel, setNewLabel] = React.useState("");
  const [newPriority, setNewPriority] = React.useState("0");
  const [adding, setAdding] = React.useState(false);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newKey.trim()) return;
    setAdding(true);
    try {
      await createProviderApiKey(provider.id, {
        apiKey: newKey.trim(),
        label: newLabel.trim() || undefined,
        priority: parseInt(newPriority, 10) || 0,
        isActive: true,
      });
      setNewKey("");
      setNewLabel("");
      setNewPriority("0");
      refetch();
      toast.success("API key added");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to add API key");
    } finally {
      setAdding(false);
    }
  };

  const handleDelete = async (key: ProviderApiKey) => {
    if (!window.confirm(`Delete API key ${key.label ? `"${key.label}"` : ""}?`)) return;
    try {
      await deleteProviderApiKey(provider.id, key.id);
      refetch();
      toast.success("API key deleted");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete API key");
    }
  };

  const handleToggle = async (key: ProviderApiKey, next: boolean) => {
    try {
      await setProviderApiKeyActive(provider.id, key.id, next);
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update API key");
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-center gap-2">
        <Key size={18} className="text-muted-foreground" />
        <h2 className="text-lg font-semibold">API Keys</h2>
        <Badge variant="secondary">{apiKeys?.length ?? 0}</Badge>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      <form onSubmit={handleAdd} className="space-y-3 max-w-2xl">
        <div className="grid grid-cols-1 md:grid-cols-[1fr_auto] gap-2">
          <Input
            type="password"
            value={newKey}
            onChange={(e) => setNewKey(e.target.value)}
            placeholder="sk-..."
            disabled={adding}
          />
          <Button type="submit" disabled={adding || !newKey.trim()}>
            <Plus size={16} className="mr-1" />
            Add Key
          </Button>
        </div>
        <div className="grid grid-cols-2 gap-2">
          <Input
            value={newLabel}
            onChange={(e) => setNewLabel(e.target.value)}
            placeholder="Label (optional)"
            disabled={adding}
          />
          <Input
            type="number"
            value={newPriority}
            onChange={(e) => setNewPriority(e.target.value)}
            placeholder="Priority"
            disabled={adding}
          />
        </div>
      </form>

      {loading ? (
        <LoadingSpinner />
      ) : apiKeys && apiKeys.length > 0 ? (
        <div className="border rounded-md divide-y">
          {apiKeys.map((key) => (
            <div key={key.id} className="flex items-center justify-between px-4 py-3 gap-4">
              <div className="min-w-0">
                <p className="text-sm font-medium truncate">{key.label || "Unlabeled"}</p>
                <p className="font-mono text-xs text-muted-foreground truncate">{key.maskedKey}</p>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <Switch
                  checked={key.isActive}
                  onCheckedChange={(next) => handleToggle(key, next)}
                  aria-label={`Toggle ${key.label || "API key"}`}
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => handleDelete(key)}
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
          icon={<Key size={20} />}
          title="No API keys configured"
          description="Add a provider API key above."
        />
      )}
    </section>
  );
}
