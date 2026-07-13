import type { Metadata } from "next";
import "./globals.css";
import { ThemeProvider } from "@/components/theme-provider";
import { Sidebar } from "@/components/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ToastProvider } from "@/components/ui/toast";
import { ConfirmDialogProvider } from "@/components/ui/confirm-dialog";
import { Badge } from "@/components/ui/badge";

export const metadata: Metadata = {
  title: "Zorch Admin Dashboard",
  description: "AI Key Orchestration Platform Administration",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head />
      <body>
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          <ToastProvider>
            <ConfirmDialogProvider>
              <TooltipProvider>
                <div className="flex min-h-screen bg-background">
                  <Sidebar />
                  <main className="flex-1 lg:ml-64">
                    <header className="h-14 border-b border-border flex items-center justify-between px-6 sticky top-0 bg-background/80 backdrop-blur-md z-20">
                      <h1 className="font-semibold text-sm text-foreground">Admin Console</h1>
                      <div className="flex items-center gap-4">
                        <div className="flex items-center gap-2 text-sm text-muted-foreground">
                          System Status:
                          <Badge variant="success">Healthy</Badge>
                        </div>
                        <div className="w-8 h-8 rounded-md bg-muted flex items-center justify-center text-xs font-medium text-muted-foreground">
                          AD
                        </div>
                      </div>
                    </header>
                    <div className="px-6 py-8 lg:px-8">
                      {children}
                    </div>
                  </main>
                </div>
              </TooltipProvider>
            </ConfirmDialogProvider>
          </ToastProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
