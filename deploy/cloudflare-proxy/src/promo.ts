/// <reference path="../promo-configuration.d.ts" />

type ByteRange = { offset: number; length: number };

// Browsers request byte ranges when starting playback or seeking through MP4s.
function byteRange(value: string | null, size: number): ByteRange | "unsatisfiable" | null {
  if (!value) return null;
  const match = /^bytes=(\d*)-(\d*)$/.exec(value.trim());
  // A server may ignore unsupported range formats, including multipart ranges.
  if (!match || (!match[1] && !match[2])) return null;
  if (!size) return "unsatisfiable";
  if (!match[1]) {
    const suffix = Number(match[2]);
    if (!suffix) return "unsatisfiable";
    const length = Math.min(suffix, size);
    return { offset: size - length, length };
  }
  const offset = Number(match[1]);
  const end = match[2] ? Math.min(Number(match[2]), size - 1) : size - 1;
  if (!Number.isSafeInteger(offset) || offset >= size || end < offset) return "unsatisfiable";
  return { offset, length: end - offset + 1 };
}

function headersFor(object: R2Object): Headers {
  const headers = new Headers();
  object.writeHttpMetadata(headers);
  headers.set("Content-Type", "video/mp4");
  headers.set("Content-Disposition", 'inline; filename="stellar-syndicates-promo-v09-1080p.mp4"');
  headers.set("Content-Length", String(object.size));
  headers.set("Accept-Ranges", "bytes");
  headers.set("ETag", object.httpEtag);
  headers.set("Last-Modified", object.uploaded.toUTCString());
  headers.set("Cache-Control", "public, max-age=31536000, immutable");
  headers.set("Access-Control-Allow-Origin", "*");
  headers.set("Access-Control-Expose-Headers", "Content-Length, Content-Range, Accept-Ranges, ETag");
  headers.set("X-Content-Type-Options", "nosniff");
  return headers;
}

export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url);
    if (!["GET", "HEAD"].includes(request.method)) {
      return new Response("Method not allowed", { status: 405, headers: { Allow: "GET, HEAD" } });
    }
    if (url.pathname === "/video.mp4") {
      return new Response(null, {
        status: 302,
        headers: { Location: `/${env.PROMO_VIDEO_KEY}`, "Cache-Control": "public, max-age=60" },
      });
    }
    if (url.pathname !== `/${env.PROMO_VIDEO_KEY}`) return env.ASSETS.fetch(request);

    // The public endpoint exposes only this versioned promo, never arbitrary keys.
    const metadata = await env.MEDIA.head(env.PROMO_VIDEO_KEY);
    if (!metadata) return new Response("Video not found", { status: 404 });
    const headers = headersFor(metadata);
    const ifNoneMatch = request.headers.get("If-None-Match");
    if (ifNoneMatch?.split(",").some(value => {
      const tag = value.trim().replace(/^W\//, "");
      return tag === "*" || tag === metadata.httpEtag;
    })) {
      headers.delete("Content-Length");
      return new Response(null, { status: 304, headers });
    }
    if (request.method === "HEAD") return new Response(null, { headers });

    const ifRange = request.headers.get("If-Range");
    const canUseRange = !ifRange || ifRange === metadata.httpEtag ||
      Date.parse(ifRange) >= Math.floor(metadata.uploaded.getTime() / 1000) * 1000;
    const range = canUseRange ? byteRange(request.headers.get("Range"), metadata.size) : null;
    if (range === "unsatisfiable") {
      headers.set("Content-Range", `bytes */${metadata.size}`);
      headers.set("Content-Length", "0");
      return new Response(null, { status: 416, headers });
    }
    const object = await env.MEDIA.get(env.PROMO_VIDEO_KEY, range ? { range } : {});
    if (!object) return new Response("Video not found", { status: 404 });
    if (range) {
      headers.set("Content-Range", `bytes ${range.offset}-${range.offset + range.length - 1}/${metadata.size}`);
      headers.set("Content-Length", String(range.length));
    }
    // Stream from R2 so playback never buffers the full video in Worker memory.
    return new Response(object.body, { status: range ? 206 : 200, headers });
  },
} satisfies ExportedHandler<PromoEnv>;
