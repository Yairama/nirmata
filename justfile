set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

frontend := "apps/nirmata-desktop/frontend"

# List available recipes.
default:
    @just --list

# Install the exact frontend dependencies from package-lock.json.
setup:
    npm ci --prefix {{frontend}}

# Start the Vite frontend development server.
dev:
    npm run dev --prefix {{frontend}}

# Type-check and compile the frontend assets.
frontend-build:
    npm run build --prefix {{frontend}}

# Build the complete desktop application in debug mode.
build: frontend-build
    cargo build -p nirmata-desktop

# Build the optimized executable and unsigned Windows installer.
release: frontend-build
    npm exec --prefix {{frontend}} -- tauri build --config apps/nirmata-desktop/src-tauri/tauri.conf.json --bundles nsis --ci --no-sign

# Start Vite and run the desktop application with Tauri hot reload.
run:
    npm exec --prefix {{frontend}} -- tauri dev --config apps/nirmata-desktop/src-tauri/tauri.conf.json

# Run fast static checks without executing the test suites.
check:
    npm run typecheck --prefix {{frontend}}
    cargo fmt --all -- --check
    cargo check --workspace

# Run frontend unit tests.
test-unit:
    npm test --prefix {{frontend}}

# Run browser behavior, accessibility, and screenshot tests.
test-e2e:
    npm run test:e2e --prefix {{frontend}}

# Run the secondary frontend source-safety checks.
test-safety:
    node --test {{frontend}}/safety-check.test.mjs

# Run all Rust workspace tests with nextest.
test-rust:
    cargo nextest run --workspace

# Run all frontend and Rust test suites.
test: test-unit test-e2e test-safety test-rust
