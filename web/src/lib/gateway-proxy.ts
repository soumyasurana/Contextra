import 'server-only';

const requestHeaders = ['x-api-key', 'authorization', 'x-request-id', 'content-type', 'accept'];
const responseHeaders = ['content-type', 'x-request-id', 'retry-after'];

export async function proxyGateway(request: Request): Promise<Response> {
  const incoming = new URL(request.url);
  const base = process.env.INTERNAL_GATEWAY_URL || process.env.GATEWAY_URL || 'http://127.0.0.1:3000';
  const headers = new Headers();
  for (const name of requestHeaders) {
    const value = request.headers.get(name);
    if (value) headers.set(name, value);
  }

  try {
    const gateway = new URL(base);
    gateway.pathname = gateway.pathname.replace(/\/+$/, '') + incoming.pathname;
    gateway.search = incoming.search;
    const options: RequestInit & { duplex?: 'half' } = {
      method: request.method,
      headers,
      cache: 'no-store',
      redirect: 'manual',
      signal: AbortSignal.any([request.signal, AbortSignal.timeout(65000)]),
    };
    if (request.body) {
      options.body = request.body;
      options.duplex = 'half';
    }

    const upstream = await fetch(gateway, options);
    const forwarded = new Headers({ 'cache-control': 'no-store' });
    for (const name of responseHeaders) {
      const value = upstream.headers.get(name);
      if (value) forwarded.set(name, value);
    }
    return new Response(upstream.body, { status: upstream.status, headers: forwarded });
  } catch {
    return Response.json({ error: { message: 'The web proxy could not reach the gateway.' } }, { status: 502 });
  }
}
