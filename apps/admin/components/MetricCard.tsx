import { ArrowUpRight, ArrowDownRight, Minus } from "lucide-react";

interface MetricCardProps {
  title: string;
  value: string;
  description?: string;
  trend?: "up" | "down" | "flat";
  icon?: React.ReactNode;
}

export function MetricCard({ title, value, description, trend, icon }: MetricCardProps) {
  return (
    <div className="border rounded-md p-6 bg-background">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
          {title}
        </span>
        {icon && <span className="text-muted-foreground">{icon}</span>}
      </div>
      <div className="mt-3 text-3xl font-semibold tabular-nums tracking-tight">
        {value}
      </div>
      {description && (
        <p className="mt-1 text-xs text-muted-foreground flex items-center gap-1">
          {trend === "up" && <ArrowUpRight size={12} className="text-success" />}
          {trend === "down" && <ArrowDownRight size={12} className="text-destructive" />}
          {trend === "flat" && <Minus size={12} />}
          {description}
        </p>
      )}
    </div>
  );
}
