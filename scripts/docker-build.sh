#!/usr/bin/env bash
# Build Docker images for Skreaver

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Version from Cargo.toml
VERSION=$(grep -m1 '^version' "${PROJECT_ROOT}/Cargo.toml" | cut -d '"' -f2)

echo -e "${GREEN}Building Skreaver Docker Images${NC}"
echo "Version: ${VERSION}"
echo "Project Root: ${PROJECT_ROOT}"
echo ""

# Build the image
echo -e "${YELLOW}Building multi-stage Docker image...${NC}"
cd "${PROJECT_ROOT}"

docker build \
    --tag "skreaver:${VERSION}" \
    --tag "skreaver:latest" \
    --progress=plain \
    .

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ Build successful!${NC}"
    echo ""
    echo "Images created:"
    echo "  - skreaver:${VERSION}"
    echo "  - skreaver:latest"
    echo ""

    # Show image sizes
    echo "Image sizes:"
    docker images | grep "skreaver" | head -2
    echo ""

    # Test the image
    echo -e "${YELLOW}Testing image...${NC}"
    docker run --rm skreaver:latest --version

    echo ""
    echo -e "${GREEN}✓ All checks passed!${NC}"
else
    echo ""
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
