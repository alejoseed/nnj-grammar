# Running nnj-grammar

The app is two processes:

- **Backend** — the Rust analysis API. Tokenizes and matches grammar.
- **Frontend** — the web UI in your browser. Talks to the backend.

You need both running at the same time.

## Backend

From the repo root:

```bash
cargo run --bin nnj-grammar-server
```

Serves the API on <http://127.0.0.1:7878>. It auto-loads `grammar/local/` if
present. Leave this running in its own terminal.

## Frontend

In a second terminal:

```bash
mise exec node@26 -- npm --prefix web run dev
```

Serves the UI on <http://localhost:5173>. Open that URL in your browser.
Requests to `/api/*` are proxied to the backend automatically.

## Both at once

```bash
./dev.sh
```

Starts both together. Press `Ctrl+C` once to stop both.
