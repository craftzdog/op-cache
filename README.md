# op-cache

A fast caching proxy for 1Password CLI `op read` commands.

## Why?

`op read` is slow (~200-500ms) because it needs to communicate with the 1Password desktop app. If you're reading the same secret multiple times (e.g., in scripts), this adds up.

`op-cache` maintains a local daemon that caches secrets in memory, reducing repeated reads to ~1-2ms.

## Installation

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# Binary at target/release/op-cache
```

## Usage

```bash
# Read a secret (auto-starts daemon if needed)
op-cache read op://vault/item/field

# Check daemon status
op-cache status

# View cache statistics
op-cache stats

# Clear the cache
op-cache clear

# Stop the daemon
op-cache stop
```

### Running Commands with Secrets

`op-cache run` resolves `op://` references through the cache, then replaces the current process with your command (via `exec`). If any resolution fails, the command is aborted.

Each reference is first looked up in the cache. Any cache misses are resolved together in a single `op inject` call, so 1Password prompts for authorization at most once per run - even if the env has many uncached secrets.

```bash
# Run a command with secrets in env vars
export DATABASE_URL="op://Private/DB/url"
export API_KEY="op://Private/API/token"
op-cache run -- ./my-app

# Works with any command
op-cache run -- env | grep SECRET
op-cache run -- docker compose up
```

### Multiple 1Password Accounts

Use `--account` to target a specific account, on either `read` or `run`:

```bash
op-cache read --account my.1password.com op://Private/API/token
op-cache run --account my.1password.com -- ./my-app
```

If `--account` is omitted, the `OP_ACCOUNT` environment variable is used when set. Cache entries are partitioned per account, so the same reference on different accounts never returns the wrong secret.

### Using an Env File

Use `--env-file` to load environment variables from a file, just like `op run --env-file`:

```bash
op-cache run --env-file=.env -- ./my-app
```

The env file uses standard `.env` format:

```bash
# .env
DATABASE_URL="op://Private/DB/url"
API_KEY="op://Private/API/token"
DEBUG=true
```

- `KEY=VALUE`, `KEY="VALUE"`, and `KEY='VALUE'` are all supported
- Lines starting with `#` are comments
- Empty lines are ignored
- Env file entries override variables from the current process environment

### Example

```bash
$ op-cache read op://Private/API/token
sk-abc123...

$ op-cache stats
Cache Statistics:
  Entries: 1
  Hits:    0
  Misses:  1
  Hit Rate: 0.0%

$ op-cache read op://Private/API/token  # Cache hit - fast!
sk-abc123...

$ op-cache stats
Cache Statistics:
  Entries: 1
  Hits:    1
  Misses:  1
  Hit Rate: 50.0%
```

## How It Works

```
┌─────────────┐                      ┌─────────────┐
│  op-cache   │◄── Unix Socket ────►│   Daemon    │
│  (client)   │   /tmp/op-cache.sock │  (cache)    │
└──────┬──────┘                      └─────────────┘
       │
       │ cache miss
       ▼
┌─────────────┐
│   op read   │
└─────────────┘
```

1. Client checks daemon cache for the secret
2. On cache hit: return immediately (~1-2ms)
3. On cache miss: client runs `op read`, stores result in cache, returns value
4. Daemon is auto-started on first use

The client (not the daemon) executes `op read`. This ensures proper access to your desktop session and 1Password app integration.

`op-cache run` follows the same cache-first flow, but batches cache misses: instead of one `op read` per missing reference, it resolves all of them in a single `op inject` call, then stores each result in the cache individually. This avoids triggering a separate 1Password authorization prompt per secret.

## Configuration

Config file: `~/.config/op-cache/config.yaml`

```yaml
socket_path: /tmp/op-cache.sock
ttl_seconds: 86400      # Cache TTL (default: 24 hours)
max_entries: 1000       # Max cached secrets
op_path: op             # Path to op CLI
op_timeout_seconds: 30  # Timeout for op commands
```

All settings are optional - sensible defaults are used.

## Performance

| Operation | Latency |
|-----------|---------|
| Cache hit | ~1-2ms |
| Cache miss | ~200-500ms (op read time) |
| Cold start | ~50ms (daemon spawn) |

## Security Notes

- Secrets are cached in memory only (never written to disk)
- Cache is per-user (Unix socket permissions)
- TTL ensures secrets expire (default 24 hours)
- `op-cache clear` immediately purges all cached secrets
- Daemon stops cleanly on `op-cache stop` or system shutdown

## License

MIT
