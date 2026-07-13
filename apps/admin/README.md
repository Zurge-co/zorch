# Zorch Admin Dashboard

The Zorch Admin Dashboard is a comprehensive management interface for the Zorch AI Key Orchestration Platform. It allows administrators to monitor system health, manage API keys, configure AI providers, and analyze usage patterns.

## Project Overview

Built with **Next.js 14 (App Router)** and **shadcn/ui**, the dashboard provides a high-performance, responsive interface for managing the Zorch ecosystem.

### Tech Stack
- **Framework**: Next.js 14
- **Language**: TypeScript
- **Styling**: Tailwind CSS
- **UI Components**: shadcn/ui (Radix UI)
- **Icons**: Lucide React
- **Theme**: next-themes (Light/Dark mode)

## Architecture

### Directory Structure
- `app/`: Next.js App Router pages and layouts.
- `components/`: Shared UI components.
  - `ui/`: Low-level primitive components from shadcn/ui.
  - Others: Feature-specific components (e.g., `Sidebar`, `ThemeProvider`).
- `lib/`: Utility functions and API clients.
  - `api.ts`: Centralized API client for interacting with the Zorch backend.
  - `utils.ts`: Tailwind class merging and helper functions.

### Data Flow
The dashboard follows a unidirectional data flow:
`Next.js Page` $\rightarrow$ `lib/api.ts` $\rightarrow$ `Zorch Backend (Rust)` $\rightarrow$ `Database (PostgreSQL / ClickHouse)`

1. **Request**: A page component calls an exported function from `lib/api.ts` inside a `useEffect` or Server Component.
2. **Client**: `api.ts` uses the `fetch` API to call the `/api/v1/admin/*` endpoints.
3. **Backend**: The Zorch Rust backend processes the request, performing authentication and authorization.
4. **Storage**: 
   - **PostgreSQL**: Used for relational data like API keys, provider configurations, and user accounts.
   - **ClickHouse**: Used for high-volume telemetry and analytics (token usage, latency, cost trends).
5. **Response**: Data is returned as JSON and typed via TypeScript interfaces in `api.ts`.

## API Integration

### Configuration
The dashboard requires the following environment variable to connect to the backend:

| Variable | Description | Default |
|---|---|---|
| `NEXT_PUBLIC_ZORCH_API_URL` | The base URL of the Zorch backend API | `http://localhost:8080` |

### Integration Pattern
API calls are encapsulated in `lib/api.ts` using a `handleFetch` wrapper that ensures consistent header management and error handling.

**Example Pattern:**
```typescript
async function handleFetch<T>(url: string): Promise<T> {
  const response = await fetch(url, { headers: { "Content-Type": "application/json" } });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  return response.json();
}
```

### Admin Endpoint Contracts
| Function | Endpoint | Method | Description |
|---|---|---|---|
| `fetchDashboardStats()` | `/api/v1/admin/dashboard` | GET | Real-time RPM, TPM, and recent activity |
| `fetchApiKeys()` | `/api/v1/admin/api-keys` | GET | List of all managed API keys |
| `fetchProviders()` | `/api/v1/admin/providers` | GET | List of configured AI providers and health status |
| `fetchAnalytics()` | `/api/v1/admin/analytics` | GET | Aggregated token usage, costs, and latency |

## Build and Development

### Prerequisites
- Node.js 20+
- npm or yarn

### Installation
```bash
cd apps/admin
npm install
```

### Running Locally
```bash
npm run dev
```
The dashboard will be available at `http://localhost:3000`.

### Production Build
```bash
npm run build
npm start
```

## Component Conventions

### shadcn/ui Usage
The project uses shadcn/ui for consistent design. 
- **Customization**: Modify components in `components/ui/` to change global styles.
- **Composition**: Combine primitives (e.g., `Card`, `Table`, `Badge`) to launder complex views.
- **Styling**: Use Tailwind CSS for layouts and `cn()` utility for conditional classes.

### State Management
- **Local State**: Managed via `useState` and `useEffect` within pages.
- **Global State**: The `ThemeProvider` manages the visual theme across the application.
