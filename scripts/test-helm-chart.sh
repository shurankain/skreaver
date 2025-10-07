#!/usr/bin/env bash
# Test Helm chart rendering without installing

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CHART_DIR="${PROJECT_ROOT}/helm/skreaver"

echo "Testing Helm chart: ${CHART_DIR}"
echo ""

# Check if helm is available (optional)
if command -v helm &> /dev/null; then
    echo "✓ Helm is installed"

    # Lint the chart
    echo ""
    echo "Running helm lint..."
    helm lint "${CHART_DIR}"

    # Dry-run template rendering
    echo ""
    echo "Testing template rendering (dry-run)..."
    helm template test-release "${CHART_DIR}" > /dev/null
    echo "✓ Templates render successfully"

    # Show sample output
    echo ""
    echo "Sample rendered deployment (first 30 lines):"
    helm template test-release "${CHART_DIR}" -s templates/deployment.yaml | head -30

else
    echo "⚠ Helm not installed - skipping validation"
    echo "  Install Helm: https://helm.sh/docs/intro/install/"
    echo ""
    echo "Manual validation checklist:"
    echo "  - Chart.yaml has valid apiVersion (v2)"
    echo "  - values.yaml is valid YAML"
    echo "  - All templates use correct Go template syntax"
    echo "  - Service names reference {{ .Release.Name }} correctly"
fi

echo ""
echo "Chart structure:"
find "${CHART_DIR}" -type f ! -path "*/charts/*" | sort
