"use client";

import React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { TagEditor } from "./TagEditor";
import { ApiKeyTag } from "@/lib/api";

function detectBrowserTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
}

const TIMEZONES = [
  "UTC",
  "America/New_York",
  "America/Chicago",
  "America/Denver",
  "America/Los_Angeles",
  "Europe/London",
  "Europe/Berlin",
  "Europe/Paris",
  "Asia/Tokyo",
  "Asia/Shanghai",
  "Asia/Bangkok",
  "Asia/Singapore",
  "Australia/Sydney",
];

function timezoneOptions(detected: string): string[] {
  const base = [...TIMEZONES];
  if (detected !== "UTC" && !base.includes(detected)) {
    base.unshift(detected);
  }
  return base;
}

export interface ApiKeyFormData {
  name: string;
  scopes: string;
  expiresInDays: string;
  tags: ApiKeyTag[];
  allowedHoursStart: string;
  allowedHoursEnd: string;
  windowTimezone: string;
  requestsPerMinute: string;
  requestsPerDay: string;
  maxSpendUsd: string;
  allowedModels: string;
}

interface ApiKeyFormProps {
  mode: "create" | "edit";
  defaultValues?: Partial<ApiKeyFormData>;
  onSubmit: (data: ApiKeyFormData) => void;
  onCancel: () => void;
  submitLabel: string;
  loading?: boolean;
}

export function ApiKeyForm({ mode, defaultValues, onSubmit, onCancel, submitLabel, loading }: ApiKeyFormProps) {
  const detectedTz = detectBrowserTimezone();
  const [name, setName] = React.useState(defaultValues?.name ?? "");
  const [scopes, setScopes] = React.useState(defaultValues?.scopes ?? "");
  const [expiresInDays, setExpiresInDays] = React.useState(defaultValues?.expiresInDays ?? "");
  const [tags, setTags] = React.useState<ApiKeyTag[]>(defaultValues?.tags ?? []);
  const [allowedHoursStart, setAllowedHoursStart] = React.useState(defaultValues?.allowedHoursStart ?? "");
  const [allowedHoursEnd, setAllowedHoursEnd] = React.useState(defaultValues?.allowedHoursEnd ?? "");
  const [windowTimezone, setWindowTimezone] = React.useState(defaultValues?.windowTimezone ?? detectedTz);
  const [requestsPerMinute, setRequestsPerMinute] = React.useState(defaultValues?.requestsPerMinute ?? "");
  const [requestsPerDay, setRequestsPerDay] = React.useState(defaultValues?.requestsPerDay ?? "");
  const [maxSpendUsd, setMaxSpendUsd] = React.useState(defaultValues?.maxSpendUsd ?? "");
  const [allowedModels, setAllowedModels] = React.useState(defaultValues?.allowedModels ?? "");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit({
      name: name.trim(),
      scopes: scopes.trim(),
      expiresInDays: expiresInDays.trim(),
      tags,
      allowedHoursStart: allowedHoursStart.trim(),
      allowedHoursEnd: allowedHoursEnd.trim(),
      windowTimezone,
      requestsPerMinute: requestsPerMinute.trim(),
      requestsPerDay: requestsPerDay.trim(),
      maxSpendUsd: maxSpendUsd.trim(),
      allowedModels: allowedModels.trim(),
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-8 max-w-3xl">
      <div className="space-y-2">
        <Label htmlFor="name">
          Key Name <span className="text-destructive">*</span>
        </Label>
        <Input
          id="name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. prod-mobile-app"
          disabled={loading}
          required
        />
      </div>

      {mode === "create" && (
        <>
          <div className="space-y-2">
            <Label htmlFor="scopes">Scopes (comma-separated, optional)</Label>
            <Input
              id="scopes"
              value={scopes}
              onChange={(e) => setScopes(e.target.value)}
              placeholder="default"
              disabled={loading}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="expires">Expires In Days (optional)</Label>
            <Input
              id="expires"
              type="number"
              min={1}
              value={expiresInDays}
              onChange={(e) => setExpiresInDays(e.target.value)}
              placeholder="30"
              disabled={loading}
            />
          </div>
        </>
      )}

      <div className="space-y-2">
        <Label>Tags (optional)</Label>
        <TagEditor tags={tags} onChange={setTags} />
      </div>

      <div className="space-y-2">
        <Label>Allowed Hours (optional)</Label>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
          <div>
            <Label className="text-xs text-muted-foreground">Start (0-23)</Label>
            <Input
              type="number"
              min={0}
              max={23}
              value={allowedHoursStart}
              onChange={(e) => setAllowedHoursStart(e.target.value)}
              placeholder="9"
              disabled={loading}
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">End (0-23)</Label>
            <Input
              type="number"
              min={0}
              max={23}
              value={allowedHoursEnd}
              onChange={(e) => setAllowedHoursEnd(e.target.value)}
              placeholder="18"
              disabled={loading}
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Timezone</Label>
            <select
              value={windowTimezone}
              onChange={(e) => setWindowTimezone(e.target.value)}
              disabled={loading}
              className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              {timezoneOptions(windowTimezone).map((tz) => (
                <option key={tz} value={tz}>
                  {tz}
                </option>
              ))}
            </select>
          </div>
        </div>
        <p className="text-xs text-muted-foreground">
          Leave blank for 24/7 access. Both start and end must be set together.
        </p>
      </div>

      <div className="space-y-2">
        <Label>Governance Limits (optional)</Label>
        <div className="grid grid-cols-2 gap-2">
          <div>
            <Label className="text-xs text-muted-foreground">RPM</Label>
            <Input
              type="number"
              min={1}
              value={requestsPerMinute}
              onChange={(e) => setRequestsPerMinute(e.target.value)}
              placeholder="100"
              disabled={loading}
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">RPD</Label>
            <Input
              type="number"
              min={1}
              value={requestsPerDay}
              onChange={(e) => setRequestsPerDay(e.target.value)}
              placeholder="10000"
              disabled={loading}
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Budget ($)</Label>
            <Input
              type="number"
              min={0}
              step="0.01"
              value={maxSpendUsd}
              onChange={(e) => setMaxSpendUsd(e.target.value)}
              placeholder="100"
              disabled={loading}
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Models</Label>
            <Input
              value={allowedModels}
              onChange={(e) => setAllowedModels(e.target.value)}
              placeholder="gpt-4o, claude-3-5-sonnet"
              disabled={loading}
            />
          </div>
        </div>
        <p className="text-xs text-muted-foreground">Leave blank to use default limits.</p>
      </div>

      <div className="flex items-center gap-3 pt-2">
        <Button type="button" variant="outline" onClick={onCancel} disabled={loading}>
          Cancel
        </Button>
        <Button type="submit" disabled={loading || !name.trim()}>
          {loading ? "Saving..." : submitLabel}
        </Button>
      </div>
    </form>
  );
}
