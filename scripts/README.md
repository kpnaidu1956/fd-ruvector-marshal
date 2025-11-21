# Ruvector Scripts

Automation scripts for development, publishing, and maintenance.

## Publishing Scripts

### publish-crates.sh

Automated script to publish all Ruvector crates to crates.io in the correct dependency order.

**Prerequisites**:
- Rust and Cargo installed
- `CRATES_API_KEY` in `.env` file (never hardcoded!)
- All crates build successfully
- All tests pass

**Usage**:

```bash
# Make executable
chmod +x scripts/publish-crates.sh

# Run publishing
./scripts/publish-crates.sh
```

**What it does**:
1. Loads `CRATES_API_KEY` from `.env` (secure, not hardcoded)
2. Authenticates with crates.io
3. Publishes crates in dependency order:
   - Phase 1: `ruvector-core`, `router-core` (base crates)
   - Phase 2: `ruvector-node`, `ruvector-wasm`, `ruvector-cli`, `ruvector-bench`
   - Phase 3: `router-cli`, `router-ffi`, `router-wasm`
4. Waits between publishes for crates.io indexing
5. Reports success/failure summary

**Safety Features**:
- ✅ Reads credentials from `.env` (gitignored)
- ✅ Never hardcodes API keys
- ✅ Verifies packages before publishing
- ✅ Skips already published versions
- ✅ Provides detailed error messages
- ✅ Waits for crates.io indexing

## Security

⚠️ **Important**: This directory may contain scripts that use sensitive credentials.

**Always**:
- Store credentials in `.env` (gitignored)
- Use `.env.example` for templates
- Never hardcode API keys in scripts
- Review scripts before execution

See [SECURITY.md](../docs/development/SECURITY.md) for security best practices.

## Development Scripts

*More scripts will be added here as the project grows*

Potential additions:
- `test-all.sh` - Run all tests across crates
- `bench-all.sh` - Run all benchmarks
- `check-format.sh` - Verify code formatting
- `update-docs.sh` - Update documentation

## Contributing

When adding new scripts:

1. **Document thoroughly** - Add comments and usage examples
2. **Use .env for secrets** - Never hardcode credentials
3. **Make executable** - `chmod +x scripts/your-script.sh`
4. **Add to this README** - Document purpose and usage
5. **Test thoroughly** - Verify on clean checkout
6. **Error handling** - Exit on errors (`set -e`)
7. **Colored output** - Use colors for clarity

## Resources

- [Publishing Documentation](../docs/development/PUBLISHING.md)
- [Security Guidelines](../docs/development/SECURITY.md)
- [Contributing Guide](../docs/development/CONTRIBUTING.md)
