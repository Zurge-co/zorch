"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { PageHeader } from "@/components/PageHeader";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { EmptyState } from "@/components/ui/empty-state";
import { fetchApiKeys, revokeApiKey, ApiKey, ApiKeyTag } from "@/lib/api";
import { useFetchData } from "@/lib/useFetchData";
import { useMutationWithFeedback } from "@/lib/useMutationWithFeedback";
import { KeyRound, Plus, Search, Trash2, Pencil, Clock } from "lucide-react";

function TagChips({ tags }: { tags: ApiKeyTag[] }) {
  if (!tags || tags.length === 0) return null;
  return (
    <div className="flex flex-wrap gap-1">
      {tags.map((tag, i) => (
        <span key={i} className="inline-flex items-center rounded-md bg-muted px-1.5 py-0.5 text-xs font-mono">
          {tag.key}:{tag.value}
        </span>
      ))}
    </div>
  );
}

function WindowBadge({ apiKey }: { apiKey: ApiKey }) {
  if (apiKey.allowedHoursStart == null || apiKey.allowedHoursEnd == null) {
    return (
      <Badge variant="secondary" className="text-xs">
        24/7
      </Badge>
    );
  }
  const tz = apiKey.windowTimezone || "UTC";
  const now = new Date();
  let currentHour: number;
  try {
    currentHour = parseInt(
      now.toLocaleString("en-US", { timeZone: tz, hour: "numeric", hour12: false }),
      10
    );
    if (isNaN(currentHour)) currentHour = now.getUTCHours();
  } catch {
    currentHour = now.getUTCHours();
  }
  const start = apiKey.allowedHoursStart;
  const end = apiKey.allowedHoursEnd;
  const allowed = start <= end ? currentHour >= start && currentHour < end : currentHour >= start || currentHour < end;

  return (
    <div className="flex flex-col gap-0.5">
      <Badge variant={allowed ? "success" : "destructive"} className="text-xs gap-1">
        <Clock size={10} />
        {allowed ? "Allowed now" : "Blocked"}
      </Badge>
      <span className="text-[10px] text-muted-foreground">
        {start}:00–{end}:00 {tz}
      </span>
    </div>
  );
}

function LimitsBadge({ apiKey }: { apiKey: ApiKey }) {
  const parts: string[] = [];
  if (apiKey.requestsPerMinute != null) parts.push(`${apiKey.requestsPerMinute} RPM`);
  if (apiKey.requestsPerDay != null) parts.push(`${apiKey.requestsPerDay} RPD`);
  if (apiKey.maxSpendUsd != null) parts.push(`$${apiKey.maxSpendUsd}`);
  if (apiKey.allowedModels != null && apiKey.allowedModels.length > 0) {
    parts.push(`${apiKey.allowedModels.length} models`);
  }
  if (parts.length === 0) {
    return (
      <Badge variant="secondary" className="text-xs">
        Default
      </Badge>
    );
  }
  return <span className="text-[10px] text-muted-foreground">{parts.join(" / ")}</span>;
}

export default function ApiKeysPage() {
  const router = useRouter();
  const { data: apiKeys, loading, error, refetch } = useFetchData<ApiKey[]>(fetchApiKeys);
  const { mutate, mutating } = useMutationWithFeedback({ refetch });
  const [search, setSearch] = useState("");

  const handleRevoke = (id: string, name: string) =>
    mutate(() => revokeApiKey(id), {
      confirm: `Revoke API key "${name}"?`,
      errorPrefix: "Failed to revoke API key",
      danger: true,
      success: "API key revoked",
    });

  const filtered = (apiKeys ?? []).filter(
    (k) =>
      k.name.toLowerCase().includes(search.toLowerCase()) ||
      k.id.toLowerCase().includes(search.toLowerCase())
  );

  const isInitialLoading = loading && !apiKeys;

  return (
    <div className="space-y-8">
      <PageHeader
        title="API Key Management"
        description="Issue, revoke and monitor keys for your users."
        actions={
          <Button onClick={() => router.push("/api-keys/new")} className="gap-2">
            <Plus size={16} />
            Create New Key
          </Button>
        }
      />

      {isInitialLoading && <LoadingSpinner />}
      {!isInitialLoading && error && <ErrorMessage message={error} />}

      {!isInitialLoading && (
        <>
          <div className="relative max-w-sm">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" size={16} />
            <Input
              className="pl-9"
              placeholder="Search keys by name or ID..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>

          {(!apiKeys || apiKeys.length === 0) ? (
            <EmptyState
              icon={<KeyRound size={24} />}
              title="No API keys found"
              description="Create a key to allow access to the platform."
            />
          ) : (
            <div className="border rounded-md overflow-hidden">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Key Name</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Tags</TableHead>
                    <TableHead>Window</TableHead>
                    <TableHead>Limits</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filtered.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={6} className="text-center text-muted-foreground py-8">
                        No matching keys.
                      </TableCell>
                    </TableRow>
                  ) : (
                    filtered.map((key) => (
                      <TableRow key={key.id}>
                        <TableCell className="font-medium">{key.name}</TableCell>
                        <TableCell>
                          <Badge
                            variant={
                              key.status === "active"
                                ? "success"
                                : key.status === "revoked"
                                ? "destructive"
                                : "secondary"
                            }
                          >
                            {key.status}
                          </Badge>
                        </TableCell>
                        <TableCell>
                          <TagChips tags={key.tags} />
                        </TableCell>
                        <TableCell>
                          <WindowBadge apiKey={key} />
                        </TableCell>
                        <TableCell>
                          <LimitsBadge apiKey={key} />
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="flex justify-end gap-2">
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={key.status !== "active"}
                              onClick={() => router.push(`/api-keys/${key.id}/edit`)}
                            >
                              Edit
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="text-destructive"
                              disabled={mutating || key.status !== "active"}
                              onClick={() => handleRevoke(key.id, key.name)}
                            >
                              <Trash2 size={14} className="mr-1" />
                              Revoke
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          )}
        </>
      )}
    </div>
  );
}
