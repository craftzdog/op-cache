# Changelog

## 0.2.0

### Added
- `--account` flag on `read` and `run` subcommands to target a specific 1Password account.
- Ambient `OP_ACCOUNT` environment variable is now included in cache key computation, preventing cross-account cache collisions.

### Fixed
- Cache entries are now partitioned by effective account (explicit `--account` or `OP_ACCOUNT` env var), fixing a bug where the same reference on different accounts could return a stale secret from the wrong account.

## 0.1.0

- Initial release: daemon-based caching proxy for `op read`, with `read`, `run`, `status`, `stats`, `clear`, and `stop` subcommands.
