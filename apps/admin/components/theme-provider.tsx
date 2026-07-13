"use client";

import * as React from "react";
import { ThemeProvider as NextThemeProvider } from "next-themes";

/**
 * Context provider for managing the visual theme (light/dark/system) 
 * across the application using next-themes.
 * 
 * @param {React.ComponentProps<typeof NextThemeProvider>} props - Theme provider configuration.
 * @returns {JSX.Element} The theme provider wrapper.
 */
export function ThemeProvider({ children, ...props }: React.ComponentProps<typeof NextThemeProvider>) {
  return <NextThemeProvider {...props}>{children}</NextThemeProvider>;
}
