#!/usr/bin/env bash
# Start the backend (Rust API) and frontend (web UI) together.
# Press Ctrl+C once to stop both.
set -euo pipefail
cd "$(dirname "$0")"

frontend_source="../nnj-grammar-fe"
frontend_dir="./web"

if [ -d "$frontend_dir" ]; then
    echo "Frontend folder/link exists"
else
    if [ ! -d "$frontend_source" ]; then
        git clone https://github.com/alejoseed/nnj-grammar-fe.git $frontend_source
    fi
    ln -s "$(realpath "$frontend_source")" "$frontend_dir"
fi

if [ ! -d "$frontend_dir/node_modules" ]; then
    echo "Installing frontend dependencies..."
    mise exec node@26 -- npm --prefix web ci
fi

trap 'kill ${backend:-} ${frontend:-} 2>/dev/null || true' EXIT INT TERM

cargo run --bin nnj-grammar-server &
backend=$!

mise exec node@26 -- npm --prefix web run dev &
frontend=$!

echo "Backend:  http://127.0.0.1:7878"
echo "Frontend: http://localhost:5173"
echo "Press Ctrl+C to stop both."

wait
