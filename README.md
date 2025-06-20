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

# Build the optimized static binary
cargo build --release --target x86_64-unknown-linux-musl
```

## Generating release (authorized publishers only):

```bash
# Login to GitHub
gh auth login

# Create a release with the new compiled binary
gh release create v1.0.0 ./target/x86_64-unknown-linux-musl/release/myls \
  --title "v1.0.0" \
  --notes "First version."
```
