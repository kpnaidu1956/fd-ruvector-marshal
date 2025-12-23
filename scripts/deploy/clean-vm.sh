#!/bin/bash
# Complete cleanup script for ruvector-rag on VM
# This removes ALL data, binaries, and configurations
# Usage: ./clean-vm.sh [--keep-code]

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

KEEP_CODE=false
if [[ "$1" == "--keep-code" ]]; then
    KEEP_CODE=true
fi

echo ""
echo "=========================================="
echo -e "${RED}COMPLETE VM CLEANUP${NC}"
echo "=========================================="
echo ""
echo "This will DELETE:"
echo "  - All ruvector services"
echo "  - All ruvector binaries"
echo "  - All vector databases and data"
echo "  - All model caches"
echo "  - All logs"
if [[ "$KEEP_CODE" == false ]]; then
    echo "  - Source code (use --keep-code to preserve)"
fi
echo ""
read -p "Are you sure? Type 'yes' to confirm: " confirm
if [[ "$confirm" != "yes" ]]; then
    echo "Aborted."
    exit 1
fi

echo ""
log_info "Starting cleanup..."

# 1. Stop all services
log_info "Stopping services..."
sudo systemctl stop ruvector-rag 2>/dev/null || true
sudo systemctl stop caddy 2>/dev/null || true
sudo systemctl disable ruvector-rag 2>/dev/null || true

# 2. Kill any running processes
log_info "Killing running processes..."
sudo pkill -9 -f ruvector 2>/dev/null || true
sudo pkill -9 -f "ruvector-rag" 2>/dev/null || true
sleep 2

# 3. Remove systemd services
log_info "Removing systemd services..."
sudo rm -f /etc/systemd/system/ruvector-rag.service
sudo rm -rf /etc/systemd/system/ruvector-rag.service.d
sudo systemctl daemon-reload

# 4. Remove binaries
log_info "Removing binaries..."
sudo rm -rf /opt/ruvector-rag
sudo rm -f /usr/local/bin/ruvector*

# 5. Remove data directories (all users)
log_info "Removing data directories..."

# Current user
rm -rf ~/.local/share/ruvector-rag
rm -rf ~/.cache/ruvector-rag
rm -rf ~/ruvector-rag

# Root
sudo rm -rf /root/.local/share/ruvector-rag
sudo rm -rf /root/.cache/ruvector-rag

# rag user
sudo rm -rf /home/rag/.local/share/ruvector-rag
sudo rm -rf /home/rag/.cache/ruvector-rag
sudo rm -rf /home/rag

# kpnaidu user (if different from current)
sudo rm -rf /home/kpnaidu/.local/share/ruvector-rag
sudo rm -rf /home/kpnaidu/.cache/ruvector-rag

# System locations
sudo rm -rf /var/lib/ruvector-rag
sudo rm -rf /var/log/ruvector-rag

# 6. Remove logs
log_info "Removing logs..."
sudo rm -rf /var/log/caddy
sudo rm -f /var/log/caddy.log
rm -rf ~/fd-ruvector-marshal/logs 2>/dev/null || true

# 7. Clean Caddy config (optional - keep Caddy installed)
log_info "Cleaning Caddy config..."
sudo rm -f /etc/caddy/Caddyfile
sudo rm -f /etc/caddy/.api_key

# 8. Remove rag user
log_info "Removing rag user..."
sudo userdel -r rag 2>/dev/null || true

# 9. Remove source code (unless --keep-code)
if [[ "$KEEP_CODE" == false ]]; then
    log_info "Removing source code..."
    rm -rf ~/fd-ruvector-marshal
else
    log_info "Keeping source code at ~/fd-ruvector-marshal"
    # Clean build artifacts
    rm -rf ~/fd-ruvector-marshal/target
fi

# 10. Clean Rust build cache (saves space)
log_info "Cleaning Rust build cache..."
rm -rf ~/.cargo/registry/cache 2>/dev/null || true

echo ""
echo "=========================================="
echo -e "${GREEN}CLEANUP COMPLETE${NC}"
echo "=========================================="
echo ""
echo "Removed:"
echo "  ✓ All ruvector services"
echo "  ✓ All binaries"
echo "  ✓ All data and vector databases"
echo "  ✓ All model caches"
echo "  ✓ All logs"
if [[ "$KEEP_CODE" == false ]]; then
    echo "  ✓ Source code"
else
    echo "  - Source code preserved"
fi
echo ""
echo "To start fresh:"
echo "  1. git clone <repo> ~/fd-ruvector-marshal"
echo "  2. cd ~/fd-ruvector-marshal"
echo "  3. cargo build --release -p ruvector-rag"
echo "  4. sudo ./scripts/deploy/deploy-vm.sh rags.goalign.ai"
echo ""
