# Promo hosting

The 74-second promo is served at <https://promo.stellarsyndicates.com/>.
The stable direct-video link is <https://promo.stellarsyndicates.com/video.mp4>.

This is an independent `stellar-syndicates-promo` Worker. `wrangler.promo.jsonc`
defines its custom domain, static watch page, and private R2 bucket. The game
gateway and play host have separate deployments.

The watch page and its existing game artwork live in `promo-public/`. The MP4
lives in the `stellar-syndicates-promo` R2 bucket. The Worker exposes one exact
versioned key, supports byte-range requests for seeking, and streams the file
without buffering it in memory. `video.mp4` redirects to the current version.
The bucket itself has no public domain or development URL.

## Current video

- Object: `stellar-syndicates-promo-v09-web.mp4`
- Source: `client/art-src/promo-slideshow-v1/output/stellar-syndicates-promo-v09-1080p.mp4`
- Streaming copy: `client/art-src/promo-slideshow-v1/output/stellar-syndicates-promo-v09-web.mp4`, remuxed without re-encoding so its index is available before playback.
- 1920 × 1080, 74 seconds, silent; the faster cut with corrected planet orientation.

## Updating

Run commands from `deploy/cloudflare-proxy`.

For a browser-recorded MP4 on macOS, prepare a streaming copy first. This
repackages the existing video without changing its encoded frames:

```sh
swift scripts/prepare-promo-video.swift /absolute/path/to/recording.mp4 /absolute/path/to/NEW-VIDEO.mp4
```

1. Upload a new **versioned** filename so immutable caches remain valid:

   ```sh
   ./node_modules/.bin/wrangler r2 object put stellar-syndicates-promo/NEW-VIDEO.mp4 \
     --file /absolute/path/to/NEW-VIDEO.mp4 --content-type video/mp4 \
     --cache-control 'public, max-age=31536000, immutable' --remote -c wrangler.promo.jsonc
   ```

2. Update `PROMO_VIDEO_KEY` and its `run_worker_first` path in
   `wrangler.promo.jsonc`, the video/metadata URLs and duration in
   `promo-public/index.html`, and the download filename in `src/promo.ts`.
   Use a frame of that video for `promo-public/poster.jpg`.
3. Run `npm run cf-typegen:promo`, `npm run test:promo`, and
   `./node_modules/.bin/tsc --noEmit`.
4. Run `npm run deploy:promo`. Wrangler manages DNS and TLS for the custom domain.
5. Verify the watch page, normal playback, seeking, and a range response:

   ```sh
   curl -I https://promo.stellarsyndicates.com/
   curl -sS -D - -o /dev/null -H 'Range: bytes=0-1023' \
     https://promo.stellarsyndicates.com/NEW-VIDEO.mp4
   ```

The range request should return `206`, `Content-Type: video/mp4`,
`Content-Length: 1024`, and a matching `Content-Range`.

Deploying this Worker publishes only `promo-public/` and `src/promo.ts`, not the
full source slideshow, local exports, PowerPoint, or other game assets.
