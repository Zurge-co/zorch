"use client";

import React from "react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { AlertCircle, Clock, Settings2 } from "lucide-react";
import { fetchGatewayConfig } from "@/lib/api";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { PageHeader } from "@/components/PageHeader";
import { useFetchData } from "@/lib/useFetchData";

export default function SettingsPage() {
  const { data, loading, error } = useFetchData(fetchGatewayConfig);

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader
          title="Gateway Settings"
          description="View current gateway runtime configuration."
        />
        <LoadingSpinner />
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-8">
        <PageHeader
          title="Gateway Settings"
          description="View current gateway runtime configuration."
        />
        <ErrorMessage message={error} />
      </div>
    );
  }

  const config = data!;

  return (
    <div className="space-y-8">
      <PageHeader
        title="Gateway Settings"
        description="Runtime configuration loaded from environment variables. Restart the gateway to apply changes."
      />

      <div className="grid gap-6 md:grid-cols-2">
        <section className="border rounded-md p-6 space-y-4">
          <div className="flex items-center gap-2">
            <Clock size={18} className="text-muted-foreground" />
            <h2 className="text-lg font-semibold">Timeouts</h2>
          </div>
          <div className="space-y-2">
            <Label htmlFor="timeoutSecs">Upstream Timeout (seconds)</Label>
            <Input id="timeoutSecs" value={config.timeoutSecs} disabled />
          </div>
          <div className="space-y-2">
            <Label htmlFor="circuitBreakerTimeoutSecs">Circuit Breaker Recovery (seconds)</Label>
            <Input id="circuitBreakerTimeoutSecs" value={config.circuitBreakerTimeoutSecs} disabled />
            <p className="text-xs text-muted-foreground">
              A backend that fails is excluded from random routing until this timeout passes,
              then it receives probe requests again.
            </p>
          </div>
        </section>

        <section className="border rounded-md p-6 space-y-4">
          <div className="flex items-center gap-2">
            <Settings2 size={18} className="text-muted-foreground" />
            <h2 className="text-lg font-semibold">Runtime</h2>
          </div>
          <div className="space-y-2">
            <Label htmlFor="appPort">App Port</Label>
            <Input id="appPort" value={config.appPort} disabled />
          </div>
          <div className="space-y-2">
            <Label htmlFor="rustLog">Rust Log Level</Label>
            <Input id="rustLog" value={config.rustLog} disabled />
          </div>
          <div className="space-y-2">
            <Label htmlFor="inspectorCaptureLevel">Inspector Capture Level</Label>
            <div className="flex items-center gap-2">
              <Input id="inspectorCaptureLevel" value={config.inspectorCaptureLevel} disabled />
              <Badge variant="outline">{config.inspectorCaptureLevel}</Badge>
            </div>
          </div>
        </section>
      </div>

      <div className="flex items-start gap-3 rounded-lg border border-border bg-muted/40 px-4 py-3 text-sm text-muted-foreground">
        <AlertCircle className="mt-0.5 h-5 w-5 text-muted-foreground shrink-0" />
        <div>
          <p className="font-medium text-foreground">Environment variables</p>
          <p>
            These values are read from environment variables such as{" "}
            <code className="rounded bg-muted px-1 py-0.5">ZORCH_TIMEOUT_SECS</code> and{" "}
            <code className="rounded bg-muted px-1 py-0.5">
              ZORCH_CIRCUIT_BREAKER_TIMEOUT_SECS
            </code>{" "}
            at startup. Update the gateway deployment and restart to change them.
          </p>
        </div>
      </div>
    </div>
  );
}
