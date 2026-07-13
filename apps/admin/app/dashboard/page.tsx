"use client";

import React from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import {
  Activity,
  Cpu,
  Zap,
  AlertCircle,
  Inbox,
} from "lucide-react";
import { fetchDashboardStats, RecentActivity } from "@/lib/api";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { ErrorMessage } from "@/components/ErrorMessage";
import { PageHeader } from "@/components/PageHeader";
import { MetricCard } from "@/components/MetricCard";
import { EmptyState } from "@/components/ui/empty-state";
import { useFetchData } from "@/lib/useFetchData";

function formatTrend(percent: number): string {
  if (!Number.isFinite(percent) || percent === 0) return "0%";
  const sign = percent > 0 ? "+" : "";
  return `${sign}${percent.toFixed(1)}%`;
}

function trendDirection(percent: number): "up" | "down" | "flat" {
  if (!Number.isFinite(percent) || Math.abs(percent) < 0.5) return "flat";
  return percent > 0 ? "up" : "down";
}

export default function DashboardPage() {
  const { data, loading, error } = useFetchData(fetchDashboardStats);

  if (loading) {
    return (
      <div className="space-y-8">
        <PageHeader
          title="Dashboard Overview"
          description="Real-time orchestration metrics and system health."
        />
        <LoadingSpinner />
      </div>
    );
  }

  const stats = data?.stats;
  const requestsTrend = stats?.requestsTrendPercent ?? 0;
  const tokensTrend = stats?.tokensTrendPercent ?? 0;
  const errorTrend = stats?.errorRateTrendPercent ?? 0;
  const requests24h = stats?.requestsLast24h ?? 0;
  const hasTraffic = requests24h > 0;

  const requestsDescription = hasTraffic
    ? `${formatTrend(requestsTrend)} vs prior hour`
    : "No traffic in last 24h";
  const tokensDescription = hasTraffic
    ? `${formatTrend(tokensTrend)} vs prior hour`
    : "No traffic in last 24h";
  const errorDescription = hasTraffic
    ? `${formatTrend(errorTrend)} vs prior hour`
    : "Awaiting traffic";

  return (
    <div className="space-y-8">
      <PageHeader
        title="Dashboard Overview"
        description="Real-time orchestration metrics and system health."
      />

      {error && <ErrorMessage message={error} />}

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <MetricCard
          title="Requests/Min"
          value={stats?.requestsPerMinute || "0"}
          description={requestsDescription}
          trend={trendDirection(requestsTrend)}
          icon={<Activity size={16} />}
        />
        <MetricCard
          title="Tokens/Min"
          value={stats?.tokensPerMinute || "0"}
          description={tokensDescription}
          trend={trendDirection(tokensTrend)}
          icon={<Zap size={16} />}
        />
        <MetricCard
          title="Error Rate"
          value={stats?.errorRate || "0%"}
          description={errorDescription}
          trend={trendDirection(errorTrend)}
          icon={<AlertCircle size={16} />}
        />
        <MetricCard
          title="Active Providers"
          value={(stats?.activeProviders || 0).toString()}
          description={hasTraffic ? `${requests24h} requests / 24h` : "Awaiting traffic"}
          trend="flat"
          icon={<Cpu size={16} />}
        />
      </div>

      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-base font-semibold text-foreground">Recent Activity</h2>
          <span className="text-xs text-muted-foreground">Showing last 50 events</span>
        </div>

        <div className="border rounded-md overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Key ID</TableHead>
                <TableHead>Provider</TableHead>
                <TableHead>Model</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Latency</TableHead>
                <TableHead className="text-right">Timestamp</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(!data?.recentActivity || data.recentActivity.length === 0) ? (
                <TableRow>
                  <EmptyState variant="table-cell" colSpan={6} icon={<Inbox size={24} />} title="No recent activity found" />
                </TableRow>
              ) : (
                data.recentActivity.map((activity: RecentActivity) => (
                  <TableRow key={activity.id}>
                    <TableCell className="font-mono text-xs">{activity.keyId}</TableCell>
                    <TableCell>{activity.provider}</TableCell>
                    <TableCell>{activity.model}</TableCell>
                    <TableCell>
                      <Badge variant={activity.status === "success" ? "success" : "destructive"}>
                        {activity.status}
                      </Badge>
                    </TableCell>
                    <TableCell>{activity.latency}</TableCell>
                    <TableCell className="text-right text-muted-foreground text-xs">
                      {new Date(activity.timestamp).toLocaleTimeString()}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>
    </div>
  );
}
