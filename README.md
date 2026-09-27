# perceptui

A terminal UI for browsing the Enma and Perception documentation sites.

Pages are fetched once and cached in /tmp/enma-docs. Later launches check for updates in the background.

## Build

cargo build --release

## Run

./target/release/enma-tui
