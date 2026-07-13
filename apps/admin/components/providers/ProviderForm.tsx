"use client";

import React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ProviderAuthType } from "@/lib/api";

const AUTH_TYPES: Array<{ value: ProviderAuthType; label: string; suggestedBaseUrl: string }> = [
  { value: "bearer", label: "Bearer Authorization", suggestedBaseUrl: "https://api.openai.com/v1" },
  { value: "anthropic", label: "Anthropic", suggestedBaseUrl: "https://api.anthropic.com/v1" },
  { value: "custom", label: "Custom Header", suggestedBaseUrl: "https://api.example.com/v1" },
];

const suggestedBaseUrlFor = (authType: ProviderAuthType) =>
  AUTH_TYPES.find((a) => a.value === authType)?.suggestedBaseUrl ?? "https://api.openai.com/v1";

export interface ProviderFormData {
  name: string;
  authType: ProviderAuthType;
  baseUrl: string;
  authHeaderName: string;
  authPrefix: string;
  isActive: boolean;
}

interface ProviderFormProps {
  defaultValues?: Partial<ProviderFormData>;
  onSubmit: (data: ProviderFormData) => void;
  onCancel: () => void;
  submitLabel: string;
  loading?: boolean;
}

export function ProviderForm({
  defaultValues,
  onSubmit,
  onCancel,
  submitLabel,
  loading,
}: ProviderFormProps) {
  const [name, setName] = React.useState(defaultValues?.name ?? "");
  const [authType, setAuthType] = React.useState<ProviderAuthType>(defaultValues?.authType ?? "bearer");
  const [baseUrl, setBaseUrl] = React.useState(defaultValues?.baseUrl ?? suggestedBaseUrlFor("bearer"));
  const [authHeaderName, setAuthHeaderName] = React.useState(defaultValues?.authHeaderName ?? "");
  const [authPrefix, setAuthPrefix] = React.useState(defaultValues?.authPrefix ?? "");
  const [isActive, setIsActive] = React.useState(defaultValues?.isActive ?? true);

  const handleAuthTypeChange = (next: ProviderAuthType) => {
    const previousSuggestion = suggestedBaseUrlFor(authType);
    setAuthType(next);
    setBaseUrl((current) => {
      if (!current.trim() || current.trim() === previousSuggestion) {
        return suggestedBaseUrlFor(next);
      }
      return current;
    });
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit({
      name: name.trim(),
      authType,
      baseUrl: baseUrl.trim(),
      authHeaderName: authHeaderName.trim(),
      authPrefix: authPrefix.trim(),
      isActive,
    });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6 max-w-2xl">
      <div className="space-y-2">
        <Label htmlFor="name">Name</Label>
        <Input
          id="name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. OpenAI"
          disabled={loading}
          required
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="authType">Auth Type</Label>
        <select
          id="authType"
          value={authType}
          onChange={(e) => handleAuthTypeChange(e.target.value as ProviderAuthType)}
          disabled={loading}
          className="h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
        >
          {AUTH_TYPES.map((item) => (
            <option key={item.value} value={item.value}>
              {item.label}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-2">
        <Label htmlFor="baseUrl">Base URL</Label>
        <Input
          id="baseUrl"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder={suggestedBaseUrlFor(authType)}
          disabled={loading}
          required
        />
        <p className="text-xs text-muted-foreground">
          Base URL should be the API version root, for example {suggestedBaseUrlFor(authType)}.
        </p>
      </div>

      {authType === "custom" && (
        <>
          <div className="space-y-2">
            <Label htmlFor="authHeaderName">Auth Header Name</Label>
            <Input
              id="authHeaderName"
              value={authHeaderName}
              onChange={(e) => setAuthHeaderName(e.target.value)}
              placeholder="x-api-key"
              disabled={loading}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="authPrefix">Auth Prefix (optional)</Label>
            <Input
              id="authPrefix"
              value={authPrefix}
              onChange={(e) => setAuthPrefix(e.target.value)}
              placeholder="Bearer"
              disabled={loading}
            />
          </div>
        </>
      )}

      <div className="flex items-center justify-between rounded-lg border border-border bg-muted/40 px-4 py-3">
        <div>
          <Label htmlFor="isActive" className="text-sm font-medium">
            Active
          </Label>
          <p className="text-xs text-muted-foreground">Enable routing to this provider</p>
        </div>
        <Switch id="isActive" checked={isActive} onCheckedChange={setIsActive} disabled={loading} />
      </div>

      <div className="flex items-center gap-3 pt-2">
        <Button type="button" variant="outline" onClick={onCancel} disabled={loading}>
          Cancel
        </Button>
        <Button type="submit" disabled={loading}>
          {loading ? "Saving..." : submitLabel}
        </Button>
      </div>
    </form>
  );
}
