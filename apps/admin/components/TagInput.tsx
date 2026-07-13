"use client";

import * as React from "react";
import { X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export interface TagInputProps {
  value: string[];
  onChange: (next: string[]) => void;
  placeholder?: string;
  disabled?: boolean;
  id?: string;
  className?: string;
}

export function TagInput({
  value,
  onChange,
  placeholder,
  disabled,
  id,
  className,
}: TagInputProps) {
  const [inputValue, setInputValue] = React.useState("");

  const commit = (raw: string) => {
    if (disabled) return;
    const parts = raw.split(",").map((p) => p.trim()).filter(Boolean);
    if (parts.length === 0) return;
    const seen = new Set(value);
    const next = [...value];
    for (const p of parts) {
      if (!seen.has(p)) {
        seen.add(p);
        next.push(p);
      }
    }
    if (next.length !== value.length) onChange(next);
    setInputValue("");
  };

  const removeAt = (idx: number) => {
    if (disabled) return;
    const next = value.slice();
    next.splice(idx, 1);
    onChange(next);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") { e.preventDefault(); commit(inputValue); }
    else if (e.key === "," && inputValue.length > 0) { e.preventDefault(); commit(inputValue); }
    else if (e.key === "Backspace" && inputValue === "" && value.length > 0) {
      e.preventDefault();
      removeAt(value.length - 1);
    }
  };

  return (
    <div
      className={cn(
        "flex min-h-9 w-full flex-wrap items-center gap-1.5 rounded-lg border border-input bg-background px-2.5 py-1.5 text-sm transition-colors focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/40",
        disabled && "cursor-not-allowed opacity-50",
        className,
      )}
    >
      {value.map((tag, idx) => (
        <Badge
          key={`${tag}-${idx}`}
          variant="secondary"
          className="gap-1 font-mono text-xs"
        >
          {tag}
          {!disabled && (
            <button
              type="button"
              onClick={() => removeAt(idx)}
              aria-label={`Remove ${tag}`}
              className="ml-0.5 rounded-sm text-muted-foreground transition-colors hover:text-destructive focus:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            >
              <X size={12} />
            </button>
          )}
        </Badge>
      ))}
      <input
        id={id}
        type="text"
        value={inputValue}
        onChange={(e) => setInputValue(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        placeholder={value.length === 0 ? placeholder : undefined}
        className="min-w-[120px] flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed"
        autoComplete="off"
        spellCheck={false}
      />
    </div>
  );
}
