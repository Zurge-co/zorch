"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { X, Plus } from "lucide-react";
import { toast } from "@/components/ui/toast";
import { ApiKeyTag } from "@/lib/api";

interface TagEditorProps {
  tags: ApiKeyTag[];
  onChange: (tags: ApiKeyTag[]) => void;
}

export function TagEditor({ tags, onChange }: TagEditorProps) {
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");

  const addTag = () => {
    const k = newKey.trim().toLowerCase();
    const v = newValue.trim();
    if (!k || !v) return;
    if (tags.some((t) => t.key === k)) {
      toast.error(`Duplicate tag key: ${k}`);
      return;
    }
    if (tags.length >= 16) {
      toast.error("Maximum 16 tags per key");
      return;
    }
    if (!/^[a-z0-9_-]+$/.test(k)) {
      toast.error("Tag key: lowercase a-z, 0-9, _, - only");
      return;
    }
    if (k.length > 32) {
      toast.error("Tag key: max 32 characters");
      return;
    }
    if (v.length > 128) {
      toast.error("Tag value: max 128 characters");
      return;
    }
    onChange([...tags, { key: k, value: v }]);
    setNewKey("");
    setNewValue("");
  };

  const removeTag = (idx: number) => {
    onChange(tags.filter((_, i) => i !== idx));
  };

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-2">
        {tags.map((tag, i) => (
          <span
            key={i}
            className="inline-flex items-center gap-1 rounded-md bg-muted px-2 py-1 text-xs font-mono"
          >
            {tag.key}:{tag.value}
            <button
              type="button"
              onClick={() => removeTag(i)}
              className="text-muted-foreground hover:text-foreground"
            >
              <X size={10} />
            </button>
          </span>
        ))}
      </div>
      <div className="flex gap-2">
        <Input
          placeholder="key"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          className="w-32 text-xs"
          onKeyDown={(e) => e.key === "Enter" && addTag()}
        />
        <Input
          placeholder="value"
          value={newValue}
          onChange={(e) => setNewValue(e.target.value)}
          className="w-40 text-xs"
          onKeyDown={(e) => e.key === "Enter" && addTag()}
        />
        <Button type="button" variant="outline" size="sm" onClick={addTag} disabled={!newKey.trim() || !newValue.trim()}>
          <Plus size={12} />
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        key:value pairs for cost attribution. Lowercase a-z, 0-9, _, - for keys. Max 16 tags.
      </p>
    </div>
  );
}
