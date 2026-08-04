CI builds `dist/web/` and publishes it to GitHub Pages on every push to `main`.
One-time setup: repo **Settings → Pages → Source: "GitHub Actions"**. Releases
also carry native binaries and a zipped web bundle.

Or skip Pages entirely and copy `dist/web/` into a static site. It is a folder of files with no external requests; it will run
anywhere you can put a folder of files.
