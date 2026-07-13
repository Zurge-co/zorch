"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { PageHeader } from "@/components/PageHeader";
import { ModelForm, ModelFormData } from "@/components/models/ModelForm";
import { createModel } from "@/lib/api";
import { toast } from "@/components/ui/toast";

export default function NewModelPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (data: ModelFormData) => {
    setLoading(true);
    try {
      await createModel(data);
      toast.success(`Model "${data.publicName}" created`);
      router.push("/models");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create model");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-8">
      <PageHeader
        title="Create Model"
        description="Define a public model name that clients will use."
      />
      <ModelForm
        onSubmit={handleSubmit}
        onCancel={() => router.push("/models")}
        submitLabel="Create Model"
        loading={loading}
      />
    </div>
  );
}
