# Design Preview System

This lets you rapidly test CLI design changes without rebuilding the entire pqcoin project.

## Quick Start

1. **Edit the design**: `design-preview/src/main.rs`
2. **Preview it**: `./preview.sh`

That's it! No need to publish or rebuild the full project.

## How It Works

- `design-preview/` is a standalone Rust project with just `colored` and `comfy-table`
- `preview.sh` builds and runs it (takes ~2 seconds after first compile)
- Test different banners, tables, command outputs, etc.

## Example Workflow

```bash
# Edit the banner design
nano design-preview/src/main.rs

# See it live
./preview.sh

# Try different colors, layouts, tables
# Iterate quickly without waiting for full builds
```

## What's Included

The preview currently shows:
- Main banner/launch screen
- Init command output
- Status command output

Add more demo functions to test other commands!

## Tips

- First run downloads dependencies (~10 seconds)
- Subsequent runs are instant (~1-2 seconds)
- Edit `main()` to show different screens
- Copy working designs back to main `src/` files when done
