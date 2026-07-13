"use client";

import { useRouter } from "next/navigation";
import { PageHeader } from "@/components/PageHeader";
import { ProviderForm, ProviderFormData } from "@/components/providers/ProviderForm";
import { createProvider } from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { useState } from "react";

export default function NewProviderPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (data: ProviderFormData) => {
    if (!data.name || !data.baseUrl) {
      toast.error("Name and Base URL are required");
      return;
    }
    if (data.authType === "custom" && !data.authHeaderName) {
      toast.error("Auth header name is required for custom auth type");
      return;
    }

    setLoading(true);
    try {
      const payload: Parameters<typeof createProvider>[0] = {
        name: data.name,
        baseUrl: data.baseUrl,
        authType: data.authType,
        isActive: data.isActive,
      };
      if (data.authType === "custom") {
        payload.authHeaderName = data.authHeaderName;
        if (data.authPrefix) payload.authPrefix = data.authPrefix;
      }
      await createProvider(payload);
      toast.success(`Provider "${data.name}" created`);
      router.push("/providers");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create provider");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-8">
      <PageHeader
        title="Add Provider"
        description="Register a new AI provider endpoint."
      />
      <ProviderForm
        onSubmit={handleSubmit}
        onCancel={() => router.push("/providers")}
        submitLabel="Save Provider"
        loading={loading}
      />
    </div>
  );
}
