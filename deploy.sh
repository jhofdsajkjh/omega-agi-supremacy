#!/bin/bash
set -e

# =============================================================================
# OMEGA AGI Supremacy - Deployment Script
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        log_error "Docker Compose is not installed. Please install Docker Compose first."
        exit 1
    fi
    
    log_info "Prerequisites check passed."
}

# Create necessary directories
create_directories() {
    log_info "Creating directory structure..."
    
    mkdir -p data/{core,swarm,evolution}
    mkdir -p config/grafana/provisioning/datasources
    mkdir -p config/grafana/provisioning/dashboards
    mkdir -p models
    
    log_info "Directory structure created."
}

# Copy environment file
setup_env() {
    if [ ! -f .env ]; then
        log_info "Creating .env file from template..."
        cp .env.example .env
        log_warn "Please edit .env and set your GITHUB_TOKEN before deploying."
    else
        log_info ".env file already exists, skipping."
    fi
}

# Build health monitoring system
build_health_monitor() {
    log_info "Building health monitoring system..."
    mkdir -p scripts

    # Rust health monitor
    if command -v rustc &> /dev/null && [ -f scripts/health_monitor.rs ]; then
        log_info "Compiling health_monitor.rs..."
        rustc scripts/health_monitor.rs -o scripts/health_monitor --edition 2021 -C opt-level=2 2>/dev/null || \
            log_warn "Health monitor compilation skipped (manual compile if needed)"
    fi

    # Python self-diagnosis (V2.0 with fault diagnosis)
    if [ -f scripts/self_diagnosis.py ]; then
        log_info "Self-diagnosis V2.0 ready: scripts/self_diagnosis.py"
    fi

    # Fault diagnosis expert system
    if [ -f scripts/fault_diagnosis.py ]; then
        chmod +x scripts/fault_diagnosis.py
        log_info "Fault diagnosis expert system: scripts/fault_diagnosis.py"
    fi

    # Auto repair executor
    if [ -f scripts/auto_repair.py ]; then
        chmod +x scripts/auto_repair.py
        log_info "Auto repair executor: scripts/auto_repair.py"
    fi

    # Fault patterns library
    if [ -f scripts/fault_patterns.yaml ]; then
        log_info "Fault patterns library: scripts/fault_patterns.yaml"
    fi

    # Health monitoring config
    if [ -f scripts/health_monitor.yaml ]; then
        log_info "Health monitor config: scripts/health_monitor.yaml"
    fi

    # Auto-recovery script
    if [ -f scripts/auto_recovery.sh ]; then
        chmod +x scripts/auto_recovery.sh
        log_info "Auto-recovery script ready: scripts/auto_recovery.sh"
    fi

    log_info "Health monitoring system built."
}

# Pull latest images or build
build_images() {
    log_info "Building Docker images..."
    
    if docker compose build --parallel; then
        log_info "Docker images built successfully."
    else
        log_error "Docker build failed."
        exit 1
    fi
}

# Start services
start_services() {
    log_info "Starting OMEGA AGI services..."
    
    docker compose up -d
    
    log_info "Services started. Checking health..."
    sleep 10
    
    # Run health check
    if ./scripts/health_check.sh; then
        log_info "All services are healthy!"
    else
        log_warn "Some services may not be fully healthy yet. Check logs with: docker compose logs"
    fi

    # Run enhanced self-diagnosis (V2.0)
    log_info "Running enhanced self-diagnosis..."
    if [ -f scripts/self_diagnosis.py ]; then
        python3 scripts/self_diagnosis.py --once --json 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    score = d.get('health_score', {}).get('score', 0)
    level = d.get('health_score', {}).get('level', 'UNKNOWN')
    faults = len(d.get('fault_classifications', []))
    print(f'  Health Score: {score:.1}/100 [{level}]')
    if faults > 0:
        print(f'  Detected Faults: {faults}')
        for fc in d.get('fault_classifications', [])[:3]:
            print(f'    - [{fc[\"severity\"]}] {fc[\"fault_type\"]}')
    else:
        print('  No faults detected.')
except: pass
" 2>/dev/null || log_warn "Self-diagnosis run skipped (python3 or dependencies not available)"
    fi

    log_info "Enhanced self-diagnosis complete."
}

# Main deployment
main() {
    log_info "========================================"
    log_info "  OMEGA AGI Supremacy Deployment"
    log_info "========================================"
    
    check_prerequisites
    create_directories
    setup_env
    build_health_monitor
    build_images
    start_services
    
    log_info "========================================"
    log_info "  Deployment Complete!"
    log_info "========================================"
    log_info "Services:"
    log_info "  - OMEGA Core:     http://localhost:8080"
    log_info "  - OMEGA Metrics: http://localhost:9090"
    log_info "  - OMEGA Monitor: http://localhost:9091 (with --profile monitoring)"
    log_info "  - Grafana:       http://localhost:3000 (with --profile monitoring)"
    log_info ""
    log_info "Useful commands:"
    log_info "  docker compose logs -f        # View logs"
    log_info "  docker compose ps             # Check status"
    log_info "  docker compose down           # Stop services"
    log_info "  ./scripts/health_check.sh     # Run health check"
}

main "$@"