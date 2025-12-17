# GitHub Actions Workflows

This document describes the GitHub Actions workflows configured for this project.

## Active Workflows

### 1. CI Workflow (`.github/workflows/ci.yml`)
**Triggers:** Push to main/master/develop branches, Pull requests to main/master/develop branches

**Purpose:** Ensures code quality and correctness on every push and pull request.

**Steps:**
- Installs system dependencies (libssl-dev, pkg-config)
- Sets up Rust toolchain
- Installs sqlx-cli
- Builds the project using `./cargo_db.sh build`
- Runs all tests using `./cargo_db.sh test`
- Checks code formatting with `./cargo_db.sh fmt --all -- --check`
- Runs clippy linter with `./cargo_db.sh clippy --all-targets -- -D warnings`

### 2. Release Workflow (`.github/workflows/release.yml`)
**Triggers:** Push of version tags (e.g., `v1.0.0`)

**Purpose:** Automates the release process, creating a Debian package and GitHub release.

**Steps:**
- Installs all required dependencies for building
- Sets up Rust toolchain
- Builds the project in release mode
- Runs tests in release mode
- Executes `./build_debian.sh` to create the `.deb` package
- Creates a GitHub release with the generated `.deb` file attached
- Auto-generates release notes

**Usage:** To trigger a release, create and push a tag:
```bash
git tag v0.1.2
git push origin v0.1.2
```

### 3. Security Audit Workflow (`.github/workflows/security-audit.yml`)
**Triggers:** 
- Daily at 00:00 UTC (scheduled)
- Changes to Cargo.toml or Cargo.lock
- Manual trigger via workflow_dispatch

**Purpose:** Scans dependencies for known security vulnerabilities.

**Steps:**
- Sets up Rust toolchain
- Installs cargo-audit
- Runs security audit on dependencies

### 4. Code Coverage Workflow (`.github/workflows/coverage.yml`)
**Triggers:** Push to main/master/develop branches, Pull requests to main/master/develop branches

**Purpose:** Generates code coverage reports and uploads them to Codecov.

**Steps:**
- Installs dependencies and Rust toolchain with llvm-tools
- Installs cargo-llvm-cov
- Generates coverage report in LCOV format
- Uploads to Codecov (requires CODECOV_TOKEN secret to be configured)

**Note:** To enable Codecov integration, add a `CODECOV_TOKEN` secret in repository settings.

## Dependabot Configuration (`.github/dependabot.yml`)

Dependabot is configured to automatically check for dependency updates:
- **Cargo dependencies:** Checked weekly, up to 10 PRs open at once
- **GitHub Actions:** Checked weekly, up to 5 PRs open at once

All Dependabot PRs are automatically labeled for easy filtering.

## Additional Workflow Suggestions

Here are some additional workflows that could be useful for this project:

### 1. Nightly Build Workflow
Test against Rust nightly to catch potential future compatibility issues early.

### 2. Multi-Platform Build Workflow
Test compilation and tests on multiple platforms (Linux, macOS, Windows) to ensure cross-platform compatibility.

### 3. Benchmarking Workflow
Run performance benchmarks on every push to track performance regressions over time.

### 4. Documentation Workflow
Automatically build and publish Rust documentation (cargo doc) to GitHub Pages.

### 5. Release Preview Workflow
Create draft releases on push to develop branch to preview what the next release would look like.

### 6. Changelog Automation
Automatically generate or update CHANGELOG.md based on commits or PR labels.

## Monitoring Workflow Status

All workflow runs can be monitored from the Actions tab in the GitHub repository:
https://github.com/vduda-zirnitra/pjmeter/actions

The CI workflow status badge is displayed in the README.md file.
