#!/bin/bash
set -e

# =============================================================================
# OMEGA AGI Supremacy - Health Check Script
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

OVERALL_STATUS=0

check_service() {
    local SERVICE_NAME=$1
    local HEALTH_URL=$2
    local PORT=$3
    
    echo -n "Checking $SERVICE_NAME... "
    
    # Check if container is running
    if ! docker compose ps "$SERVICE_NAME" | grep -q "Up"; then
        echo -e "${RED}DOWN${NC} (container not running)"
        OVERALL_STATUS=1
        return 1
    fi
    
    # Check health endpoint if URL provided
    if [ -n "$HEALTH_URL" ]; then
        if curl -sf "$HEALTH_URL" > /dev/null 2>&1; then
            echo -e "${GREEN}HEALTHY${NC}"
        else
            echo -e "${YELLOW}RUNNING (health check not responding)${NC}"
        fi
    else
        echo -e "${GREEN}RUNNING${NC}"
    fi
}

echo "========================================"
echo "  OMEGA AGI Health Check"
echo "========================================"
echo ""

# Check core services
check_service "omega-core" "http://localhost:8080/health" "8080"
check_service "omega-swarm" "" ""
check_service "omega-evolution" "" ""

# Check optional monitoring (if running)
if docker compose ps omega-monitor 2>/dev/null | grep -q "Up"; then
    check_service "omega-monitor" "http://localhost:9091/-/healthy" "9091"
fi

if docker compose ps omega-grafana 2>/dev/null | grep -q "Up"; then
    check_service "omega-grafana" "" "3000"
fi

echo ""
echo "========================================"

if [ $OVERALL_STATUS -eq 0 ]; then
    echo -e "${GREEN}All core services are healthy!${NC}"
else
    echo -e "${YELLOW}Some services may need attention.${NC}"
    echo "Run 'docker compose logs' for details."
fi

exit $OVERALL_STATUS