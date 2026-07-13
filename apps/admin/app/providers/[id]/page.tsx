"use client";

import { useRouter, useParams } from "next/navigation";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { ProviderTargetsSection } from "@/components/providers/ProviderTargetsSection";
import { ProviderKeysSection } from "@/components/providers/ProviderKeysSection";
import { ModelPricingSection } from "@/components/ModelPricingSection";
import { fetchProviders, toggleProviderActive, deleteProvider, Provider } from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { useMutationWithFeedback } from "@/lib/useMutationWithFeedback";
import { ArrowLeft, Pencil, Server, Trash2 } from "lucide-react";

export default function ProviderDetailPage() {
  const params = useParams();
  const id = params.id as string;
  const router = useRouter();
  const { data: providers, loading, error, refetch } = useFetchData(fetchProviders);
  const provider = providers?.find((p) => p.id === id) ?? null;

  const { mutate, mutating } = useMutationWithFeedback({ refetch });

  const handleDelete = () =>
    mutate(async () => {
      await deleteProvider(id);
      router.push("/providers");
    }, {
      confirm: "Are you sure you want to delete this provider?",
      errorPrefix: "Failed to delete provider",
      danger: true,
      success: "Provider deleted",
    });

  const handleToggleRouting = (next: boolean) =>
    mutate(() => toggleProviderActive(id, next), {
      errorPrefix: "Failed to update provider routing",
      success: next ? "Routing enabled" : "Routing disabled",
    });

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader title="Provider" description="Loading provider details..." />
        <LoadingSpinner />
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-8">
        <PageHeader title="Provider" description="Failed to load provider details" />
        <ErrorMessage message={error} />
      </div>
    );
  }

  if (!provider) {
    return (
      <div className="space-y-8">
        <PageHeader title="Provider" description="Provider not found" />
        <ErrorMessage message="The requested provider does not exist." />
        <Button variant="outline" onClick={() => router.push("/providers")}>
          <ArrowLeft size={16} className="mr-2" />
          Back to Providers
        </Button>
      </div>
    );
  }

  const statusVariant =
    {
      online: "success" as const,
      degraded: "warning" as const,
      offline: "destructive" as const,
    }[provider.status] || ("secondary" as const);

  return (
    <div className="space-y-8">
      <PageHeader
        title={provider.name}
        description={provider.baseUrl}
        actions={
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => router.push("/providers")}>
              <ArrowLeft size={16} className="mr-2" />
              Back
            </Button>
            <Button variant="outline" size="sm" onClick={() => router.push(`/providers/${id}/edit`)}>
              <Pencil size={16} className="mr-2" />
              Edit
            </Button>
            <Button variant="ghost" size="sm" className="text-destructive" onClick={handleDelete} disabled={mutating}>
              <Trash2 size={16} className="mr-2" />
              Delete
            </Button>
          </div>
        }
      />

      <div className="grid gap-6 md:grid-cols-3">
        <div className="md:col-span-2 space-y-2">
          <div className="flex items-center gap-3">
            <Server size={20} className="text-muted-foreground" />
            <h2 className="text-lg font-semibold">Overview</h2>
          </div>
          <div className="border rounded-md divide-y">
            <div className="px-4 py-3 flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Status</span>
              <Badge variant={statusVariant} className="capitalize">
                {provider.status}
              </Badge>
            </div>
            <div className="px-4 py-3 flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Auth Type</span>
              <Badge variant="outline">{provider.authType ?? "bearer"}</Badge>
            </div>
            <div className="px-4 py-3 flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Base URL</span>
              <span className="font-mono text-sm truncate max-w-md">{provider.baseUrl}</span>
            </div>
            <div className="px-4 py-3 flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Avg Latency</span>
              <span className="text-sm font-medium">{provider.latency}</span>
            </div>
            <div className="px-4 py-3 flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Per 1M Tokens</span>
              <span className="text-sm font-medium">{provider.costPer1m}</span>
            </div>
          </div>
        </div>

        <div className="space-y-2">
          <h2 className="text-lg font-semibold">Routing</h2>
          <div className="border rounded-md p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium">Accept requests</p>
                <p className="text-xs text-muted-foreground">
                  {provider.status === "online" ? "Active in gateway routing" : "Skipped by gateway"}
                </p>
              </div>
              <Switch
                checked={provider.status === "online"}
                onCheckedChange={handleToggleRouting}
                disabled={mutating}
              />
            </div>
          </div>
        </div>
      </div>

      <hr className="border-border" />
      <ProviderTargetsSection provider={provider} />

      <hr className="border-border" />
      <ProviderKeysSection provider={provider} />

      <hr className="border-border" />
      <ModelPricingSection provider={provider} />
    </div>
  );
}
