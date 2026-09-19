# Upload120

**Local website for TikTok-focused MP4 timing and metadata patches.**

Upload120 is now a website-only TikTok helper. Drop an MP4, MOV, or M4V file into the static web page, choose a TikTok method, and download a patched copy. The video stays on your device, and the frame data is not uploaded or re-encoded by Upload120.

---

## Current Website

- **Local processing** - all patching runs in the browser.
- **TikTok methods** - TikTok Web Signal, Balanced Sync, and Classic Force.
- **Batch queue** - add multiple files and process them from one page.
- **TikTok readiness notes** - checks file size, shape, resolution, and FPS signals before processing.
- **Auto multiplier** - pick signal strength automatically from the detected FPS.
- **Method guide** - `docs/method-guide.html` explains exactly when humans and AI support should use each method.
- **Sources page** - `docs/sources.html` lists the public research sources used for method decisions.
- **No install** - open `docs/index.html` directly or host the `docs/` folder as a static site.

For best results, upload the patched file from TikTok Web on a computer. Use MP4 or MOV, vertical 1080 x 1920 when possible, and avoid in-upload edits, cropping, music, or mobile reposting because those steps can re-encode the file.

---

## Usage

1. Open `docs/index.html` in a browser or use the hosted static site.
2. Drop one or more video files into the page.
3. Choose a TikTok method and multiplier.
4. Click **Process** and download the patched file.

Everything runs locally. No telemetry, no server upload, and no account connection is required.

## Methods

- **TikTok Web Signal** - recommended first try. Adds a normal-speed edit list and iTunes-style metadata without changing local playback speed.
- **Balanced Sync** - strong fallback when TikTok Web Signal does not work. Changes movie and track timing, then adds an edit-list guard and local metadata.
- **Classic Force** - legacy fallback. Changes movie and video track timing like older patchers; desktop playback can look slow.

Removed methods:

- **Header Lite** was removed because movie-header-only timing is not a defensible TikTok FPS method.
- **API Clean** was removed because metadata-only tagging is not an FPS method.

---

## Development

```bash
npm test
```

The tests cover the browser patcher and website wiring. There is no native app build pipeline in the current project.

---

## Sources

See `docs/sources.html` for the current source list behind the TikTok method decisions.

---

## License

MIT - see [LICENSE](LICENSE).
