"use client";

import { useRouter } from "next/navigation";
import { useState, useEffect } from "react";
import { PageHeader } from "@/components/PageHeader";
import { Badge } from "@/components/ui/badge";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { EmptyState } from "@/components/ui/empty-state";
import { Button } from "@/components/ui/button";
import { Play, ArrowLeft } from "lucide-react";

interface MiddlewareRun {
  id: string;
  requestId: string | null;
  middlewareConfigId: string | null;
  phase: string;
  status: string;
  action: string;
  durationMs: number;
  bodyChanged: boolean;
  metadata: Record<string, unknown>;
  error: string | null;
  createdAt: string;
}

const PHASE_LABELS: Record<string, string> = {
  "request.pre_governance": "Pre-Governance",
  "request.pre_upstream": "Pre-Upstream",
};

const STATUS_VARIANTS: Record<string, "success" | "warning" | "destructive" | "secondary"> = {
  success: "success",
  blocked: "destructive",
  error: "destructive",
  skipped: "warning",
};

export default function MiddlewareRunsPage() {
  const router = useRouter();
  const [runs, setRuns] = useState<MiddlewareRun[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    setLoading(true);
    fetch("/api/v1/admin/middleware/runs")
      .then((r) => r.json())
      .then((data) => setRuns(data as MiddlewareRun[]))
      .catch((err) => setError(err instanceof Error ? err.message : "Failed to load runs"))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, []);

  return (
    <div className="space-y-8">
      <PageHeader
        title="Middleware Runs"
        description="Recent middleware execution history."
        actions={
          <div className="flex gap-2">
            <Button variant="outline" onClick={load}>
              Refresh
            </Button>
            <Button variant="outline" onClick={() => router.push("/middleware")}>
              <ArrowLeft size={16} className="mr-2" />
              Back to Configs
            </Button>
          </div>
        }
      />

      {loading && <LoadingSpinner />}
      {error && <ErrorMessage message={error} />}

      {!loading && !error && (!runs || runs.length === 0) && (
        <EmptyState
          icon={<Play size={24} />}
          title="No middleware runs yet"
          description="Middleware runs will appear here after requests are processed."
        />
      )}

      {!loading && runs && runs.length > 0 && (
        <div className="border rounded-md overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-muted">
              <tr>
                <th className="px-4 py-2 text-left font-medium">Config ID</th>
                <th className="px-4 py-2 text-left font-medium">Phase</th>
                <th className="px-4 py-2 text-left font-medium">Status</th>
                <th className="px-4 py-2 text-left font-medium">Action</th>
                <th className="px-4 py-2 text-left font-medium">Duration</th>
                <th className="px-4 py-2 text-left font-medium">Time</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {runs.map((run) => (
                <tr key={run.id}>
                  <td className="px-4 py-2 font-mono text-xs">
                    {run.middlewareConfigId ?? "—"}
                  </td>
                  <td className="px-4 py-2 text-xs">{PHASE_LABELS[run.phase] ?? run.phase}</td>
                  <td className="px-4 py-2">
                    <Badge variant={STATUS_VARIANTS[run.status] ?? "secondary"} className="text-[10px]">
                      {run.status}
                    </Badge>
                  </td>
                  <td className="px-4 py-2 text-xs">{run.action}</td>
                  <td className="px-4 py-2 text-xs tabular-nums">{run.durationMs}ms</td>
                  <td className="px-4 py-2 text-xs text-muted-foreground">
                    {new Date(run.createdAt).toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
