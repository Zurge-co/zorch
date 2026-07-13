"use client";

import * as React from "react";
import { DollarSign, Download, Loader2, Plus, Save, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import {
  deletePricing,
  fetchPricing,
  fetchProviderTargets,
  setPricing,
  previewProviderModels,
  type ModelTarget,
  type PricingEntry,
  type Provider,
} from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { toast } from "@/components/ui/toast";

interface ModelPricingSectionProps {
  provider: Provider;
}

export function ModelPricingSection({ provider }: ModelPricingSectionProps) {
  const { data: allPricing, loading, error, refetch } = useFetchData<PricingEntry[]>(fetchPricing);
  const { data: targets, loading: targetsLoading } = useFetchData<ModelTarget[]>(() =>
    fetchProviderTargets(provider.id)
  );

  const pricing = React.useMemo(
    () => (allPricing ?? []).filter((e) => e.provider === provider.name),
    [allPricing, provider.name]
  );

  const pricedModels = React.useMemo(() => new Set(pricing.map((e) => e.model)), [pricing]);
  const unpricedTargets = React.useMemo(
    () => (targets ?? []).filter((t) => t.isActive && !pricedModels.has(t.targetModel)),
    [targets, pricedModels]
  );

  const [syncExpanded, setSyncExpanded] = React.useState(false);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <DollarSign size={18} className="text-muted-foreground" />
          <h2 className="text-lg font-semibold">Model Pricing</h2>
          <Badge variant="secondary">{pricing.length}</Badge>
        </div>
        <Button type="button" variant="outline" size="sm" onClick={() => setSyncExpanded((v) => !v)}>
          <Download size={14} className="mr-2" />
          {syncExpanded ? "Hide Sync" : "Sync from upstream"}
        </Button>
      </div>

      {error && <p className="text-sm text-destructive">{error}</p>}

      {loading ? (
        <LoadingSpinner />
      ) : pricing.length > 0 ? (
        <div className="border rounded-md divide-y">
          {pricing.map((entry) => (
            <ModelPricingRow key={entry.id} entry={entry} onChanged={refetch} />
          ))}
        </div>
      ) : (
        <EmptyState
          variant="compact"
          icon={<DollarSign size={20} />}
          title="No pricing configured"
          description="Add pricing for unpriced targets or sync from upstream."
        />
      )}

      {unpricedTargets.length > 0 && (
        <div className="space-y-2">
          <h3 className="text-sm font-medium text-muted-foreground">Unpriced target models ({unpricedTargets.length})</h3>
          <div className="border border-dashed rounded-md divide-y">
            {unpricedTargets.map((target) => (
              <UnpricedTargetRow key={target.id} providerName={provider.name} target={target} onChanged={refetch} />
            ))}
          </div>
        </div>
      )}

      {syncExpanded && (
        <SyncPricingPanel provider={provider} existing={pricing} onChanged={refetch} />
      )}
    </section>
  );
}

interface ModelPricingRowProps {
  entry: PricingEntry;
  onChanged: () => void;
}

function ModelPricingRow({ entry, onChanged }: ModelPricingRowProps) {
  const [inputCost, setInputCost] = React.useState(entry.inputCostPer1m.toFixed(4));
  const [outputCost, setOutputCost] = React.useState(entry.outputCostPer1m.toFixed(4));
  const [maxContext, setMaxContext] = React.useState(
    entry.maxContextTokens > 0 ? String(entry.maxContextTokens) : ""
  );
  const [markup, setMarkup] = React.useState(String(entry.markupPercent));
  const [saving, setSaving] = React.useState(false);

  const baseline = {
    inputCost: entry.inputCostPer1m.toFixed(4),
    outputCost: entry.outputCostPer1m.toFixed(4),
    maxContext: entry.maxContextTokens > 0 ? String(entry.maxContextTokens) : "",
    markup: String(entry.markupPercent),
  };

  const dirty =
    inputCost !== baseline.inputCost ||
    outputCost !== baseline.outputCost ||
    maxContext !== baseline.maxContext ||
    markup !== baseline.markup;

  const handleRevert = () => {
    setInputCost(baseline.inputCost);
    setOutputCost(baseline.outputCost);
    setMaxContext(baseline.maxContext);
    setMarkup(baseline.markup);
  };

  const handleSave = async () => {
    const inV = parseFloat(inputCost);
    const outV = parseFloat(outputCost);
    const mkV = parseFloat(markup);
    const ctxV = parseInt(maxContext || "0", 10);
    if ([inV, outV, mkV, ctxV].some((v) => Number.isNaN(v))) {
      toast.error("Costs, markup, and context must be numbers");
      return;
    }
    if (inV < 0 || outV < 0 || mkV < 0 || ctxV < 0) {
      toast.error("Values must be non-negative");
      return;
    }
    setSaving(true);
    try {
      await setPricing({
        provider: entry.provider,
        model: entry.model,
        inputCostPer1m: inV,
        outputCostPer1m: outV,
        markupPercent: mkV,
        maxContextTokens: ctxV,
      });
      toast.success(`Pricing for ${entry.model} saved`);
      onChanged();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save pricing");
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!window.confirm(`Delete pricing for ${entry.provider}/${entry.model}?`)) return;
    try {
      await deletePricing(entry.id);
      toast.success(`Pricing for ${entry.model} deleted`);
      onChanged();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete pricing");
    }
  };

  return (
    <div className="flex flex-col md:flex-row md:items-end gap-3 px-4 py-3">
      <div className="min-w-0 flex-1 md:self-center">
        <p className="font-mono text-sm truncate" title={entry.model}>
          {entry.model}
        </p>
      </div>
      <div className="flex flex-wrap items-end gap-2">
        <CostField label="In $/1M" value={inputCost} onChange={setInputCost} disabled={saving} />
        <CostField label="Out $/1M" value={outputCost} onChange={setOutputCost} disabled={saving} />
        <CostField label="Ctx" value={maxContext} onChange={setMaxContext} disabled={saving} placeholder="0" />
        <CostField label="Markup %" value={markup} onChange={setMarkup} disabled={saving} />
      </div>
      <div className="flex items-center gap-1">
        {dirty && !saving && (
          <>
            <Button variant="ghost" size="icon-sm" onClick={handleRevert} aria-label="Revert">
              <X size={14} />
            </Button>
            <Button size="sm" className="h-8 gap-1" onClick={handleSave} disabled={saving}>
              {saving ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />}
              Save
            </Button>
          </>
        )}
        <Button variant="ghost" size="icon-sm" className="text-muted-foreground hover:text-destructive" onClick={handleDelete} disabled={saving}>
          <Trash2 size={14} />
        </Button>
      </div>
    </div>
  );
}

function CostField({
  label,
  value,
  onChange,
  disabled,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
  placeholder?: string;
}) {
  const id = React.useId();
  return (
    <div className="space-y-1">
      <Label htmlFor={id} className="text-[10px] uppercase tracking-wider text-muted-foreground">
        {label}
      </Label>
      <Input
        id={id}
        type="number"
        step="0.0001"
        min="0"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        className="h-8 w-28 px-2 text-xs tabular-nums"
      />
    </div>
  );
}

interface UnpricedTargetRowProps {
  providerName: string;
  target: ModelTarget;
  onChanged: () => void;
}

function UnpricedTargetRow({ providerName, target, onChanged }: UnpricedTargetRowProps) {
  const [isAdding, setIsAdding] = React.useState(false);
  const [inputCost, setInputCost] = React.useState("0");
  const [outputCost, setOutputCost] = React.useState("0");
  const [markup, setMarkup] = React.useState("0");
  const [maxContext, setMaxContext] = React.useState("");
  const [saving, setSaving] = React.useState(false);

  const handleSave = async () => {
    const inV = parseFloat(inputCost);
    const outV = parseFloat(outputCost);
    const mkV = parseFloat(markup);
    const ctxV = parseInt(maxContext || "0", 10);
    if ([inV, outV, mkV, ctxV].some((v) => Number.isNaN(v))) {
      toast.error("Costs, markup, and context must be numbers");
      return;
    }
    if (inV < 0 || outV < 0 || mkV < 0 || ctxV < 0) {
      toast.error("Values must be non-negative");
      return;
    }
    setSaving(true);
    try {
      await setPricing({
        provider: providerName,
        model: target.targetModel,
        inputCostPer1m: inV,
        outputCostPer1m: outV,
        markupPercent: mkV,
        maxContextTokens: ctxV,
      });
      toast.success(`Pricing for ${target.targetModel} saved`);
      setIsAdding(false);
      onChanged();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save pricing");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="px-4 py-3">
      {!isAdding ? (
        <div className="flex items-center justify-between gap-2">
          <span className="font-mono text-sm truncate" title={target.targetModel}>
            {target.targetModel}
          </span>
          <Button variant="outline" size="sm" onClick={() => setIsAdding(true)}>
            <Plus size={12} className="mr-1" />
            Add pricing
          </Button>
        </div>
      ) : (
        <div className="space-y-3">
          <p className="font-mono text-sm">{target.targetModel}</p>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
            <CostField label="In $/1M" value={inputCost} onChange={setInputCost} disabled={saving} />
            <CostField label="Out $/1M" value={outputCost} onChange={setOutputCost} disabled={saving} />
            <CostField label="Ctx" value={maxContext} onChange={setMaxContext} disabled={saving} placeholder="0" />
            <CostField label="Markup %" value={markup} onChange={setMarkup} disabled={saving} />
          </div>
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={() => setIsAdding(false)} disabled={saving}>
              Cancel
            </Button>
            <Button size="sm" onClick={handleSave} disabled={saving}>
              {saving ? <Loader2 size={12} className="animate-spin mr-1" /> : <Save size={12} className="mr-1" />}
              Save
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

interface SyncPricingPanelProps {
  provider: Provider;
  existing: PricingEntry[];
  onChanged: () => void;
}

function SyncPricingPanel({ provider, existing, onChanged }: SyncPricingPanelProps) {
  const [upstream, setUpstream] = React.useState<string[] | null>(null);
  const [fetching, setFetching] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const [addingModel, setAddingModel] = React.useState<string | null>(null);
  const [inputCost, setInputCost] = React.useState("0");
  const [outputCost, setOutputCost] = React.useState("0");
  const [adding, setAdding] = React.useState(false);

  React.useEffect(() => {
    let cancelled = false;
    setFetching(true);
    setError(null);
    previewProviderModels({
      baseUrl: provider.baseUrl,
      authType: provider.authType,
      authHeaderName: provider.authHeaderName,
      authPrefix: provider.authPrefix,
    })
      .then((res) => {
        if (!cancelled) setUpstream(res.models);
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : "Failed to fetch upstream models");
      })
      .finally(() => {
        if (!cancelled) setFetching(false);
      });
    return () => {
      cancelled = true;
    };
  }, [provider]);

  const existingSet = React.useMemo(() => new Set(existing.map((e) => e.model)), [existing]);
  const notConfigured = React.useMemo(
    () => (upstream ?? []).filter((m) => !existingSet.has(m)),
    [upstream, existingSet]
  );
  const configuredOnUpstream = React.useMemo(
    () => (upstream ?? []).filter((m) => existingSet.has(m)),
    [upstream, existingSet]
  );

  const startAdd = (model: string) => {
    setAddingModel(model);
    setInputCost("0");
    setOutputCost("0");
  };

  const submitAdd = async () => {
    if (!addingModel) return;
    const inV = parseFloat(inputCost);
    const outV = parseFloat(outputCost);
    if (Number.isNaN(inV) || Number.isNaN(outV)) {
      toast.error("Costs must be numbers");
      return;
    }
    if (inV < 0 || outV < 0) {
      toast.error("Costs must be non-negative");
      return;
    }
    setAdding(true);
    try {
      await setPricing({
        provider: provider.name,
        model: addingModel,
        inputCostPer1m: inV,
        outputCostPer1m: outV,
        markupPercent: 0,
        maxContextTokens: 0,
      });
      toast.success(`Pricing for ${addingModel} added`);
      setUpstream((prev) => (prev ? prev.filter((m) => m !== addingModel) : prev));
      setAddingModel(null);
      setInputCost("0");
      setOutputCost("0");
      onChanged();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to add pricing");
    } finally {
      setAdding(false);
    }
  };

  return (
    <div className="border rounded-md p-4 space-y-4">
      <h3 className="text-sm font-medium text-muted-foreground">Sync pricing for {provider.name}</h3>
      {fetching && upstream === null && <LoadingSpinner />}
      {error && <p className="text-sm text-destructive">{error}</p>}

      {!fetching && !error && upstream !== null && (
        <div className="space-y-4">
          <section className="space-y-2">
            <h4 className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
              New on upstream ({notConfigured.length})
            </h4>
            {notConfigured.length === 0 ? (
              <p className="text-sm text-muted-foreground">All upstream models are already configured.</p>
            ) : (
              <div className="border rounded-md divide-y">
                {notConfigured.map((model) => (
                  <div key={model} className="px-4 py-3">
                    {addingModel === model ? (
                      <div className="space-y-3">
                        <p className="font-mono text-sm">{model}</p>
                        <div className="flex flex-wrap gap-2">
                          <CostField label="In $/1M" value={inputCost} onChange={setInputCost} disabled={adding} />
                          <CostField label="Out $/1M" value={outputCost} onChange={setOutputCost} disabled={adding} />
                        </div>
                        <div className="flex justify-end gap-2">
                          <Button size="sm" variant="ghost" onClick={() => setAddingModel(null)} disabled={adding}>
                            Cancel
                          </Button>
                          <Button size="sm" onClick={submitAdd} disabled={adding}>
                            {adding ? <Loader2 size={12} className="animate-spin mr-1" /> : <Save size={12} className="mr-1" />}
                            Save
                          </Button>
                        </div>
                      </div>
                    ) : (
                      <div className="flex items-center justify-between gap-2">
                        <span className="font-mono text-sm truncate" title={model}>
                          {model}
                        </span>
                        <Button size="sm" variant="outline" onClick={() => startAdd(model)}>
                          <Plus size={12} className="mr-1" />
                          Add
                        </Button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </section>

          {configuredOnUpstream.length > 0 && (
            <section className="space-y-2">
              <h4 className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                Already configured ({configuredOnUpstream.length})
              </h4>
              <div className="flex flex-wrap gap-2">
                {configuredOnUpstream.map((m) => (
                  <span key={m} className="font-mono text-xs px-2 py-1 bg-muted rounded">
                    {m}
                  </span>
                ))}
              </div>
            </section>
          )}
        </div>
      )}
    </div>
  );
}
