"use client";

import React from "react";

interface ChartCardProps {
  title: string;
  description: string;
  children: React.ReactNode;
  className?: string;
}

export function ChartCard({ title, description, children, className }: ChartCardProps) {
  return (
    <div className={`border rounded-md bg-background ${className ?? ""}`}>
      <div className="p-6 border-b border-border">
        <h3 className="text-base font-semibold">{title}</h3>
        <p className="text-sm text-muted-foreground mt-1">{description}</p>
      </div>
      <div className="h-[320px] w-full px-6 pb-6 pt-6">
        {children}
      </div>
    </div>
  );
}
