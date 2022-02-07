#!/bin/bash
git pull
cargo build --release
kill -SIGTERM $(pgrep bot)
nohup ./target/release/bot &
disown