"use client";

import { useRouter, useParams } from "next/navigation";
import { PageHeader } from "@/components/PageHeader";
import { ProviderForm, ProviderFormData } from "@/components/providers/ProviderForm";
import { fetchProviders, updateProvider } from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { useState } from "react";

export default function EditProviderPage() {
  const params = useParams();
  const id = params.id as string;
  const router = useRouter();
  const { data: providers, loading, error } = useFetchData(fetchProviders);
  const provider = providers?.find((p) => p.id === id) ?? null;
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (data: ProviderFormData) => {
    if (!data.name) {
      toast.error("Provider name is required");
      return;
    }
    if (data.authType === "custom" && !data.authHeaderName) {
      toast.error("Auth header name is required for custom auth type");
      return;
    }

    setSaving(true);
    try {
      const payload: Parameters<typeof updateProvider>[1] = {
        name: data.name,
        baseUrl: data.baseUrl,
        authType: data.authType,
        isActive: data.isActive,
      };
      if (data.authType === "custom") {
        payload.authHeaderName = data.authHeaderName;
        if (data.authPrefix) payload.authPrefix = data.authPrefix;
      }
      await updateProvider(id, payload);
      toast.success(`Provider "${data.name}" updated`);
      router.push(`/providers/${id}`);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update provider");
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit Provider" description="Update provider endpoint configuration." />
        <LoadingSpinner />
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit Provider" description="Update provider endpoint configuration." />
        <ErrorMessage message={error} />
        <Button variant="outline" onClick={() => router.push("/providers")}>
          Back to Providers
        </Button>
      </div>
    );
  }

  if (!provider) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit Provider" description="Update provider endpoint configuration." />
        <ErrorMessage message="Provider not found" />
        <Button variant="outline" onClick={() => router.push("/providers")}>
          Back to Providers
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={`Edit Provider: ${provider.name}`}
        description="Update provider endpoint configuration."
      />
      <ProviderForm
        defaultValues={{
          name: provider.name,
          authType: provider.authType ?? "bearer",
          baseUrl: provider.baseUrl,
          authHeaderName: provider.authHeaderName ?? "",
          authPrefix: provider.authPrefix ?? "",
          isActive: provider.status === "online",
        }}
        onSubmit={handleSubmit}
        onCancel={() => router.push(`/providers/${id}`)}
        submitLabel="Update Provider"
        loading={saving}
      />
    </div>
  );
}
