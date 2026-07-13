"use client";

import { useRouter, useParams } from "next/navigation";
import { useState } from "react";
import { PageHeader } from "@/components/PageHeader";
import { ModelForm, ModelFormData } from "@/components/models/ModelForm";
import { fetchModels, updateModel } from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";

export default function EditModelPage() {
  const params = useParams();
  const id = params.id as string;
  const router = useRouter();
  const { data: models, loading, error } = useFetchData(fetchModels);
  const model = models?.find((m) => m.id === id) ?? null;
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (data: ModelFormData) => {
    setSaving(true);
    try {
      await updateModel(id, data);
      toast.success(`Model "${data.publicName}" updated`);
      router.push("/models");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update model");
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit Model" description="Loading model..." />
        <LoadingSpinner />
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit Model" description="Failed to load model" />
        <ErrorMessage message={error} />
        <Button variant="outline" onClick={() => router.push("/models")}>
          Back to Models
        </Button>
      </div>
    );
  }

  if (!model) {
    return (
      <div className="space-y-8">
        <PageHeader title="Edit Model" description="Model not found" />
        <ErrorMessage message="The requested model does not exist." />
        <Button variant="outline" onClick={() => router.push("/models")}>
          Back to Models
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title={`Edit Model: ${model.publicName}`}
        description="Update the public model name and status."
      />
      <ModelForm
        defaultValues={{ publicName: model.publicName, isActive: model.isActive }}
        onSubmit={handleSubmit}
        onCancel={() => router.push("/models")}
        submitLabel="Save Changes"
        loading={saving}
      />
    </div>
  );
}
