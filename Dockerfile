# Multi-stage build for optimal size and security
# Build stage
FROM python:3.11-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    gcc \
    g++ \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install uv for fast Python package management
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.cargo/bin:$PATH"

# Set working directory
WORKDIR /build

# Copy dependency files
COPY pyproject.toml uv.lock ./

# Install Python dependencies
RUN uv venv && \
    . .venv/bin/activate && \
    uv sync --frozen --no-dev

# Runtime stage
FROM python:3.11-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    git \
    nodejs \
    npm \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install AI CLI tools
RUN npm install -g @anthropic-ai/claude-code@latest || true
RUN npm install -g @google/gemini-cli@latest || true

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash hats && \
    mkdir -p /app /workspace /app/.agent /app/.cache && \
    chown -R hats:hats /app /workspace

# Copy Python environment from builder
COPY --from=builder --chown=hats:hats /build/.venv /app/.venv

# Set environment variables
ENV PATH="/app/.venv/bin:$PATH" \
    PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1

# Copy application code
WORKDIR /app
COPY --chown=hats:hats . /app/

# Make scripts executable
RUN chmod +x /app/hats_orchestrator.py /app/hats

# Create volume mount points
VOLUME ["/workspace", "/app/.agent", "/app/.cache"]

# Switch to non-root user
USER hats

# Set working directory for execution
WORKDIR /workspace

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import sys; import os; sys.exit(0 if os.path.exists('/app/hats_orchestrator.py') else 1)"

# Default entrypoint
ENTRYPOINT ["python", "/app/hats_orchestrator.py"]

# Default command (show help)
CMD ["--help"]