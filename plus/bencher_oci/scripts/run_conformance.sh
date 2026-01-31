#!/bin/bash
# OCI Distribution Spec Conformance Test Runner
#
# This script sets up and runs the official OCI conformance tests against
# the Bencher OCI registry.
#
# Prerequisites:
# - Go 1.17+ installed
# - Bencher API server running (cargo run -p bencher_api --features plus)
#
# Usage:
#   ./plus/bencher_oci/scripts/run_conformance.sh [options]
#
# Options:
#   --pull-only      Run only pull tests
#   --skip-build     Skip building the conformance binary
#   --api-url URL    Use a different API URL (default: http://localhost:61016)

set -e

# Default configuration
API_URL="${OCI_ROOT_URL:-http://localhost:61016}"
NAMESPACE="${OCI_NAMESPACE:-test/repo}"
CROSSMOUNT_NAMESPACE="${OCI_CROSSMOUNT_NAMESPACE:-test/other}"
CONFORMANCE_DIR="${CONFORMANCE_DIR:-./distribution-spec/conformance}"
SKIP_BUILD=false
PULL_ONLY=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --pull-only)
            PULL_ONLY=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --api-url)
            API_URL="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "=== OCI Conformance Test Runner ==="
echo "API URL: $API_URL"
echo "Namespace: $NAMESPACE"
echo "Crossmount Namespace: $CROSSMOUNT_NAMESPACE"
echo ""

# Check if API is running
echo "Checking API connectivity..."
if ! curl -s -f "$API_URL/v2/" > /dev/null 2>&1; then
    echo "ERROR: Cannot connect to API at $API_URL"
    echo "Please start the Bencher API server first:"
    echo "  cargo run -p bencher_api --features plus"
    exit 1
fi
echo "API is running!"

# Clone/update conformance tests if needed
if [[ ! -d "distribution-spec" ]]; then
    echo ""
    echo "Cloning OCI distribution-spec repository..."
    git clone --depth 1 https://github.com/opencontainers/distribution-spec.git
fi

# Build conformance tests if needed
if [[ ! -f "$CONFORMANCE_DIR/conformance.test" ]] && [[ "$SKIP_BUILD" != "true" ]]; then
    echo ""
    echo "Building conformance tests..."
    cd "$CONFORMANCE_DIR"
    go test -c
    cd - > /dev/null
fi

# Check conformance binary exists
if [[ ! -f "$CONFORMANCE_DIR/conformance.test" ]]; then
    echo "ERROR: Conformance test binary not found at $CONFORMANCE_DIR/conformance.test"
    echo "Try running without --skip-build option"
    exit 1
fi

# Set environment variables
export OCI_ROOT_URL="$API_URL"
export OCI_NAMESPACE="$NAMESPACE"
export OCI_CROSSMOUNT_NAMESPACE="$CROSSMOUNT_NAMESPACE"
export OCI_DEBUG="${OCI_DEBUG:-0}"

if [[ "$PULL_ONLY" == "true" ]]; then
    export OCI_TEST_PULL=1
    export OCI_TEST_PUSH=0
    export OCI_TEST_CONTENT_DISCOVERY=0
    export OCI_TEST_CONTENT_MANAGEMENT=0
else
    export OCI_TEST_PULL=1
    export OCI_TEST_PUSH=1
    export OCI_TEST_CONTENT_DISCOVERY=1
    export OCI_TEST_CONTENT_MANAGEMENT=1
fi

# Create output directory
OUTPUT_DIR="${OUTPUT_DIR:-./oci-conformance-results}"
mkdir -p "$OUTPUT_DIR"
export OCI_REPORT_DIR="$OUTPUT_DIR"

echo ""
echo "Running conformance tests..."
echo "Test categories enabled:"
echo "  - Pull: ${OCI_TEST_PULL}"
echo "  - Push: ${OCI_TEST_PUSH}"
echo "  - Content Discovery: ${OCI_TEST_CONTENT_DISCOVERY}"
echo "  - Content Management: ${OCI_TEST_CONTENT_MANAGEMENT}"
echo ""

# Run the tests
cd "$CONFORMANCE_DIR"
./conformance.test -test.v 2>&1 | tee "$OUTPUT_DIR/test-output.log"
TEST_RESULT=$?

cd - > /dev/null

# Copy results
if [[ -f "$CONFORMANCE_DIR/report.html" ]]; then
    cp "$CONFORMANCE_DIR/report.html" "$OUTPUT_DIR/"
fi
if [[ -f "$CONFORMANCE_DIR/junit.xml" ]]; then
    cp "$CONFORMANCE_DIR/junit.xml" "$OUTPUT_DIR/"
fi

echo ""
echo "=== Test Complete ==="
echo "Results saved to: $OUTPUT_DIR"
if [[ -f "$OUTPUT_DIR/report.html" ]]; then
    echo "Open $OUTPUT_DIR/report.html to view the detailed report"
fi

exit $TEST_RESULT
