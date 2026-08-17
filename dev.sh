#!/usr/bin/env bash
# Start the backend (Rust API) and frontend (web UI) together.
# Press Ctrl+C once to stop both.
set -euo pipefail
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
