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

## Layout arrangements (Postgres)

Students can send their hand arrangements from the frontend's Layout panel.
The server stores them in Postgres when `NNJ_GRAMMAR_DATABASE_URL` is set and
creates its tables on startup. Without it, `POST /api/layouts` answers 503 and
everything else works as before.

```bash
NNJ_GRAMMAR_DATABASE_URL=postgres://user:password@127.0.0.1:5432/nnj_grammar \
NNJ_GRAMMAR_CLIENT_SALT=some-long-random-string \
cargo run --bin nnj-grammar-server
```

A submission needs an open class code. Open one per class session:

Every submission is checked against a fresh analysis of its sentence and
stored as `pending`. To review:

```sql
SELECT id, created_at, class_code, nickname, left(device_hash, 8) AS device, sentence,
       moved_nodes, displacement_px, order_breaks, status
FROM layout_submissions
ORDER BY order_breaks DESC, displacement_px DESC;

-- Several nicknames from one device:
SELECT device_hash, array_agg(DISTINCT nickname), count(*)
FROM layout_submissions GROUP BY device_hash HAVING count(DISTINCT nickname) > 1;

UPDATE layout_submissions SET status = 'rejected', review_note = 'scrambled'
WHERE id = 42;
```
