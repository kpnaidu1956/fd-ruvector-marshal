# RuVector Automation Scripts

This directory contains automation scripts to streamline development and prevent common issues.

## 📜 Available Scripts

### 🔄 sync-lockfile.sh
Automatically syncs `package-lock.json` with `package.json` changes.

**Usage:** `./scripts/sync-lockfile.sh`

### 🪝 install-hooks.sh
Installs git hooks for automatic lock file management.

**Usage:** `./scripts/install-hooks.sh`

### 🤖 ci-sync-lockfile.sh
CI/CD script for automatic lock file fixing.

**Usage:** `./scripts/ci-sync-lockfile.sh`

## 🚀 Quick Start

1. **Install git hooks** (recommended):
   ```bash
   ./scripts/install-hooks.sh
   ```

2. **Test the hook**:
   ```bash
   cd npm/packages/ruvector
   npm install chalk
   git add package.json
   git commit -m "test: Add chalk dependency"
   # Hook automatically updates lock file
   ```

See [CONTRIBUTING.md](../docs/CONTRIBUTING.md) for full documentation.
