import * as React from "react";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
  /**
   * "default" — page-level center stack with icon, title, description.
   * "table-cell" — renders a raw `<td>` (caller wraps it in `<tr>`).
   * "compact" — chart-card level, smaller padding, no padded wrapper.
   */
  variant?: "default" | "table-cell" | "compact";
  colSpan?: number;
  className?: string;
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  variant = "default",
  colSpan,
  className,
}: EmptyStateProps) {
  if (variant === "table-cell") {
    return (
      <td colSpan={colSpan} className={cn("text-center py-12 text-muted-foreground", className)}>
        <div className="flex flex-col items-center gap-2">
          {icon}
          <div className="space-y-1">
            <p className="text-sm font-medium">{title}</p>
            {description && <p className="text-xs">{description}</p>}
          </div>
          {action}
        </div>
      </td>
    );
  }

  if (variant === "compact") {
    return (
      <div className={cn("flex h-full items-center justify-center text-muted-foreground", className)}>
        <div className="space-y-1 text-center text-sm">
          <p>{title}</p>
          {description && <p className="text-xs">{description}</p>}
        </div>
      </div>
    );
  }

  return (
    <div className={cn("flex flex-col items-center justify-center gap-3 py-12 text-muted-foreground", className)}>
      {icon && (
        <span className="text-muted-foreground/70 [&_svg]:h-6 [&_svg]:w-6">{icon}</span>
      )}
      <div className="space-y-1 text-center">
        <p className="text-sm font-medium">{title}</p>
        {description && <p className="text-xs text-muted-foreground">{description}</p>}
      </div>
      {action}
    </div>
  );
}
