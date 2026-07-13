"use client";

import { useRouter, useParams } from "next/navigation";
import React from "react";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  fetchModels,
  fetchProviders,
  fetchProviderTargetModels,
  fetchModelTargets,
  createModelTarget,
  updateModelTarget,
  deleteModelTarget,
  Provider,
  ProviderTargetModel,
  ModelTarget,
  Model,
} from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { toast } from "@/components/ui/toast";
import { ArrowLeft, Layers, Plus, Trash2 } from "lucide-react";

export default function ModelTargetsPage() {
  const params = useParams();
  const id = params.id as string;
  const router = useRouter();

  const { data: models, loading: modelsLoading, error: modelsError } = useFetchData(fetchModels);
  const { data: providers } = useFetchData(fetchProviders);
  const {
    data: targets,
    loading: targetsLoading,
    error: targetsError,
    refetch: refetchTargets,
  } = useFetchData(() => fetchModelTargets(id));

  const model = models?.find((m) => m.id === id) ?? null;

  const [providerId, setProviderId] = React.useState("");
  const [providerTargetModelId, setProviderTargetModelId] = React.useState("");
  const [priority, setPriority] = React.useState("0");
  const [isActive, setIsActive] = React.useState(true);
  const [creating, setCreating] = React.useState(false);

  const selectedProvider = React.useMemo(
    () => providers?.find((p) => p.id === providerId) ?? null,
    [providers, providerId]
  );

  const {
    data: providerTargetModels,
    loading: loadingTargetModels,
    refetch: refetchProviderTargetModels,
  } = useFetchData<ProviderTargetModel[]>(
    React.useCallback(
      () => (selectedProvider ? fetchProviderTargetModels(selectedProvider.id) : Promise.resolve([])),
      [selectedProvider?.id]
    )
  );

  const resetForm = () => {
    setProviderId("");
    setProviderTargetModelId("");
    setPriority("0");
    setIsActive(true);
  };

  // Refetch target models from DB whenever the selected provider changes.
  React.useEffect(() => {
    if (selectedProvider) {
      refetchProviderTargetModels();
    }
  }, [selectedProvider?.id, refetchProviderTargetModels]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!providerTargetModelId) return;
    setCreating(true);
    try {
      await createModelTarget(id, {
        providerTargetModelId,
        priority: parseInt(priority, 10) || 0,
        isActive,
      });
      resetForm();
      refetchTargets();
      toast.success("Target added");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to add target");
    } finally {
      setCreating(false);
    }
  };

  if (modelsLoading) {
    return (
      <div className="space-y-8">
        <PageHeader title="Manage Targets" description="Loading model..." />
        <LoadingSpinner />
      </div>
    );
  }

  if (modelsError) {
    return (
      <div className="space-y-8">
        <PageHeader title="Manage Targets" description="Failed to load model" />
        <ErrorMessage message={modelsError} />
      </div>
    );
  }

  if (!model) {
    return (
      <div className="space-y-8">
        <PageHeader title="Manage Targets" description="Model not found" />
        <ErrorMessage message="The requested model does not exist." />
      </div>
    );
  }

  return (
    <div className="space-y-8 min-w-0">
      <PageHeader
        title={`Manage Targets: ${model.publicName}`}
        description="Higher priority targets are tried first. Targets with equal priority are shuffled."
        actions={
          <Button variant="outline" onClick={() => router.push("/models")}>
            <ArrowLeft size={16} className="mr-2" />
            Back to Models
          </Button>
        }
      />

      <form onSubmit={handleCreate} className="space-y-4 border rounded-md p-4">
        <div className="flex items-center gap-2">
          <Layers size={18} className="text-muted-foreground" />
          <h2 className="text-lg font-semibold">Add Target</h2>
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-4">
          <div className="space-y-2">
            <Label htmlFor="provider">Provider</Label>
            <select
              id="provider"
              className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
              value={providerId}
              onChange={(e) => {
                setProviderId(e.target.value);
                setProviderTargetModelId("");
              }}
              required
            >
              <option value="">Select provider</option>
              {providers?.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-2">
            <Label htmlFor="targetModel">Target Model</Label>
            <select
              id="targetModel"
              className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm disabled:opacity-50"
              value={providerTargetModelId}
              onChange={(e) => setProviderTargetModelId(e.target.value)}
              disabled={!providerId || loadingTargetModels}
              required
            >
              <option value="">{providerId ? "Select target model" : "Choose a provider first"}</option>
              {(providerTargetModels ?? []).map((tm) => (
                <option key={tm.id} value={tm.id}>
                  {tm.targetModel}
                </option>
              ))}
            </select>
            {providerId && !loadingTargetModels && (!providerTargetModels || providerTargetModels.length === 0) && (
              <p className="text-sm text-muted-foreground">No target models configured for this provider.</p>
            )}
          </div>
          <div className="space-y-2">
            <Label htmlFor="priority">Priority</Label>
            <Input
              id="priority"
              type="number"
              value={priority}
              onChange={(e) => setPriority(e.target.value)}
              required
            />
          </div>
          <div className="flex items-end gap-2 pb-1">
            <Switch id="targetIsActive" checked={isActive} onCheckedChange={setIsActive} />
            <Label htmlFor="targetIsActive">Active</Label>
          </div>
        </div>
        <Button type="submit" disabled={creating || !providerTargetModelId}>
          <Plus size={16} className="mr-2" />
          Add Target
        </Button>
      </form>

      {targetsError && <ErrorMessage message={targetsError} />}

      {targetsLoading ? (
        <LoadingSpinner />
      ) : (
        <div className="border rounded-md overflow-hidden">
          <Table className="table-fixed">
            <TableHeader>
              <TableRow>
                <TableHead className="w-[22%]">Provider</TableHead>
                <TableHead className="w-[33%]">Upstream Model</TableHead>
                <TableHead className="w-[12%]">Priority</TableHead>
                <TableHead className="w-[14%]">Status</TableHead>
                <TableHead className="w-[19%] text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {targets?.map((target) => (
                <TargetRow
                  key={target.id}
                  modelId={id}
                  target={target}
                  providers={providers ?? []}
                  onChange={refetchTargets}
                />
              ))}
              {(!targets || targets.length === 0) && (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground py-8">
                    No targets for this model.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

function TargetRow({
  modelId,
  target,
  providers,
  onChange,
}: {
  modelId: string;
  target: ModelTarget;
  providers: Provider[];
  onChange: () => void;
}) {
  const [editing, setEditing] = React.useState(false);
  const [providerId, setProviderId] = React.useState(target.providerId);
  const [providerTargetModelId, setProviderTargetModelId] = React.useState(target.providerTargetModelId);
  const [priority, setPriority] = React.useState(target.priority.toString());
  const [isActive, setIsActive] = React.useState(target.isActive);
  const [submitting, setSubmitting] = React.useState(false);

  const selectedProvider = React.useMemo(
    () => providers.find((p) => p.id === providerId) ?? null,
    [providers, providerId]
  );

  const { data: providerTargetModels } = useFetchData<ProviderTargetModel[]>(
    React.useCallback(
      () => (selectedProvider ? fetchProviderTargetModels(selectedProvider.id) : Promise.resolve([])),
      [selectedProvider?.id]
    )
  );

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!providerTargetModelId) return;
    setSubmitting(true);
    try {
      await updateModelTarget(modelId, target.id, {
        providerTargetModelId,
        priority: parseInt(priority, 10) || 0,
        isActive,
      });
      setEditing(false);
      onChange();
      toast.success("Target updated");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update target");
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async () => {
    if (!window.confirm(`Delete target "${target.targetModel}"?`)) return;
    try {
      await deleteModelTarget(modelId, target.id);
      onChange();
      toast.success("Target deleted");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete target");
    }
  };

  if (editing) {
    return (
      <TableRow>
        <TableCell>
          <select
            className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm"
            value={providerId}
            onChange={(e) => {
              setProviderId(e.target.value);
              setProviderTargetModelId("");
            }}
            required
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </TableCell>
        <TableCell>
          <select
            className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm disabled:opacity-50"
            value={providerTargetModelId}
            onChange={(e) => setProviderTargetModelId(e.target.value)}
            disabled={!providerId}
            required
          >
            <option value="">Select target model</option>
            {(providerTargetModels ?? []).map((tm) => (
              <option key={tm.id} value={tm.id}>
                {tm.targetModel}
              </option>
            ))}
          </select>
        </TableCell>
        <TableCell>
          <Input type="number" value={priority} onChange={(e) => setPriority(e.target.value)} required />
        </TableCell>
        <TableCell>
          <Switch checked={isActive} onCheckedChange={setIsActive} />
        </TableCell>
        <TableCell className="text-right space-x-2">
          <Button type="button" size="sm" onClick={handleSave} disabled={submitting || !providerTargetModelId}>
            Save
          </Button>
          <Button type="button" variant="ghost" size="sm" onClick={() => setEditing(false)}>
            Cancel
          </Button>
        </TableCell>
      </TableRow>
    );
  }

  return (
    <TableRow>
      <TableCell className="truncate" title={target.providerName}>{target.providerName}</TableCell>
      <TableCell className="font-mono text-sm truncate" title={target.targetModel}>{target.targetModel}</TableCell>
      <TableCell>{target.priority}</TableCell>
      <TableCell>
        {target.isActive ? (
          <Badge variant="default">Active</Badge>
        ) : (
          <Badge variant="secondary">Inactive</Badge>
        )}
      </TableCell>
      <TableCell className="text-right space-x-2">
        <Button variant="ghost" size="sm" onClick={() => setEditing(true)}>
          Edit
        </Button>
        <Button variant="ghost" size="sm" className="text-destructive" onClick={handleDelete}>
          <Trash2 size={14} className="mr-1" />
          Delete
        </Button>
      </TableCell>
    </TableRow>
  );
}
