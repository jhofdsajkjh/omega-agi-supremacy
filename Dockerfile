# ============================================================
# OMEGA AGI System - Production Dockerfile
# Multi-stage build: Rust compile -> Python runtime
# ============================================================

# ---------- Stage 1: Rust Build ----------
FROM python:3.11-slim AS rust-builder

# Install system dependencies for Rust compilation
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    libssl-dev \
    pkg-config \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Rust 1.95.0 via rustup
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH="/usr/local/cargo/bin:${PATH}"
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- \
    --default-toolchain 1.95.0 \
    --profile minimal \
    -y \
    && rustup default 1.95.0

WORKDIR /build

# Copy Rust crate manifests first (layer caching: rebuild only when deps change)
COPY omega-agi/hypercore/Cargo.toml omega-agi/hypercore/Cargo.lock /build/omega-agi/hypercore/
COPY omega-agi/runtime/Cargo.toml   omega-agi/runtime/Cargo.lock   /build/omega-agi/runtime/
COPY apex_agi_runtime_os/Cargo.toml  apex_agi_runtime_os/Cargo.lock /build/apex_agi_runtime_os/
COPY projects/apex-spiral/Cargo.toml  projects/apex-spiral/Cargo.lock /build/projects/apex-spiral/

# Create dummy source files so cargo can resolve and cache dependencies
RUN mkdir -p /build/omega-agi/hypercore/src && echo "" > /build/omega-agi/hypercore/src/lib.rs \
    && mkdir -p /build/omega-agi/runtime/src && echo "" > /build/omega-agi/runtime/src/lib.rs \
    && mkdir -p /build/apex_agi_runtime_os/src && echo "" > /build/apex_agi_runtime_os/src/lib.rs \
    && mkdir -p /build/projects/apex-spiral/src && echo "" > /build/projects/apex-spiral/src/lib.rs

# Pre-build dependencies (cached layer)
RUN for crate in omega-agi/hypercore omega-agi/runtime apex_agi_runtime_os projects/apex-spiral; do \
      (cd /build/$crate && cargo build --release 2>/dev/null || true); \
    done

# Now copy actual source code (invalidates cache only when source changes)
COPY omega-agi/hypercore/src /build/omega-agi/hypercore/src
COPY omega-agi/hypercore/tests /build/omega-agi/hypercore/tests
COPY omega-agi/runtime/src /build/omega-agi/runtime/src
COPY apex_agi_runtime_os/src /build/apex_agi_runtime_os/src
COPY projects/apex-spiral/src /build/projects/apex-spiral/src

# Build all Rust crates in release mode
RUN for crate in omega-agi/hypercore omega-agi/runtime apex_agi_runtime_os projects/apex-spiral; do \
      echo ">>> Building $crate" && \
      (cd /build/$crate && cargo build --release); \
    done

# ---------- Stage 2: Runtime ----------
FROM python:3.11-slim AS runtime

LABEL maintainer="OMEGA AGI System" \
      description="OMEGA AGI - Self-evolving autonomous intelligence system" \
      version="5.0"

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN groupadd --gid 1000 omega && \
    useradd --uid 1000 --gid omega --shell /bin/bash --create-home omega

WORKDIR /opt/omega-agi

# Install Python dependencies
RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir \
    pytest \
    requests \
    || pip install --no-cache-dir pytest requests

# Attempt faiss-cpu (optional - continue if unavailable)
RUN pip install --no-cache-dir faiss-cpu || echo "faiss-cpu unavailable, skipping"

# Copy Python application files
COPY apex_pr_engine.py           /opt/omega-agi/
COPY auto_pr_submitter.py        /opt/omega-agi/
COPY deep_audit_engine.py        /opt/omega-agi/
COPY quantum_vault.py            /opt/omega-agi/
COPY tiangong_agi_v5_unified.py  /opt/omega-agi/
COPY tiangong_security_daemon.py /opt/omega-agi/

# Copy configuration files
COPY .claude-code-config.json      /opt/omega-agi/
COPY tiangong_security_config.json /opt/omega-agi/

# Copy pipeline modules
COPY omega_pipeline/ /opt/omega-agi/omega_pipeline/

# Copy TDD scoring workspace
COPY apex_tdd_workspace/ /opt/omega-agi/apex_tdd_workspace/

# Copy compiled Rust artifacts from builder stage
COPY --from=rust-builder /build/omega-agi/hypercore/target/release/*.rlib /opt/omega-agi/rust-lib/hypercore/ 2>/dev/null || true
COPY --from=rust-builder /build/omega-agi/hypercore/target/release/lib*.so  /opt/omega-agi/rust-lib/hypercore/ 2>/dev/null || true
COPY --from=rust-builder /build/omega-agi/runtime/target/release/*.rlib     /opt/omega-agi/rust-lib/runtime/     2>/dev/null || true
COPY --from=rust-builder /build/omega-agi/runtime/target/release/lib*.so    /opt/omega-agi/rust-lib/runtime/     2>/dev/null || true
COPY --from=rust-builder /build/apex_agi_runtime_os/target/release/*.rlib   /opt/omega-agi/rust-lib/apex_runtime/ 2>/dev/null || true
COPY --from=rust-builder /build/apex_agi_runtime_os/target/release/lib*.so  /opt/omega-agi/rust-lib/apex_runtime/ 2>/dev/null || true
COPY --from=rust-builder /build/projects/apex-spiral/target/release/*.rlib  /opt/omega-agi/rust-lib/apex-spiral/   2>/dev/null || true
COPY --from=rust-builder /build/projects/apex-spiral/target/release/lib*.so /opt/omega-agi/rust-lib/apex-spiral/   2>/dev/null || true

# Also copy full target dirs for cargo test capability
COPY --from=rust-builder /build/omega-agi/hypercore/  /opt/omega-agi/rust-crates/omega-agi/hypercore/
COPY --from=rust-builder /build/omega-agi/runtime/    /opt/omega-agi/rust-crates/omega-agi/runtime/
COPY --from=rust-builder /build/apex_agi_runtime_os/  /opt/omega-agi/rust-crates/apex_agi_runtime_os/
COPY --from=rust-builder /build/projects/apex-spiral/ /opt/omega-agi/rust-crates/projects/apex-spiral/

# Create persistent directories
RUN mkdir -p /opt/omega-agi/evolution_runs /opt/omega-agi/memory && \
    chown -R omega:omega /opt/omega-agi

# Set environment variables
ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    RUST_BACKTRACE=1 \
    PATH="/opt/omega-agi:${PATH}"

# Switch to non-root user
USER omega

# Health check
HEALTHCHECK --interval=60s --timeout=10s --start-period=30s --retries=3 \
    CMD python3 -c "import sys; sys.exit(0)"

# Default command: run self-evolution loop
CMD ["python3", "omega_pipeline/self_evolution_loop.py", "--auto"]
