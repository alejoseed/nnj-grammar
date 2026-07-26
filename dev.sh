#!/usr/bin/env bash
# Start the backend (Rust API) and frontend (web UI) together.
# Press Ctrl+C once to stop both.
set -euo pipefail

cd "$(dirname "$0")"

cargo run --bin nnj-grammar-server &
backend=$!

mise exec node@26 -- npm --prefix web run dev &
frontend=$!

# Kill both children when this script exits for any reason.
trap 'kill "$backend" "$frontend" 2>/dev/null' EXIT INT TERM

echo "Backend:  http://127.0.0.1:7878"
echo "Frontend: http://localhost:5173"
echo "Press Ctrl+C to stop both."

wait
