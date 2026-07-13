"use client";

import React, { useState } from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  BarChart,
  Bar,
  LabelList,
} from "recharts";
import {
  fetchAnalytics,
  fetchTagAnalytics,
  AnalyticsData,
  TagAnalyticsData,
} from "@/lib/api";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { PageHeader } from "@/components/PageHeader";
import { ChartCard } from "@/components/ChartCard";
import { MetricCard } from "@/components/MetricCard";
import { EmptyState } from "@/components/ui/empty-state";
import { useFetchData } from "@/lib/useFetchData";
import {
  Coins,
  DollarSign,
  Clock,
  AlertTriangle,
  Tag,
} from "lucide-react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";

const TAG_FILTERS = [
  { label: "All", value: "" },
  { label: "Last 24h", value: "24h" },
  { label: "Last 7d", value: "7d" },
  { label: "Last 30d", value: "30d" },
];

const TOOLTIP_STYLE = {
  backgroundColor: "var(--background)",
  border: "1px solid var(--border)",
  borderRadius: "8px",
  boxShadow: "0 4px 12px rgba(0, 0, 0, 0.15)",
  padding: "8px 12px",
};

const TOOLTIP_WRAPPER_STYLE = { zIndex: 999 };

export default function AnalyticsPage() {
  const [range, setRange] = useState<string>("");
  const [tagRange, setTagRange] = useState<string>("7d");
  const [tagFilter, setTagFilter] = useState<string>("");

  const { data: analytics, loading, error } = useFetchData<AnalyticsData>(
    () => fetchAnalytics(tagFilter || undefined, range || undefined)
  );

  const { data: tagAnalytics } = useFetchData<TagAnalyticsData>(
    () => fetchTagAnalytics(tagRange || undefined)
  );

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader
          title="Analytics"
          description="Deep dive into token usage and cost efficiency."
        />
        <LoadingSpinner />
      </div>
    );
  }

  const hasData = analytics && (
    analytics.tokenUsage?.length > 0 ||
    analytics.costTrends?.length > 0 ||
    analytics.latencyBreakdown?.percentileLatencyMs?.length > 0
  );

  const totalTokens = analytics?.tokenUsage?.reduce((sum, p) => sum + p.usage, 0) || 0;
  const totalCostCents = analytics?.costTrends?.reduce((sum, p) => sum + p.cost, 0) || 0;
  const totalCost = totalCostCents / 100;
  const avgLatency = analytics?.latencyBreakdown?.avgLatencyMs?.toFixed(0) ?? "0";
  const avgProviderLatency = analytics?.latencyBreakdown?.avgProviderLatencyMs?.toFixed(0) ?? "0";
  const avgGatewayLatency = analytics?.latencyBreakdown?.avgGatewayLatencyMs?.toFixed(0) ?? "0";
  const errorRate = analytics?.errorRate ?? 0;
  const totalRequests24h = analytics?.totalRequests24h ?? 0;
  const errorRequests24h = analytics?.errorRequests24h ?? 0;

  const hasTagData = tagAnalytics && tagAnalytics.tags && tagAnalytics.tags.length > 0;

  return (
    <div className="space-y-8">
      <PageHeader
        title="Analytics"
        description="Deep dive into token usage and cost efficiency."
      />

      <div className="flex gap-4 items-center">
        <Select value={range || "all"} onValueChange={(v) => v && setRange(v === "all" ? "" : v)}>
          <SelectTrigger className="w-36">
            <SelectValue placeholder="Time range" />
          </SelectTrigger>
          <SelectContent>
            {TAG_FILTERS.map((f) => (
              <SelectItem key={f.value || "all"} value={f.value || "all"}>
                {f.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="relative max-w-xs">
          <Tag
            size={14}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            className="pl-9"
            placeholder="Filter by tag (e.g. project:marketing)"
            value={tagFilter}
            onChange={(e) => setTagFilter(e.target.value)}
          />
        </div>
      </div>

      {error && <ErrorMessage message={error} />}

      {!hasData && !error && !hasTagData && (
        <EmptyState
          title="No analytics data available"
          description="Activity will appear here once the proxy starts receiving requests."
        />
      )}

      {hasData && (
        <>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            <MetricCard
              title="Total Tokens"
              value={totalTokens.toLocaleString()}
              description="Across all providers"
              icon={<Coins size={16} />}
            />
            <MetricCard
              title="Total Cost"
              value={`$${totalCost.toFixed(2)}`}
              description="Cumulative spend"
              icon={<DollarSign size={16} />}
            />
            <MetricCard
              title="Error Rate"
              value={`${errorRate.toFixed(2)}%`}
              description={totalRequests24h > 0
                ? `${errorRequests24h} of ${totalRequests24h} requests`
                : "Last 24 hours"}
              trend={errorRate > 1 ? "up" : "flat"}
              icon={<AlertTriangle size={16} />}
            />
            <MetricCard
              title="Avg Latency"
              value={`${avgLatency}ms`}
              description="Total round-trip time"
              icon={<Clock size={16} />}
            />
            <MetricCard
              title="Avg Provider Latency"
              value={`${avgProviderLatency}ms`}
              description="Time waiting for upstream"
              icon={<Clock size={16} />}
            />
            <MetricCard
              title="Avg Gateway Latency"
              value={`${avgGatewayLatency}ms`}
              description="Gateway + middleware overhead"
              icon={<Clock size={16} />}
            />
          </div>

          <div className="grid gap-6 md:grid-cols-2">
            <ChartCard
              title="Token Usage"
              description="Total tokens processed across all providers"
            >
              {analytics?.tokenUsage && analytics.tokenUsage.length > 0 ? (
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={analytics.tokenUsage}>
                    <defs>
                      <linearGradient id="colorUsage" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="var(--primary)" stopOpacity={0.2} />
                        <stop offset="95%" stopColor="var(--primary)" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" vertical={false} className="stroke-border" />
                    <XAxis
                      dataKey="name"
                      axisLine={false}
                      tickLine={false}
                      tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                    />
                    <YAxis
                      axisLine={false}
                      tickLine={false}
                      tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                    />
                    <Tooltip
                      contentStyle={TOOLTIP_STYLE}
                      wrapperStyle={TOOLTIP_WRAPPER_STYLE}
                    />
                    <Area
                      type="monotone"
                      dataKey="usage"
                      stroke="var(--primary)"
                      fillOpacity={1}
                      fill="url(#colorUsage)"
                      strokeWidth={2}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              ) : (
                <EmptyState variant="compact" title="No token usage data" />
              )}
            </ChartCard>

            <ChartCard
              title="Cost Trends"
              description="Total expenditure in USD"
            >
              {analytics?.costTrends && analytics.costTrends.length > 0 ? (
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={analytics.costTrends}>
                    <CartesianGrid strokeDasharray="3 3" vertical={false} className="stroke-border" />
                    <XAxis
                      dataKey="name"
                      axisLine={false}
                      tickLine={false}
                      tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                    />
                    <YAxis
                      axisLine={false}
                      tickLine={false}
                      tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                    />
                    <Tooltip
                      contentStyle={TOOLTIP_STYLE}
                      wrapperStyle={TOOLTIP_WRAPPER_STYLE}
                    />
                    <Bar dataKey="cost" fill="var(--primary)" radius={[4, 4, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              ) : (
                <EmptyState variant="compact" title="No cost data" />
              )}
            </ChartCard>
          </div>

          <div className="grid gap-6 md:grid-cols-2">
            <ChartCard
              title="Latency Percentiles"
              description="Round-trip time from gateway to model response"
            >
              {analytics?.latencyBreakdown?.percentileLatencyMs && analytics.latencyBreakdown.percentileLatencyMs.length > 0 ? (
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={analytics.latencyBreakdown.percentileLatencyMs}>
                    <CartesianGrid strokeDasharray="3 3" vertical={false} className="stroke-border" />
                    <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "var(--muted-foreground)" }} />
                    <YAxis type="number" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "var(--muted-foreground)" }} />
                    <Tooltip
                      cursor={{ fill: "rgba(128, 128, 128, 0.1)" }}
                      contentStyle={TOOLTIP_STYLE}
                      wrapperStyle={TOOLTIP_WRAPPER_STYLE}
                    />
                    <Bar dataKey="value" fill="var(--primary)" radius={[4, 4, 0, 0]}>
                      <LabelList dataKey="value" position="top" formatter={(value: any) => `${Number(value).toLocaleString()}ms`} fontSize={11} fill="var(--muted-foreground)" />
                    </Bar>
                  </BarChart>
                </ResponsiveContainer>
              ) : (
                <EmptyState variant="compact" title="No latency data" />
              )}
            </ChartCard>

            <ChartCard
              title="Latency Breakdown"
              description="Average time spent waiting for provider vs gateway overhead"
            >
              {analytics?.latencyBreakdown ? (
                <ResponsiveContainer width="100%" height="100%">
                    <BarChart
                    data={[
                      {
                        name: "Provider",
                        value: analytics.latencyBreakdown.avgProviderLatencyMs,
                      },
                      {
                        name: "Gateway",
                        value: analytics.latencyBreakdown.avgGatewayLatencyMs,
                      },
                    ]}
                  >
                    <CartesianGrid strokeDasharray="3 3" vertical={false} className="stroke-border" />
                    <XAxis dataKey="name" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "var(--muted-foreground)" }} />
                    <YAxis type="number" axisLine={false} tickLine={false} tick={{ fontSize: 11, fill: "var(--muted-foreground)" }} />
                    <Tooltip
                      cursor={{ fill: "rgba(128, 128, 128, 0.1)" }}
                      contentStyle={TOOLTIP_STYLE}
                      wrapperStyle={TOOLTIP_WRAPPER_STYLE}
                    />
                    <Bar dataKey="value" fill="var(--primary)" radius={[4, 4, 0, 0]}>
                      <LabelList dataKey="value" position="top" formatter={(value: any) => `${Number(value).toLocaleString()}ms`} fontSize={11} fill="var(--muted-foreground)" />
                    </Bar>
                  </BarChart>
                </ResponsiveContainer>
              ) : (
                <EmptyState variant="compact" title="No latency breakdown data" />
              )}
            </ChartCard>
          </div>
        </>
      )}

      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold flex items-center gap-2">
            <Tag size={18} />
            Top Tags by Cost
          </h3>
          <Select value={tagRange} onValueChange={(v) => v && setTagRange(v)}>
            <SelectTrigger className="w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="24h">Last 24h</SelectItem>
              <SelectItem value="7d">Last 7d</SelectItem>
              <SelectItem value="30d">Last 30d</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {hasTagData ? (
          <div className="border rounded-md overflow-hidden">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Tag</TableHead>
                  <TableHead>Requests</TableHead>
                  <TableHead>Input Tokens</TableHead>
                  <TableHead>Output Tokens</TableHead>
                  <TableHead>Cost</TableHead>
                  <TableHead>Error Rate</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {tagAnalytics!.tags.map((entry, i) => (
                  <TableRow key={i}>
                    <TableCell>
                      <Badge variant="outline" className="font-mono text-xs">
                        {entry.tag}
                      </Badge>
                    </TableCell>
                    <TableCell>{entry.requests.toLocaleString()}</TableCell>
                    <TableCell>{entry.inputTokens.toLocaleString()}</TableCell>
                    <TableCell>{entry.outputTokens.toLocaleString()}</TableCell>
                    <TableCell>${(entry.costCents / 100).toFixed(2)}</TableCell>
                    <TableCell>
                      <Badge variant={entry.errorRate > 5 ? "destructive" : "secondary"}>
                        {entry.errorRate.toFixed(1)}%
                      </Badge>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        ) : (
          !hasData && !error && (
            <EmptyState
              title="No tag analytics"
              description="Tag breakdown appears once tagged API keys generate traffic."
            />
          )
        )}
      </div>
    </div>
  );
}
