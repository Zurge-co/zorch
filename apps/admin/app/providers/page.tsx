"use client";

import Link from "next/link";
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
import { Switch } from "@/components/ui/switch";
import { PageHeader } from "@/components/PageHeader";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { EmptyState } from "@/components/ui/empty-state";
import { fetchProviders, deleteProvider, toggleProviderActive, Provider } from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { useMutationWithFeedback } from "@/lib/useMutationWithFeedback";
import { Plus, Server, Trash2 } from "lucide-react";

export default function ProvidersPage() {
  const router = useRouter();
  const { data: providers, loading, error, refetch } = useFetchData(fetchProviders);
  const { mutate, mutating } = useMutationWithFeedback({ refetch });

  const handleDelete = (id: string) =>
    mutate(() => deleteProvider(id), {
      confirm: "Are you sure you want to delete this provider?",
      errorPrefix: "Failed to delete provider",
      danger: true,
      success: "Provider deleted",
    });

  const handleToggleRouting = (provider: Provider, next: boolean) =>
    mutate(() => toggleProviderActive(provider.id, next), {
      errorPrefix: "Failed to update provider routing",
      success: next ? "Routing enabled" : "Routing disabled",
    });

  const isInitialLoading = loading && !providers;

  const statusVariant = (status: string) =>
    ({
      online: "success" as const,
      degraded: "warning" as const,
      offline: "destructive" as const,
    }[status] || "secondary" as const);

  return (
    <div className="space-y-8">
      <PageHeader
        title="Provider Configuration"
        description="Manage model providers and routing status."
        actions={
          <Button onClick={() => router.push("/providers/new")} className="gap-2">
            <Plus size={16} />
            Add Provider
          </Button>
        }
      />

      {isInitialLoading && <LoadingSpinner />}
      {!isInitialLoading && error && <ErrorMessage message={error} />}

      {!isInitialLoading && (
        <>
          {(!providers || providers.length === 0) ? (
            <EmptyState
              icon={<Server size={24} />}
              title="No providers configured"
              description="Add a provider to start routing requests across models."
            />
          ) : (
            <div className="border rounded-md overflow-hidden">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Auth Type</TableHead>
                    <TableHead>Base URL</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Routing</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {providers.map((provider) => (
                    <TableRow key={provider.id}>
                      <TableCell className="font-medium">
                        <Link href={`/providers/${provider.id}`} className="hover:underline">
                          {provider.name}
                        </Link>
                      </TableCell>
                      <TableCell>
                        <Badge variant="outline">{provider.authType ?? "bearer"}</Badge>
                      </TableCell>
                      <TableCell className="font-mono text-xs truncate max-w-xs">
                        {provider.baseUrl}
                      </TableCell>
                      <TableCell>
                        <Badge variant={statusVariant(provider.status)} className="capitalize">
                          {provider.status}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <Switch
                          checked={provider.status === "online"}
                          onCheckedChange={(next) => handleToggleRouting(provider, next)}
                          disabled={mutating}
                          aria-label={`Toggle routing for ${provider.name}`}
                        />
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-2">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => router.push(`/providers/${provider.id}`)}
                          >
                            View
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => router.push(`/providers/${provider.id}/edit`)}
                          >
                            Edit
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            className="text-destructive"
                            onClick={() => handleDelete(provider.id)}
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
        </>
      )}
    </div>
  );
}
