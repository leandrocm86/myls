## Installation (pre-built binary)

Download the latest binary directly:

```bash
# Download
wget https://github.com/leandrocm86/myls/releases/latest/download/myls -O /usr/local/bin/myls

# Make executable
chmod +x /usr/local/bin/myls

# Run
myls --help
```


## Build

```bash
# Install musl target (only first time)
rustup target add x86_64-unknown-linux-musl
```
```bash
# Build the optimized static binary
cargo build --release --target x86_64-unknown-linux-musl
```
```bash
# Also build for other platforms (needs docker and permission to it)
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

## Generating release (authorized publishers only):

```bash
# Login to GitHub
gh auth login
```
```bash
# Create a release with the new compiled binary
gh release create v1.x.x ./target/x86_64-unknown-linux-musl/release/myls --title "v1.x.x" --notes "What changed in this version."
```
```bash
# Also add other platform's binaries (after renaming the binary accordingly)
gh release upload v1.x.x target/aarch64-unknown-linux-gnu/release/myls-aarch64
```
