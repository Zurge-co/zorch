"use client";

import { Badge } from "@/components/ui/badge";
import { ArrowUpRight, DollarSign, Info } from "lucide-react";
import { fetchPricing, type PricingEntry } from "@/lib/api";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/ui/empty-state";
import { useFetchData } from "@/lib/useFetchData";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

function formatTokenCount(tokens: number): string {
  if (tokens >= 1_000_000) {
    const m = tokens / 1_000_000;
    return m === Math.floor(m) ? `${m}M` : `${m.toFixed(1)}M`;
  }
  if (tokens >= 1_000) {
    const k = tokens / 1_000;
    return k === Math.floor(k) ? `${k}k` : `${k.toFixed(1)}k`;
  }
  return tokens.toLocaleString();
}

export default function PricingPage() {
  const { data: pricing, loading, error } = useFetchData<PricingEntry[]>(fetchPricing);

  const grouped = (pricing ?? []).reduce<Record<string, PricingEntry[]>>(
    (acc, entry) => {
      (acc[entry.provider] ??= []).push(entry);
      return acc;
    },
    {}
  );

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader
          title="Model Pricing"
          description="Read-only audit of every configured (provider, model) pricing row."
        />
        <LoadingSpinner />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <PageHeader
        title="Model Pricing"
        description="Read-only audit of every configured (provider, model) pricing row."
      />

      <div className="flex items-start gap-3 rounded-lg border border-border bg-muted/40 px-4 py-3 text-xs text-muted-foreground">
        <Info size={14} className="mt-0.5 shrink-0" />
        <p>
          Editing happens per-provider on the{" "}
          <a
            href="/providers"
            className="inline-flex items-center gap-0.5 font-medium text-foreground underline-offset-2 hover:underline"
          >
            Providers
            <ArrowUpRight size={11} />
          </a>{" "}
          page. This view is for cross-provider auditing.
        </p>
      </div>

      {error && <ErrorMessage message={error} />}

      {(!pricing || pricing.length === 0) ? (
        <EmptyState
          icon={<DollarSign size={24} />}
          title="No pricing entries"
          description="Configure pricing per provider on the Providers page to enable cost tracking."
        />
      ) : (
        <div className="space-y-8">
          {Object.entries(grouped).map(([provider, entries]) => (
            <section key={provider} className="space-y-3">
              <div className="flex items-center gap-2">
                <h2 className="text-lg font-semibold">{provider}</h2>
                <Badge variant="secondary">{entries.length} model{entries.length === 1 ? "" : "s"}</Badge>
              </div>
              <div className="border rounded-md overflow-hidden">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Model</TableHead>
                      <TableHead>Input / 1M</TableHead>
                      <TableHead>Output / 1M</TableHead>
                      <TableHead>Max Context</TableHead>
                      <TableHead>Markup</TableHead>
                      <TableHead>Updated</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {entries.map((entry) => (
                      <TableRow key={entry.id}>
                        <TableCell className="font-mono text-sm">{entry.model}</TableCell>
                        <TableCell>${entry.inputCostPer1m.toFixed(4)}</TableCell>
                        <TableCell>${entry.outputCostPer1m.toFixed(4)}</TableCell>
                        <TableCell>
                          {entry.maxContextTokens > 0 ? formatTokenCount(entry.maxContextTokens) : "Not set"}
                        </TableCell>
                        <TableCell>
                          <Badge variant={entry.markupPercent > 0 ? "success" : "secondary"}>{entry.markupPercent}%</Badge>
                        </TableCell>
                        <TableCell className="text-muted-foreground text-xs">
                          {new Date(entry.updatedAt).toLocaleString()}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
