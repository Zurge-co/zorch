import { NextRequest, NextResponse } from "next/server";

/**
 * Server-only proxy. Reads `ZORCH_ADMIN_SECRET` from process.env and injects
 * it as `X-Admin-Secret` on every upstream call. Never move this env read
 * or fetch call into a client component — the secret would leak.
 */

const TARGET = (process.env.ZORCH_API_URL ?? "http://localhost:8081").replace(
  /\/+$/,
  "",
);

function adminSecret(): string | null {
  const v = process.env.ZORCH_ADMIN_SECRET;
  return v && v.length > 0 ? v : null;
}

async function forward(req: NextRequest, pathSegments: string[]): Promise<Response> {
  const secret = adminSecret();
  if (!secret) {
    return NextResponse.json(
      { error: "ZORCH_ADMIN_SECRET is not configured on the admin server" },
      { status: 500 },
    );
  }

  const tail = pathSegments.join("/");
  const search = req.nextUrl.search ?? "";
  const url = `${TARGET}/api/${tail}${search}`;

  const headers = new Headers(req.headers);
  headers.set("X-Admin-Secret", secret);
  headers.delete("host");
  headers.delete("connection");
  headers.delete("content-length");

  const init: RequestInit = {
    method: req.method,
    headers,
    body: ["GET", "HEAD"].includes(req.method)
      ? undefined
      : await req.arrayBuffer(),
    redirect: "manual",
  };

  const upstream = await fetch(url, init);

  const outHeaders = new Headers(upstream.headers);
  for (const h of ["transfer-encoding", "connection", "keep-alive"]) {
    outHeaders.delete(h);
  }

  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: outHeaders,
  });
}

type Ctx = { params: Promise<{ path: string[] }> };

export async function GET(req: NextRequest, ctx: Ctx) {
  return forward(req, (await ctx.params).path);
}
export async function POST(req: NextRequest, ctx: Ctx) {
  return forward(req, (await ctx.params).path);
}
export async function PUT(req: NextRequest, ctx: Ctx) {
  return forward(req, (await ctx.params).path);
}
export async function PATCH(req: NextRequest, ctx: Ctx) {
  return forward(req, (await ctx.params).path);
}
export async function DELETE(req: NextRequest, ctx: Ctx) {
  return forward(req, (await ctx.params).path);
}
export async function OPTIONS() {
  return new Response(null, {
    status: 204,
    headers: {
      "access-control-allow-origin": "*",
      "access-control-allow-methods": "GET,POST,PUT,PATCH,DELETE,OPTIONS",
      "access-control-allow-headers": "content-type,x-admin-secret,authorization",
    },
  });
}
