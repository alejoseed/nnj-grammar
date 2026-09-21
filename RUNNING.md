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

Serves the API on <http://127.0.0.1:7878>. The local catalog is embedded in the
binary; a `grammar/local/` directory in the working directory is loaded instead
of that embedded copy. Leave this running in its own terminal.

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

## Docker (backend only)

```bash
docker build -t nnj-grammar .
docker run -p 127.0.0.1:7878:7878 -v /var/log/nnj-grammar:/logs nnj-grammar
```

The image runs only the backend. The server logs with slog to stdout and,
because the image sets `NNJ_GRAMMAR_LOG_DIR=/logs`, also to one file per day
(`/logs/YYYY-MM-DD.log`). The same variable works outside Docker.

Inside the container the server binds `0.0.0.0:7878` (set via
`NNJ_GRAMMAR_BIND`, an explicit opt-out of the loopback-only guard) because
loopback is unreachable through a port mapping — publish on `127.0.0.1` as
above to keep the API local to the host. The local grammar catalog is already
embedded in the binary; mount a directory at `/app/grammar/local` to use a
different one in its place.
