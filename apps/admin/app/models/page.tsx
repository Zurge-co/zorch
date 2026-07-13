"use client";

import { useRouter } from "next/navigation";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { PageHeader } from "@/components/PageHeader";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { EmptyState } from "@/components/ui/empty-state";
import { fetchModels, deleteModel } from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { useMutationWithFeedback } from "@/lib/useMutationWithFeedback";
import { Layers, Plus, Trash2 } from "lucide-react";

export default function ModelsPage() {
  const router = useRouter();
  const { data: models, loading, error, refetch } = useFetchData(fetchModels);
  const { mutate, mutating } = useMutationWithFeedback({ refetch });

  const handleDelete = (id: string, name: string) =>
    mutate(() => deleteModel(id), {
      confirm: `Delete model "${name}"?`,
      errorPrefix: "Failed to delete model",
      danger: true,
      success: "Model deleted",
    });

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader
          title="Models"
          description="Public model names and their upstream provider targets."
        />
        <LoadingSpinner />
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-8">
        <PageHeader
          title="Models"
          description="Public model names and their upstream provider targets."
        />
        <ErrorMessage message={error} />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title="Models"
        description="Public model names and their upstream provider targets."
        actions={
          <Button onClick={() => router.push("/models/new")} className="gap-2">
            <Plus size={16} />
            Create Model
          </Button>
        }
      />

      <p className="text-sm text-muted-foreground">
        Clients send the public name in their request body. Each public model maps to one or more
        provider/target pairs, tried by priority.
      </p>

      {(!models || models.length === 0) ? (
        <EmptyState
          icon={<Layers size={24} />}
          title="No models configured"
          description="Create a model to start routing requests."
        />
      ) : (
        <div className="border rounded-md overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Public Name</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {models.map((model) => (
                <TableRow key={model.id}>
                  <TableCell className="font-medium">{model.publicName}</TableCell>
                  <TableCell>
                    {model.isActive ? (
                      <Badge variant="default">Active</Badge>
                    ) : (
                      <Badge variant="secondary">Inactive</Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-2">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => router.push(`/models/${model.id}/targets`)}
                      >
                        Targets
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => router.push(`/models/${model.id}/edit`)}
                      >
                        Edit
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-destructive"
                        onClick={() => handleDelete(model.id, model.publicName)}
                        disabled={mutating}
                      >
                        <Trash2 size={14} className="mr-1" />
                        Delete
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}
