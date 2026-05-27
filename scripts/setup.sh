#!/bin/bash
set -e

# =============================================================================
# OMEGA AGI Supremacy - Setup Script
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Detect OS
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$ID
    else
        OS="unknown"
    fi
}

# Install Docker
install_docker() {
    log_info "Installing Docker..."
    
    if command -v docker &> /dev/null; then
        log_info "Docker is already installed."
        docker --version
        return 0
    fi
    
    case "$OS" in
        ubuntu|debian)
            apt-get update
            apt-get install -y ca-certificates curl gnupg lsb-release
            mkdir -p /etc/apt/keyrings
            curl -fsSL https://download.docker.com/linux/${OS}/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
            echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/${OS} $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
            apt-get update
            apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
            ;;
        centos|rhel|fedora)
            yum install -y yum-utils
            yum-config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo
            yum install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
            ;;
        *)
            log_error "Unsupported OS: $OS"
            log_info "Please install Docker manually: https://docs.docker.com/get-docker/"
            return 1
            ;;
    esac
    
    # Start Docker
    systemctl start docker || service docker start
    systemctl enable docker || true
    
    log_info "Docker installed successfully."
    docker --version
}

# Install Docker Compose (standalone)
install_docker_compose() {
    log_info "Installing Docker Compose..."
    
    # Check if docker compose plugin is available
    if docker compose version &> /dev/null; then
        log_info "Docker Compose plugin is available."
        return 0
    fi
    
    # Install standalone docker-compose
    local VERSION="v2.24.0"
    local DESTINATION="/usr/local/bin/docker-compose"
    
    if command -v docker-compose &> /dev/null; then
        log_info "Docker Compose is already installed."
        docker-compose --version
        return 0
    fi
    
    curl -SL "https://github.com/docker/compose/releases/download/${VERSION}/docker-compose-linux-x86_64" -o "$DESTINATION"
    chmod +x "$DESTINATION"
    
    log_info "Docker Compose installed successfully."
    "$DESTINATION" --version
}

# Make scripts executable
make_scripts_executable() {
    log_info "Making scripts executable..."
    chmod +x deploy.sh scripts/*.sh
    log_info "Scripts are now executable."
}

# Create sample configs
create_sample_configs() {
    log_info "Creating sample configuration files..."
    
    # Prometheus config
    cat > config/prometheus.yml << 'EOF'
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'omega-core'
    static_configs:
      - targets: ['omega-core:9090']
EOF

    # Grafana datasource
    cat > config/grafana/provisioning/datasources/datasource.yml << 'EOF'
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://omega-monitor:9090
    isDefault: true
EOF

    log_info "Sample configurations created."
}

# Main setup
main() {
    log_info "========================================"
    log_info "  OMEGA AGI Supremacy Setup"
    log_info "========================================"
    
    detect_os
    log_info "Detected OS: $OS"
    
    install_docker
    install_docker_compose
    make_scripts_executable
    create_sample_configs
    
    log_info "========================================"
    log_info "  Setup Complete!"
    log_info "========================================"
    log_info ""
    log_info "Next steps:"
    log_info "  1. cp .env.example .env"
    log_info "  2. Edit .env and set your GITHUB_TOKEN"
    log_info "  3. ./deploy.sh"
}

main "$@"