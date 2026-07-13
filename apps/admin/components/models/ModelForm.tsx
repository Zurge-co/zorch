"use client";

import React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";

export interface ModelFormData {
  publicName: string;
  isActive: boolean;
}

interface ModelFormProps {
  defaultValues?: Partial<ModelFormData>;
  onSubmit: (data: ModelFormData) => void;
  onCancel: () => void;
  submitLabel: string;
  loading?: boolean;
}

export function ModelForm({ defaultValues, onSubmit, onCancel, submitLabel, loading }: ModelFormProps) {
  const [publicName, setPublicName] = React.useState(defaultValues?.publicName ?? "");
  const [isActive, setIsActive] = React.useState(defaultValues?.isActive ?? true);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!publicName.trim()) return;
    onSubmit({ publicName: publicName.trim(), isActive });
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6 max-w-2xl">
      <div className="space-y-2">
        <Label htmlFor="publicName">Public Name</Label>
        <Input
          id="publicName"
          value={publicName}
          onChange={(e) => setPublicName(e.target.value)}
          placeholder="gpt5"
          disabled={loading}
          required
        />
        <p className="text-xs text-muted-foreground">The public name clients will use in their requests.</p>
      </div>

      <div className="flex items-center gap-3 rounded-lg border border-border bg-muted/40 px-4 py-3 w-fit">
        <Switch id="isActive" checked={isActive} onCheckedChange={setIsActive} disabled={loading} />
        <Label htmlFor="isActive" className="text-sm font-medium">
          Active
        </Label>
      </div>

      <div className="flex items-center gap-3 pt-2">
        <Button type="button" variant="outline" onClick={onCancel} disabled={loading}>
          Cancel
        </Button>
        <Button type="submit" disabled={loading || !publicName.trim()}>
          {loading ? "Saving..." : submitLabel}
        </Button>
      </div>
    </form>
  );
}
