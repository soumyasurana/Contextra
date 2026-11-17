import type { NextConfig } from "next";
import path from "path";

const nextConfig: NextConfig = {
  output: "standalone",
  turbopack: {
    root: path.resolve(__dirname),
  },
  async rewrites() {
    const gatewayUrl =
      process.env.INTERNAL_GATEWAY_URL ||
      process.env.GATEWAY_URL ||
      (process.env.NODE_ENV === "production" ? "http://gateway:3000" : "http://localhost:3000");

    return [
      {
        source: "/api/v1/:path*",
        destination: `${gatewayUrl}/api/v1/:path*`,
      },
      {
        source: "/health",
        destination: `${gatewayUrl}/health`,
      },
    ];
  },
};

export default nextConfig;
