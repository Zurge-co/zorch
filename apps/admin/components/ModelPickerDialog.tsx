"use client";

import * as React from "react";
import { Check, Search, X } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { LoadingSpinner } from "@/components/LoadingSpinner";
import { cn } from "@/lib/utils";

export interface ModelPickerDialogProps {
  open: boolean;
  onOpenChange: (next: boolean) => void;
  candidates: readonly string[];
  existingTags: readonly string[];
  /** When true, show an inline spinner instead of the list while upstream loads. */
  loading?: boolean;
  onAdd: (next: string[]) => void;
}

export function ModelPickerDialog({
  open,
  onOpenChange,
  candidates,
  existingTags,
  loading = false,
  onAdd,
}: ModelPickerDialogProps) {
  const existingSet = React.useMemo(
    () => new Set(existingTags),
    [existingTags],
  );

  const [query, setQuery] = React.useState("");
  const [selected, setSelected] = React.useState<Set<string>>(() => new Set());

  React.useEffect(() => {
    if (!open) {
      setQuery("");
      setSelected(new Set());
    }
  }, [open]);

  const selectable = React.useMemo(
    () => candidates.filter((m) => !existingSet.has(m)),
    [candidates, existingSet],
  );

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return candidates;
    return candidates.filter((m) => m.toLowerCase().includes(q));
  }, [candidates, query]);

  const toggle = React.useCallback(
    (model: string) => {
      if (existingSet.has(model)) return;
      setSelected((prev) => {
        const next = new Set(prev);
        if (next.has(model)) next.delete(model);
        else next.add(model);
        return next;
      });
    },
    [existingSet],
  );

  const selectAllVisible = React.useCallback(() => {
    setSelected((prev) => {
      const next = new Set(prev);
      for (const m of filtered) {
        if (!existingSet.has(m)) next.add(m);
      }
      return next;
    });
  }, [filtered, existingSet]);

  const clearSelection = React.useCallback(() => setSelected(new Set()), []);

  const handleConfirm = () => {
    const picked = Array.from(selected);
    if (picked.length === 0) return;
    const merged = [...existingTags];
    const seen = new Set(existingTags);
    for (const m of picked) {
      if (!seen.has(m)) {
        seen.add(m);
        merged.push(m);
      }
    }
    onAdd(merged);
    onOpenChange(false);
  };

  const handleOpenChange = (next: boolean) => {
    if (!next) setQuery("");
    onOpenChange(next);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-lg max-h-[90vh] flex flex-col gap-4 overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Select models</DialogTitle>
          <DialogDescription>
            {loading
              ? "Fetching model list from upstream…"
              : selectable.length === 0
                ? `${candidates.length} fetched — all already added`
                : `${selectable.length} available · ${selected.size} selected`}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="relative">
            <Search
              size={14}
              className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Filter by name…"
              className="pl-8 pr-8"
              autoFocus
              disabled={loading}
            />
            {query.length > 0 && (
              <button
                type="button"
                onClick={() => setQuery("")}
                aria-label="Clear filter"
                className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-sm p-1 text-muted-foreground transition-colors hover:text-foreground focus:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                <X size={12} />
              </button>
            )}
          </div>

          {selectable.length > 0 && !loading && (
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>
                {filtered.length === candidates.length
                  ? `Showing all ${candidates.length}`
                  : `Showing ${filtered.length} of ${candidates.length}`}
              </span>
              <div className="flex items-center gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  onClick={selectAllVisible}
                  disabled={filtered.length === 0}
                >
                  Select all
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  onClick={clearSelection}
                  disabled={selected.size === 0}
                >
                  Clear
                </Button>
              </div>
            </div>
          )}

          {loading ? (
            <div className="flex items-center justify-center py-8">
              <LoadingSpinner />
            </div>
          ) : candidates.length === 0 ? (
            <EmptyState
              variant="compact"
              title="No models returned"
              description="The upstream provider's /models endpoint returned an empty list."
            />
          ) : filtered.length === 0 ? (
            <EmptyState
              variant="compact"
              title="No matches"
              description={`Nothing in this provider matches "${query.trim()}".`}
            />
          ) : (
            <ul
              role="listbox"
              className="divide-y divide-border rounded-lg border border-border bg-background"
            >
              {filtered.map((model) => {
                const isExisting = existingSet.has(model);
                const isSelected = selected.has(model);
                return (
                  <li
                    key={model}
                    role="option"
                    aria-selected={isSelected}
                    aria-disabled={isExisting}
                    tabIndex={isExisting ? -1 : 0}
                    onClick={() => toggle(model)}
                    onKeyDown={(e) => {
                      if (e.key === " " || e.key === "Enter") {
                        e.preventDefault();
                        toggle(model);
                      }
                    }}
                    className={cn(
                      "flex cursor-pointer items-center justify-between gap-3 px-3 py-2 text-sm transition-colors",
                      isExisting
                        ? "cursor-not-allowed bg-muted/40 text-muted-foreground"
                        : isSelected
                          ? "bg-primary/5 hover:bg-primary/10"
                          : "hover:bg-muted/60",
                    )}
                  >
                    <span className="font-mono text-[12px] break-all">{model}</span>
                    <span className="flex shrink-0 items-center gap-2">
                      {isExisting && (
                        <Badge variant="secondary" className="font-normal">
                          Added
                        </Badge>
                      )}
                      <span
                        aria-hidden
                        className={cn(
                          "flex h-4 w-4 items-center justify-center rounded border",
                          isSelected
                            ? "border-primary bg-primary text-primary-foreground"
                            : "border-border bg-background",
                        )}
                      >
                        {isSelected && <Check size={12} strokeWidth={3} />}
                      </span>
                    </span>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        <DialogFooter className="-mx-4 -mb-4">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            onClick={handleConfirm}
            disabled={selected.size === 0 || loading}
          >
            {selected.size === 0 ? "Select models to add" : `Add ${selected.size} model${selected.size === 1 ? "" : "s"}`}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
