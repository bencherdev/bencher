# GitHub Actions Workflow Refactoring Guide

This guide provides step-by-step instructions for breaking down the monolithic `bencher.yml` workflow into smaller, focused workflows with better caching, reduced duplication, and optimized triggering.

## Table of Contents

1. [Overview](#overview)
2. [Current State Analysis](#current-state-analysis)
3. [Target Architecture](#target-architecture)
4. [Step-by-Step Implementation](#step-by-step-implementation)
5. [Path Filters for Optimization](#path-filters-for-optimization)
6. [Caching Strategy](#caching-strategy)
7. [Reusable Workflows](#reusable-workflows)
8. [Testing Your Changes](#testing-your-changes)

---

## Overview

### Goals

1. **Break down the monolithic workflow** into smaller, focused workflow files
2. **Reduce duplication** using reusable workflows and composite actions
3. **Improve cache sharing** across workflows
4. **Maintain the linear flow**: `devel` → `cloud` → `main` and `tags` → release
5. **Keep all existing checks** for `devel`, `cloud`, and tags
6. **Gate PR deployments** behind a `deploy` label
7. **Optimize execution** by only running relevant jobs when specific paths change

### Branch/Tag Strategy

| Trigger        | Behavior                                                                |
| -------------- | ----------------------------------------------------------------------- |
| `devel` branch | Full CI + deploy to Fly.io Dev + Netlify Dev                            |
| `cloud` branch | Full CI + deploy to Fly.io Test/Prod + Netlify Prod, then rebase `main` |
| `main` branch  | Minimal checks (receives rebased code from `cloud`)                     |
| Tags (`v*`)    | Full CI + GitHub Release                                                |
| PRs            | Full CI, deployment to Fly.io Dev + Netlify Dev gated by `deploy` label |

---

## Current State Analysis

### Jobs in Current Workflow

| Category        | Jobs                                                                                                                                                                                     |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Linting**     | `cargo_fmt`, `cargo_clippy`, `npx_biome_format`, `npx_biome_lint`, `check_generated`, `cargo_deny`                                                                                       |
| **Testing**     | `cargo_test`, `cargo_test_pr`, `api_smoke_test`, `release_api_smoke_test`, `docker_api_smoke_test`, `cargo_bench`, `cargo_audit`, `cargo_udeps`, `test_bencher_valid_wasm`, `npx_vitest` |
| **FOSS Checks** | `cargo_check_workspace_not_plus`, `bencher_console_not_plus`                                                                                                                             |
| **Builds**      | `build_github_action`, `build_bencher_valid_wasm`, `build_api_docker`, `build_cli`, `package_cli`, `build_console_docker`, `build_console`                                               |
| **Deployments** | `deploy_api_fly_dev`, `deploy_api_fly_test`, `deploy_api_fly`, `deploy_console_netlify_dev`, `deploy_console_netlify`                                                                    |
| **Release**     | `release_bencher`                                                                                                                                                                        |
| **Other**       | `test_cli_install`, `build_dev_container`                                                                                                                                                |

### Current Issues

1. **Single large file** (1182 lines) is hard to maintain
2. **Duplicated conditions** like `github.ref == 'refs/heads/main' || github.ref == 'refs/heads/cloud'...`
3. **Cache keys are scattered** and not consistently shared
4. **All jobs run for all triggers** (no path filtering)
5. **PR deployments run unconditionally** on `devel`/`cloud` pushes

---

## Target Architecture

### Proposed Workflow Structure

```
.github/
├── workflows/
│   ├── ci.yml                    # Main CI pipeline (lint, test, build)
│   ├── deploy-devel.yml          # Deploy to dev environment
│   ├── deploy-cloud.yml          # Deploy to prod environment
│   ├── release.yml               # Release workflow for tags
│   └── dev-container.yml         # Dev container builds
│
├── actions/
│   ├── setup-rust/
│   │   └── action.yml            # Composite action for Rust setup
│   ├── setup-node/
│   │   └── action.yml            # Composite action for Node setup
│   └── cargo-cache/
│       └── action.yml            # Composite action for cargo caching
│
└── shared/
    └── env.yml                   # Shared environment variables (referenced)
```

### Workflow Dependencies

```
┌───────────────────────────────────────────────────────┐
│                           Triggers                    │
├─────────────┬─────────────┬─────────────┬─────────────┤
│   devel     │   cloud     │    main     │   tags      │
└──────┬──────┴──────┬──────┴──────┬──────┴──────┬──────┘
       │             │             │             │           
       ▼             ▼             ▼             ▼          
    ci.yml        ci.yml       (skip)        ci.yml      
       │             │                          │          
       ▼             ▼                          │          
 deploy-devel   deploy-cloud                    │      
(PR if labeled)
       │             │                          │     
       ▼             ▼                          ▼           
  Netlify Dev   Netlify Prod              release.yml     
(PR if labeled)
                     │
                     ▼
              Rebase main
```

---

## Step-by-Step Implementation

### Step 1: Create Shared Environment Variables

Create a file to document shared environment variables. While GitHub Actions doesn't support importing env vars from files directly, you can use `workflow_call` inputs or define them consistently.

**Create `.github/shared/versions.md`** (documentation for reference):

```markdown
# Shared Versions

These versions should be kept in sync across all workflows:

| Variable            | Value  | Description            |
| ------------------- | ------ | ---------------------- |
| BENCHER_VERSION     | 0.5.10 | Bencher CLI version    |
| MOLD_VERSION        | 2.34.1 | Mold linker version    |
| WASM_PACK_VERSION   | 0.12.1 | wasm-pack version      |
| TYPESHARE_VERSION   | 1.13.2 | Typeshare CLI version  |
| ZIG_VERSION         | 0.13.0 | Zig compiler version   |
| ZIG_BUILD_VERSION   | 0.19.3 | cargo-zigbuild version |
| GLIBC_VERSION       | 2.17   | Minimum glibc version  |
| NETLIFY_CLI_VERSION | 18.0.4 | Netlify CLI version    |
| LITESTREAM_VERSION  | 0.3.13 | Litestream version     |
```

### Step 2: Create Composite Actions for Common Setup

#### 2a. Create Rust Setup Composite Action

**Create `.github/actions/setup-rust/action.yml`:**

```yaml
name: 'Setup Rust Environment'
description: 'Set up Rust with mold linker and caching'

inputs:
  mold-version:
    description: 'Mold linker version'
    required: false
    default: '2.34.1'
  cache-key:
    description: 'Cache key suffix'
    required: true
  working-directory:
    description: 'Working directory for cargo commands'
    required: false
    default: '.'

runs:
  using: 'composite'
  steps:
    - name: Setup Rust cache
      uses: actions/cache@v4
      with:
        path: |
          ~/.cargo/bin/
          ~/.cargo/registry/index/
          ~/.cargo/registry/cache/
          ~/.cargo/git/db/
          target/
        key: ${{ runner.os }}-cargo-${{ inputs.cache-key }}-${{ hashFiles('**/Cargo.lock') }}
        restore-keys: |
          ${{ runner.os }}-cargo-${{ inputs.cache-key }}-
          ${{ runner.os }}-cargo-
    
    - name: Setup mold linker
      uses: rui314/setup-mold@v1
      with:
        mold-version: ${{ inputs.mold-version }}
```

#### 2b. Create Node Setup Composite Action

**Create `.github/actions/setup-node/action.yml`:**

```yaml
name: 'Setup Node Environment'
description: 'Set up Node.js with npm caching'

inputs:
  working-directory:
    description: 'Working directory for npm install'
    required: true

runs:
  using: 'composite'
  steps:
    - name: Setup Node.js
      uses: actions/setup-node@v4
      with:
        node-version: '20'
        cache: 'npm'
        cache-dependency-path: ${{ inputs.working-directory }}/package-lock.json
    
    - name: Install dependencies
      shell: bash
      working-directory: ${{ inputs.working-directory }}
      run: npm install --include=dev
```

### Step 3: Create Reusable Workflows

#### 3a. Create Lint Workflow

**Create `.github/workflows/lint.yml`:**

```yaml
name: Lint

on:
  workflow_call:
    inputs:
      mold-version:
        type: string
        default: '2.34.1'
      typeshare-version:
        type: string
        default: '1.13.2'

env:
  CARGO_TERM_COLOR: always

jobs:
  cargo_fmt:
    name: Cargo Format
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - name: Add fmt
        run: rustup component add rustfmt
      - name: Run fmt
        run: cargo fmt -- --check

  cargo_clippy:
    name: Cargo Clippy
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        with:
          cache-key: all-features
          mold-version: ${{ inputs.mold-version }}
      - name: Add clippy
        run: rustup component add clippy
      - name: Run clippy
        run: ./scripts/clippy.sh

  check_generated:
    name: Check Generated
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        with:
          cache-key: generated
          mold-version: ${{ inputs.mold-version }}
      - name: Generate OpenAPI Spec
        run: cargo gen-spec
      - name: Check OpenAPI Spec for changes
        run: git diff --exit-code
      - name: Install Typeshare CLI
        run: cargo install typeshare-cli --version ${{ inputs.typeshare-version }} --locked
      - name: Generate Typescript Types
        run: cargo gen-ts
      - name: Check Typescript Types for changes
        run: git diff --exit-code
      - name: Generate Unix CLI install scripts
        run: cargo gen-installer sh
      - name: Check Unix CLI install script for changes
        run: git diff --exit-code
      - name: Generate Windows CLI install scripts
        run: cargo gen-installer ps1
      - name: Check Windows CLI install script for changes
        run: git diff --exit-code

  cargo_deny:
    name: Cargo Deny
    runs-on: ubuntu-22.04
    continue-on-error: true
    steps:
      - uses: actions/checkout@v6
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          log-level: warn
          command: check
          arguments: --locked

  biome_format:
    name: Biome Format
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - name: Biome format Action
        working-directory: ./services/action
        run: |
          npm install --include=dev
          npx biome ci --linter-enabled false --organize-imports-enabled false .
      - name: Biome format Console UI
        working-directory: ./services/console
        run: |
          npm install --include=dev
          npx biome ci --linter-enabled false --organize-imports-enabled false .

  biome_lint:
    name: Biome Lint
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - name: Biome check Action
        working-directory: ./services/action
        run: |
          npm install --include=dev
          npx biome ci --formatter-enabled false --organize-imports-enabled false .
      - name: Biome check Console UI
        working-directory: ./services/console
        run: |
          npm install --include=dev
          npx biome ci --formatter-enabled false --organize-imports-enabled false .
```

#### 3b. Create Test Workflow

**Create `.github/workflows/test.yml`:**

```yaml
name: Test

on:
  workflow_call:
    inputs:
      mold-version:
        type: string
        default: '2.34.1'
      is-fork:
        type: boolean
        default: false
    secrets:
      TEST_BILLING_KEY:
        required: false
      BENCHER_API_TOKEN:
        required: false
      GITHUB_TOKEN:
        required: true

env:
  CARGO_TERM_COLOR: always

jobs:
  cargo_test:
    name: Cargo Test
    runs-on: ubuntu-22.04
    env:
      TEST_BILLING_KEY: ${{ secrets.TEST_BILLING_KEY }}
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        if: ${{ !inputs.is-fork }}
        with:
          cache-key: all-features
          mold-version: ${{ inputs.mold-version }}
      - uses: rui314/setup-mold@v1
        if: ${{ inputs.is-fork }}
        with:
          mold-version: ${{ inputs.mold-version }}
      - name: cargo test
        run: RUST_BACKTRACE=1 cargo test --all-features -- --nocapture
      - name: Upload Perf JPEG
        uses: actions/upload-artifact@v4
        with:
          name: perf.jpeg
          path: ./lib/bencher_plot/perf.jpeg
          if-no-files-found: error

  api_smoke_test:
    name: API Smoke Test
    runs-on: ubuntu-22.04
    steps:
      - uses: jlumbroso/free-disk-space@main
        with:
          large-packages: false
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        if: ${{ !inputs.is-fork }}
        with:
          cache-key: api
          mold-version: ${{ inputs.mold-version }}
      - uses: rui314/setup-mold@v1
        if: ${{ inputs.is-fork }}
        with:
          mold-version: ${{ inputs.mold-version }}
      - name: Run Smoke Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-api smoke ci

  cargo_bench:
    name: Cargo Bench
    runs-on: ubuntu-22.04
    if: ${{ !inputs.is-fork }}
    continue-on-error: true
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        with:
          cache-key: cli
          mold-version: ${{ inputs.mold-version }}
      - name: Install `bencher` CLI
        run: cargo install --debug --path services/cli --locked --force
      - name: Dogfooding Benchmarks with Bencher
        run: |
          bencher run \
          --host https://api.bencher.dev \
          --project bencher \
          --token '${{ secrets.BENCHER_API_TOKEN }}' \
          --branch "$GITHUB_REF_NAME" \
          --start-point "$GITHUB_BASE_REF" \
          --testbed ubuntu-22.04 \
          --adapter rust_criterion \
          --err \
          --github-actions ${{ secrets.GITHUB_TOKEN }} \
          cargo bench --package bencher_adapter

  cargo_audit:
    name: Cargo Audit
    runs-on: ubuntu-22.04
    if: ${{ !inputs.is-fork }}
    continue-on-error: true
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        with:
          cache-key: audit
          mold-version: ${{ inputs.mold-version }}
      - run: cargo audit

  cargo_udeps:
    name: Cargo Unused Deps
    runs-on: ubuntu-22.04
    continue-on-error: true
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        if: ${{ !inputs.is-fork }}
        with:
          cache-key: nightly
          mold-version: ${{ inputs.mold-version }}
      - uses: rui314/setup-mold@v1
        if: ${{ inputs.is-fork }}
        with:
          mold-version: ${{ inputs.mold-version }}
      - name: Install nightly toolchain
        run: rustup toolchain install nightly
      - name: Install udeps
        run: cargo install --version 0.1.50 --locked --force cargo-udeps
      - name: Run API udeps
        continue-on-error: true
        working-directory: ./services/api
        run: cargo +nightly udeps --all-targets
      - name: Run CLI udeps
        continue-on-error: true
        working-directory: ./services/cli
        run: cargo +nightly udeps --all-targets

  foss_cargo_check:
    name: Cargo Check Workspace (not(plus))
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ inputs.mold-version }}
      - name: cargo check
        run: cargo check --no-default-features

  foss_console:
    name: Bencher Console (not(plus))
    runs-on: ubuntu-22.04
    env:
      WASM_PACK_VERSION: '0.12.1'
    steps:
      - uses: actions/checkout@v6
      - name: Install `wasm-pack`
        run: cargo install wasm-pack --version ${{ env.WASM_PACK_VERSION }} --locked --force
      - name: Build WASM
        working-directory: ./services/console
        run: npm run wasm:not-plus
      - name: Build Console
        working-directory: ./services/console
        run: npm run node
```

#### 3c. Create Build Workflow

**Create `.github/workflows/build.yml`:**

```yaml
name: Build

on:
  workflow_call:
    inputs:
      mold-version:
        type: string
        default: '2.34.1'
      wasm-pack-version:
        type: string
        default: '0.12.1'
      zig-version:
        type: string
        default: '0.13.0'
      zig-build-version:
        type: string
        default: '0.19.3'
      glibc-version:
        type: string
        default: '2.17'
      build-docker:
        type: boolean
        default: true
      build-cli:
        type: boolean
        default: true
    secrets:
      GITHUB_TOKEN:
        required: true
    outputs:
      wasm-artifact-name:
        description: 'WASM artifact name'
        value: 'bencher-valid-pkg'

env:
  CARGO_TERM_COLOR: always
  GITHUB_REGISTRY: ghcr.io
  API_DOCKER_IMAGE: bencher-api
  CONSOLE_DOCKER_IMAGE: bencher-console
  CLI_BIN_NAME: bencher
  WASM_BENCHER_VALID: bencher-valid-pkg

jobs:
  build_github_action:
    name: Build GitHub Action
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v6
      - name: Install Dependencies
        working-directory: ./services/action
        run: npm install --include=dev
      - name: Build
        working-directory: ./services/action
        run: npm run build
      - name: Check for changes
        run: git diff --exit-code

  build_wasm:
    name: Build `bencher_valid` WASM
    runs-on: ubuntu-22.04
    env:
      WASM_PACK_BUILD: "wasm-pack build --target web --no-default-features --features wasm,plus"
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        with:
          cache-key: wasm
          mold-version: ${{ inputs.mold-version }}
      - name: Install `wasm-pack`
        run: cargo install wasm-pack --version ${{ inputs.wasm-pack-version }} --locked --force
      - name: WASM pack `bencher_valid`
        working-directory: ./lib/bencher_valid
        run: |
          $WASM_PACK_BUILD || \
          $WASM_PACK_BUILD || \
          $WASM_PACK_BUILD || \
          $WASM_PACK_BUILD || \
          $WASM_PACK_BUILD
      - name: Upload Artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.WASM_BENCHER_VALID }}
          path: ./lib/bencher_valid/pkg
          if-no-files-found: error

  test_wasm:
    name: Test `bencher_valid` WASM
    runs-on: ubuntu-22.04
    needs: build_wasm
    steps:
      - uses: actions/checkout@v6
      - uses: ./.github/actions/setup-rust
        with:
          cache-key: wasm
          mold-version: ${{ inputs.mold-version }}
      - name: cargo test `bencher_valid` WASM
        working-directory: ./lib/bencher_valid
        run: cargo test --no-default-features --features plus,wasm

  vitest:
    name: vitest
    runs-on: ubuntu-22.04
    needs: build_wasm
    steps:
      - uses: actions/checkout@v6
      - name: Download `bencher_valid` Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.WASM_BENCHER_VALID }}
          path: ./lib/bencher_valid/pkg
      - name: npx vitest
        working-directory: ./services/console
        run: |
          npm install --include=dev
          npx vitest run

  build_api_docker:
    name: Build API Docker
    runs-on: ubuntu-22.04
    if: ${{ inputs.build-docker }}
    env:
      LITESTREAM_VERSION: '0.3.13'
      LITESTREAM_ARCH: amd64
      CACHE_NAME: ${{ github.ref_name }}
    steps:
      - uses: actions/checkout@v6
      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3
      - name: Setup Docker buildx
        uses: docker/setup-buildx-action@v3
      - name: Log in to the Container registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.GITHUB_REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Build API Docker Image
        uses: docker/build-push-action@v6
        with:
          context: .
          file: ./services/api/Dockerfile
          tags: ${{ env.API_DOCKER_IMAGE }}
          build-args: |
            MOLD_VERSION=${{ inputs.mold-version }}
            LITESTREAM_VERSION=${{ env.LITESTREAM_VERSION }}
            LITESTREAM_ARCH=${{ env.LITESTREAM_ARCH }}
          cache-from: type=registry,ref=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.API_DOCKER_IMAGE }}:cache-${{ env.CACHE_NAME }}
          cache-to: type=registry,ref=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.API_DOCKER_IMAGE }}:cache-${{ env.CACHE_NAME }},mode=max
          load: true
          push: false
      - name: Save API Docker Image
        run: |
          docker save ${{ env.API_DOCKER_IMAGE }} \
          | gzip > ${{ env.API_DOCKER_IMAGE }}.tar.gz
      - name: Upload API Docker Image Artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.API_DOCKER_IMAGE }}.tar.gz
          path: ./${{ env.API_DOCKER_IMAGE }}.tar.gz
          if-no-files-found: error

  build_console_docker:
    name: Build Console Docker
    runs-on: ubuntu-22.04
    if: ${{ inputs.build-docker }}
    env:
      CACHE_NAME: ${{ github.ref_name }}
    steps:
      - uses: actions/checkout@v6
      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3
      - name: Setup Docker buildx
        uses: docker/setup-buildx-action@v3
      - name: Log in to the Container registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.GITHUB_REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Build Console Docker Image
        uses: docker/build-push-action@v6
        with:
          context: .
          file: ./services/console/Dockerfile
          tags: ${{ env.CONSOLE_DOCKER_IMAGE }}
          cache-from: type=registry,ref=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.CONSOLE_DOCKER_IMAGE }}:cache-${{ env.CACHE_NAME }}
          cache-to: type=registry,ref=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.CONSOLE_DOCKER_IMAGE }}:cache-${{ env.CACHE_NAME }},mode=max
          load: true
          push: false
      - name: Save Console Docker Image
        run: |
          docker save ${{ env.CONSOLE_DOCKER_IMAGE }} \
          | gzip > ${{ env.CONSOLE_DOCKER_IMAGE }}.tar.gz
      - name: Upload Console Docker Image Artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.CONSOLE_DOCKER_IMAGE }}.tar.gz
          path: ./${{ env.CONSOLE_DOCKER_IMAGE }}.tar.gz
          if-no-files-found: error

  build_cli:
    name: Build CLI
    if: ${{ inputs.build-cli }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - build: linux-x86-64
            os: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
          - build: linux-arm-64
            os: ubuntu-22.04
            target: aarch64-unknown-linux-gnu
          - build: macos-x86-64
            os: macos-14
            target: x86_64-apple-darwin
          - build: macos-arm-64
            os: macos-14
            target: aarch64-apple-darwin
          - build: windows-x86-64
            os: windows-2022
            target: x86_64-pc-windows-msvc
          - build: windows-arm-64
            os: windows-2022
            target: aarch64-pc-windows-msvc
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v6
      - name: Install Rust target
        run: rustup target add ${{ matrix.target }}
      - uses: mlugg/setup-zig@v1
        if: startsWith(matrix.build, 'linux')
        with:
          version: ${{ inputs.zig-version }}
      - name: Install zigbuild
        if: startsWith(matrix.build, 'linux')
        run: cargo install --version ${{ inputs.zig-build-version }} --locked --force cargo-zigbuild
      - name: cargo zigbuild Linux CLI
        if: startsWith(matrix.build, 'linux')
        working-directory: ./services/cli
        run: cargo zigbuild --profile release-small --target ${{ matrix.target }}.${{ inputs.glibc-version }}
      - name: cargo build macOS CLI
        if: startsWith(matrix.build, 'macos')
        working-directory: ./services/cli
        run: cargo build --profile release-small --target ${{ matrix.target }}
      - name: cargo build Windows CLI
        if: startsWith(matrix.build, 'windows')
        working-directory: ./services/cli
        run: cargo build --profile release-small --target ${{ matrix.target }}
      - name: Rename Unix CLI bin
        if: (!startsWith(matrix.build, 'windows'))
        run: mv ./target/${{ matrix.target }}/release-small/${{ env.CLI_BIN_NAME }} ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}
      - name: Rename Windows CLI bin
        if: startsWith(matrix.build, 'windows')
        run: mv ./target/${{ matrix.target }}/release-small/${{ env.CLI_BIN_NAME }}.exe ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}.exe
      - name: Upload Unix CLI Artifact
        if: (!startsWith(matrix.build, 'windows'))
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}
          path: ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}
          if-no-files-found: error
      - name: Upload Windows CLI Artifact
        if: startsWith(matrix.build, 'windows')
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}.exe
          path: ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}.exe
          if-no-files-found: error

  package_cli:
    name: Package CLI
    if: ${{ inputs.build-cli }}
    needs: build_cli
    strategy:
      fail-fast: false
      matrix:
        include:
          - build: linux-x86-64
            os: ubuntu-22.04
            arch: amd64
          - build: linux-arm-64
            os: ubuntu-22.04
            arch: arm64
    runs-on: ${{ matrix.os }}
    env:
      CLI_DEB_DIR: deb
    steps:
      - uses: actions/checkout@v6
      - name: Download CLI Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ inputs.mold-version }}
      - name: Build .deb package
        run: cargo gen-pkg deb --dir ${{ env.CLI_DEB_DIR }} --arch ${{ matrix.arch }} ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}
      - name: Test install .deb package
        if: matrix.build == 'linux-x86-64'
        run: sudo dpkg -i ${{ env.CLI_DEB_DIR }}/${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}.deb
      - name: Sanity test .deb package installation
        if: matrix.build == 'linux-x86-64'
        run: |
          bencher --version
          bencher mock
          man bencher
      - name: Upload .deb Artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}.deb
          path: ${{ env.CLI_DEB_DIR }}/${{ env.CLI_BIN_NAME }}-${{ github.ref_name }}-${{ matrix.build }}.deb
          if-no-files-found: error
```

### Step 4: Create Main CI Workflow

**Create `.github/workflows/ci.yml`:**

```yaml
name: CI

on:
  workflow_dispatch:
  push:
    branches: [devel, cloud]
    tags: [v**]
  pull_request:
    branches: [main, cloud, devel]

# Cancel in-progress runs for the same branch
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

env:
  MOLD_VERSION: '2.34.1'

jobs:
  # Determine what changed
  changes:
    name: Detect Changes
    runs-on: ubuntu-22.04
    outputs:
      rust: ${{ steps.filter.outputs.rust }}
      console: ${{ steps.filter.outputs.console }}
      action: ${{ steps.filter.outputs.action }}
      api: ${{ steps.filter.outputs.api }}
      cli: ${{ steps.filter.outputs.cli }}
      docker: ${{ steps.filter.outputs.docker }}
    steps:
      - uses: actions/checkout@v6
      - uses: dorny/paths-filter@v3
        id: filter
        with:
          filters: |
            rust:
              - 'Cargo.toml'
              - 'Cargo.lock'
              - 'lib/**'
              - 'plus/**'
              - 'services/api/**'
              - 'services/cli/**'
              - 'tasks/**'
              - 'xtask/**'
            console:
              - 'services/console/**'
              - 'lib/bencher_valid/**'
            action:
              - 'services/action/**'
            api:
              - 'services/api/**'
              - 'lib/**'
              - 'plus/**'
            cli:
              - 'services/cli/**'
              - 'lib/**'
            docker:
              - 'services/api/Dockerfile'
              - 'services/console/Dockerfile'
              - 'docker/**'

  # Lint jobs - always run
  lint:
    name: Lint
    uses: ./.github/workflows/lint.yml
    with:
      mold-version: '2.34.1'
      typeshare-version: '1.13.2'

  # Test jobs - run based on changes
  test:
    name: Test
    needs: changes
    if: needs.changes.outputs.rust == 'true' || github.ref == 'refs/heads/devel' || github.ref == 'refs/heads/cloud' || startsWith(github.ref, 'refs/tags/')
    uses: ./.github/workflows/test.yml
    with:
      mold-version: '2.34.1'
      is-fork: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name != github.repository }}
    secrets:
      TEST_BILLING_KEY: ${{ secrets.TEST_BILLING_KEY }}
      BENCHER_API_TOKEN: ${{ secrets.BENCHER_API_TOKEN }}
      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  # Build jobs - run based on changes or for releases
  build:
    name: Build
    needs: changes
    uses: ./.github/workflows/build.yml
    with:
      mold-version: '2.34.1'
      build-docker: ${{ needs.changes.outputs.docker == 'true' || github.ref == 'refs/heads/devel' || github.ref == 'refs/heads/cloud' || startsWith(github.ref, 'refs/tags/') }}
      build-cli: ${{ needs.changes.outputs.cli == 'true' || startsWith(github.ref, 'refs/tags/') }}
    secrets:
      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  # Console build (for PRs and devel/cloud)
  build_console:
    name: Build Console UI
    runs-on: ubuntu-22.04
    needs: build
    if: ${{ !startsWith(github.ref, 'refs/tags/') }}
    env:
      WASM_BENCHER_VALID: bencher-valid-pkg
    steps:
      - uses: actions/checkout@v6
      - name: Download `bencher_valid` Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.WASM_BENCHER_VALID }}
          path: ./lib/bencher_valid/pkg
      - name: Build Console UI
        working-directory: ./services/console
        run: npm run netlify
      - name: Test Links
        if: github.ref == 'refs/heads/cloud' || github.ref == 'refs/heads/devel'
        timeout-minutes: 3
        continue-on-error: true
        uses: lycheeverse/lychee-action@v2.3.0
        with:
          args: --config ./services/console/lychee.toml --github-token ${{ secrets.GITHUB_TOKEN }} --user-agent "${{ secrets.LYCHEE_USER_AGENT }}" ./services/console/dist

  # Cross-platform smoke tests (only for releases)
  release_smoke_tests:
    name: Release API Smoke Test
    if: startsWith(github.ref, 'refs/tags/')
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-22.04
          - os: macos-14
          - os: windows-2022
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v6
      - uses: rui314/setup-mold@v1
        if: matrix.os == 'ubuntu-22.04'
        with:
          mold-version: ${{ env.MOLD_VERSION }}
      - name: Run Smoke Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-api smoke localhost

  # Docker smoke test
  docker_smoke_test:
    name: Docker API Smoke Test
    runs-on: ubuntu-22.04
    steps:
      - uses: jlumbroso/free-disk-space@main
        with:
          large-packages: false
      - uses: actions/checkout@v6
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ env.MOLD_VERSION }}
      - name: Run Smoke Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-api smoke docker

  # Summary job for required status checks
  ci-success:
    name: CI Success
    runs-on: ubuntu-22.04
    needs: [lint, test, build, build_console]
    if: always()
    steps:
      - name: Check all jobs passed
        run: |
          if [[ "${{ needs.lint.result }}" != "success" ]]; then
            echo "Lint failed"
            exit 1
          fi
          if [[ "${{ needs.test.result }}" != "success" && "${{ needs.test.result }}" != "skipped" ]]; then
            echo "Test failed"
            exit 1
          fi
          if [[ "${{ needs.build.result }}" != "success" ]]; then
            echo "Build failed"
            exit 1
          fi
          echo "All required jobs passed!"
```

### Step 5: Create Deploy Workflows

#### 5a. Deploy to Devel Environment

**Create `.github/workflows/deploy-devel.yml`:**

```yaml
name: Deploy Devel

on:
  workflow_run:
    workflows: ["CI"]
    branches: [devel]
    types: [completed]

concurrency:
  group: deploy-devel
  cancel-in-progress: true

env:
  MOLD_VERSION: '2.34.1'
  API_DOCKER_IMAGE: bencher-api
  FLY_REGISTRY: registry.fly.io
  WASM_BENCHER_VALID: bencher-valid-pkg
  NETLIFY_CLI_VERSION: '18.0.4'

jobs:
  deploy_api_fly_dev:
    name: Deploy API to Fly.io Dev
    runs-on: ubuntu-22.04
    if: ${{ github.event.workflow_run.conclusion == 'success' }}
    env:
      FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
    steps:
      - uses: actions/checkout@v6
      - name: Download API Docker Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.API_DOCKER_IMAGE }}.tar.gz
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      - name: Load & Tag Local Image
        run: |
          docker load < ${{ env.API_DOCKER_IMAGE }}.tar.gz
          docker tag ${{ env.API_DOCKER_IMAGE }} ${{ env.FLY_REGISTRY }}/bencher-api-dev
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - name: Deploy Local API to Fly.io
        working-directory: ./services/api
        run: flyctl deploy --local-only --config fly/fly.dev.toml --wait-timeout 300
      - uses: ./.github/actions/setup-rust
        with:
          cache-key: cli
          mold-version: ${{ env.MOLD_VERSION }}
      - name: Run Smoke Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-api smoke dev

  deploy_api_fly_test:
    name: Deploy API to Fly.io Test
    runs-on: ubuntu-22.04
    if: ${{ github.event.workflow_run.conclusion == 'success' }}
    env:
      FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
    steps:
      - uses: actions/checkout@v6
      - name: Download API Docker Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.API_DOCKER_IMAGE }}.tar.gz
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      - name: Load & Tag Litestream Image
        run: |
          docker load < ${{ env.API_DOCKER_IMAGE }}.tar.gz
          docker tag ${{ env.API_DOCKER_IMAGE }} ${{ env.FLY_REGISTRY }}/bencher-api-test
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - name: Deploy Litestream API to Fly.io Test
        working-directory: ./services/api
        run: flyctl deploy --local-only --config fly/fly.test.toml --wait-timeout 300
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ env.MOLD_VERSION }}
      - name: Run Smoke Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-api smoke test

  deploy_console_netlify_dev:
    name: Deploy Console UI to Netlify Dev
    runs-on: ubuntu-22.04
    needs: deploy_api_fly_dev
    steps:
      - uses: actions/checkout@v6
      - name: Download `bencher_valid` Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.WASM_BENCHER_VALID }}
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
          path: ./lib/bencher_valid/pkg
      - name: Build Console UI
        working-directory: ./services/console
        env:
          SENTRY_UPLOAD: true
          SENTRY_AUTH_TOKEN: ${{ secrets.SENTRY_AUTH_TOKEN }}
        run: npm run netlify
      - name: Install Netlify CLI
        run: npm install --save-dev netlify-cli@${{ env.NETLIFY_CLI_VERSION }}
      - name: Deploy Console UI to Netlify Dev
        env:
          NETLIFY_SITE_ID: ${{ secrets.NETLIFY_SITE_ID }}
          NETLIFY_AUTH_TOKEN: ${{ secrets.NETLIFY_AUTH_TOKEN }}
          DEPLOY_MESSAGE: "Deploy from devel"
        run: |
          npx netlify-cli \
          deploy \
          --alias devel \
          --message "$DEPLOY_MESSAGE" \
          --json \
          | tee netlify.json
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ env.MOLD_VERSION }}
      - name: Run Netlify Dev Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-netlify dev devel

  build_dev_container:
    name: Build dev container
    runs-on: ubuntu-22.04
    if: ${{ github.event.workflow_run.conclusion == 'success' }}
    env:
      GITHUB_REGISTRY: ghcr.io
      DEV_CONTAINER_DOCKER_IMAGE: bencher-dev-container
    steps:
      - uses: actions/checkout@v6
      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3
      - name: Setup Docker buildx
        uses: docker/setup-buildx-action@v3
      - name: Log in to the Container registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.GITHUB_REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Pre-build dev container image
        uses: devcontainers/ci@v0.3
        with:
          imageName: ${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.DEV_CONTAINER_DOCKER_IMAGE }}
          cacheFrom: ${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.DEV_CONTAINER_DOCKER_IMAGE }}
          push: always
```

#### 5b. Deploy to Cloud/Prod Environment

**Create `.github/workflows/deploy-cloud.yml`:**

```yaml
name: Deploy Cloud

on:
  workflow_run:
    workflows: ["CI"]
    branches: [cloud]
    types: [completed]

concurrency:
  group: deploy-cloud
  cancel-in-progress: false  # Don't cancel prod deployments

env:
  MOLD_VERSION: '2.34.1'
  API_DOCKER_IMAGE: bencher-api
  FLY_REGISTRY: registry.fly.io
  WASM_BENCHER_VALID: bencher-valid-pkg
  NETLIFY_CLI_VERSION: '18.0.4'

jobs:
  deploy_api_fly_test:
    name: Deploy API to Fly.io Test
    runs-on: ubuntu-22.04
    if: ${{ github.event.workflow_run.conclusion == 'success' }}
    env:
      FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
    steps:
      - uses: actions/checkout@v6
      - name: Download API Docker Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.API_DOCKER_IMAGE }}.tar.gz
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      - name: Load & Tag Litestream Image
        run: |
          docker load < ${{ env.API_DOCKER_IMAGE }}.tar.gz
          docker tag ${{ env.API_DOCKER_IMAGE }} ${{ env.FLY_REGISTRY }}/bencher-api-test
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - name: Deploy Litestream API to Fly.io Test
        working-directory: ./services/api
        run: flyctl deploy --local-only --config fly/fly.test.toml --wait-timeout 300
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ env.MOLD_VERSION }}
      - name: Run Smoke Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-api smoke test

  deploy_api_fly_prod:
    name: Deploy API to Fly.io Prod
    runs-on: ubuntu-22.04
    needs: deploy_api_fly_test
    env:
      FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
      - name: Download API Docker Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.API_DOCKER_IMAGE }}.tar.gz
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      - name: Load & Tag Litestream Image
        run: |
          docker load < ${{ env.API_DOCKER_IMAGE }}.tar.gz
          docker tag ${{ env.API_DOCKER_IMAGE }} ${{ env.FLY_REGISTRY }}/bencher-api
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - name: Deploy API to Fly.io Prod
        working-directory: ./services/api
        run: flyctl deploy --local-only --config fly/fly.toml --wait-timeout 300
      - name: Rebase main on cloud
        run: |
          git config --global user.name "Bencher"
          git config --global user.email "git@bencher.dev"
          git fetch origin cloud
          git checkout cloud
          git pull
          git fetch origin main
          git checkout main
          git pull
          git rebase origin/cloud
          git push

  deploy_console_netlify_prod:
    name: Deploy Console UI to Netlify Prod
    runs-on: ubuntu-22.04
    needs: deploy_api_fly_prod
    steps:
      - uses: actions/checkout@v6
      - name: Download `bencher_valid` Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.WASM_BENCHER_VALID }}
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
          path: ./lib/bencher_valid/pkg
      - name: Build Console UI
        working-directory: ./services/console
        env:
          SENTRY_UPLOAD: true
          SENTRY_AUTH_TOKEN: ${{ secrets.SENTRY_AUTH_TOKEN }}
        run: npm run netlify
      - name: Install Netlify CLI
        run: npm install --save-dev netlify-cli@${{ env.NETLIFY_CLI_VERSION }}
      - name: Deploy Console UI to Netlify
        env:
          NETLIFY_SITE_ID: ${{ secrets.NETLIFY_SITE_ID }}
          NETLIFY_AUTH_TOKEN: ${{ secrets.NETLIFY_AUTH_TOKEN }}
          DEPLOY_MESSAGE: "Deploy from cloud"
        run: |
          npx netlify-cli \
          deploy \
          --prod \
          --message "$DEPLOY_MESSAGE" \
          --json \
          | tee netlify.json
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ env.MOLD_VERSION }}
      - name: Run Netlify Test
        env:
          RUST_BACKTRACE: full
        run: cargo test-netlify prod --user-agent "${{ secrets.LYCHEE_USER_AGENT }}" cloud
```

### Step 6: Create Release Workflow

**Create `.github/workflows/release.yml`:**

```yaml
name: Release

on:
  workflow_run:
    workflows: ["CI"]
    types: [completed]
    tags: [v**]

env:
  GITHUB_REGISTRY: ghcr.io
  DOCKER_HUB_ORGANIZATION: bencherdev
  API_DOCKER_IMAGE: bencher-api
  CONSOLE_DOCKER_IMAGE: bencher-console
  CLI_BIN_NAME: bencher
  MOLD_VERSION: '2.34.1'

jobs:
  release:
    name: Release Bencher
    runs-on: ubuntu-22.04
    if: ${{ github.event.workflow_run.conclusion == 'success' && startsWith(github.event.workflow_run.head_branch, 'v') }}
    env:
      BUILD_LINUX_X86_64: linux-x86-64
      BUILD_LINUX_ARM_64: linux-arm-64
      BUILD_MACOS_X86_64: macos-x86-64
      BUILD_MACOS_ARM_64: macos-arm-64
      BUILD_WINDOWS_X86_64: windows-x86-64
      BUILD_WINDOWS_ARM_64: windows-arm-64
    steps:
      - uses: actions/checkout@v6
      
      # Download all artifacts from the CI workflow
      - name: Download API Docker Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.API_DOCKER_IMAGE }}.tar.gz
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download Console Docker Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CONSOLE_DOCKER_IMAGE }}.tar.gz
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI Linux x86_64 Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_X86_64 }}
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI Linux ARM64 Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_ARM_64 }}
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI macOS x86_64 Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_MACOS_X86_64 }}
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI macOS ARM64 Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_MACOS_ARM_64 }}
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI Windows x86_64 Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_WINDOWS_X86_64 }}.exe
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI Windows ARM64 Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_WINDOWS_ARM_64 }}.exe
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI Linux x86_64 .deb Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_X86_64 }}.deb
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Download CLI Linux ARM64 .deb Artifact
        uses: actions/download-artifact@v4
        with:
          name: ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_ARM_64 }}.deb
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ secrets.GITHUB_TOKEN }}
      
      # Load Docker images
      - name: Load API Image
        run: docker load < ${{ env.API_DOCKER_IMAGE }}.tar.gz
      - name: Load Console Image
        run: docker load < ${{ env.CONSOLE_DOCKER_IMAGE }}.tar.gz
      
      # Push to GHCR
      - name: Log in to the Container registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.GITHUB_REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Tag & Push API Image
        run: |
          export GITHUB_IMAGE=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.API_DOCKER_IMAGE }}
          export TAG=${{ github.event.workflow_run.head_branch }}
          docker tag ${{ env.API_DOCKER_IMAGE }} ${GITHUB_IMAGE}:latest
          docker tag ${{ env.API_DOCKER_IMAGE }} ${GITHUB_IMAGE}:${TAG}
          docker push ${GITHUB_IMAGE}:latest
          docker push ${GITHUB_IMAGE}:${TAG}
      
      - name: Tag & Push Console Image
        run: |
          export GITHUB_IMAGE=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.CONSOLE_DOCKER_IMAGE }}
          export TAG=${{ github.event.workflow_run.head_branch }}
          docker tag ${{ env.CONSOLE_DOCKER_IMAGE }} ${GITHUB_IMAGE}:latest
          docker tag ${{ env.CONSOLE_DOCKER_IMAGE }} ${GITHUB_IMAGE}:${TAG}
          docker push ${GITHUB_IMAGE}:latest
          docker push ${GITHUB_IMAGE}:${TAG}
      
      # Push to Docker Hub
      - name: Login to Docker Hub
        uses: docker/login-action@v3
        with:
          username: ${{ secrets.DOCKER_HUB_USERNAME }}
          password: ${{ secrets.DOCKER_HUB_TOKEN }}
      
      - name: Tag & Push API Image to Docker Hub
        run: |
          export DOCKER_HUB_IMAGE=${{ env.DOCKER_HUB_ORGANIZATION }}/${{ env.API_DOCKER_IMAGE }}
          export TAG=${{ github.event.workflow_run.head_branch }}
          docker tag ${{ env.API_DOCKER_IMAGE }} ${DOCKER_HUB_IMAGE}:latest
          docker tag ${{ env.API_DOCKER_IMAGE }} ${DOCKER_HUB_IMAGE}:${TAG}
          docker push ${DOCKER_HUB_IMAGE}:latest
          docker push ${DOCKER_HUB_IMAGE}:${TAG}
      
      - name: Tag & Push Console Image to Docker Hub
        run: |
          export DOCKER_HUB_IMAGE=${{ env.DOCKER_HUB_ORGANIZATION }}/${{ env.CONSOLE_DOCKER_IMAGE }}
          export TAG=${{ github.event.workflow_run.head_branch }}
          docker tag ${{ env.CONSOLE_DOCKER_IMAGE }} ${DOCKER_HUB_IMAGE}:latest
          docker tag ${{ env.CONSOLE_DOCKER_IMAGE }} ${DOCKER_HUB_IMAGE}:${TAG}
          docker push ${DOCKER_HUB_IMAGE}:latest
          docker push ${DOCKER_HUB_IMAGE}:${TAG}
      
      # Generate release notes and publish
      - uses: rui314/setup-mold@v1
        with:
          mold-version: ${{ env.MOLD_VERSION }}
      
      - name: Generate Release Notes
        run: cargo gen-notes
      
      - name: GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ github.event.workflow_run.head_branch }}
          prerelease: ${{ contains(github.event.workflow_run.head_branch, '-rc') }}
          body_path: release-notes.md
          files: |
            ${{ env.API_DOCKER_IMAGE }}.tar.gz
            ${{ env.CONSOLE_DOCKER_IMAGE }}.tar.gz
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_X86_64 }}
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_ARM_64 }}
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_MACOS_X86_64 }}
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_MACOS_ARM_64 }}
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_WINDOWS_X86_64 }}.exe
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_WINDOWS_ARM_64 }}.exe
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_X86_64 }}.deb
            ${{ env.CLI_BIN_NAME }}-${{ github.event.workflow_run.head_branch }}-${{ env.BUILD_LINUX_ARM_64 }}.deb
```

---

## Path Filters for Optimization

The CI workflow uses `dorny/paths-filter` to determine what changed. Here are the filter patterns:

| Filter    | Paths                                                                                                         | Description           |
| --------- | ------------------------------------------------------------------------------------------------------------- | --------------------- |
| `rust`    | `Cargo.toml`, `Cargo.lock`, `lib/**`, `plus/**`, `services/api/**`, `services/cli/**`, `tasks/**`, `xtask/**` | Rust code changes     |
| `console` | `services/console/**`, `lib/bencher_valid/**`                                                                 | Console UI changes    |
| `action`  | `services/action/**`, `services/api/openapi.json`                                                             | GitHub Action changes |
| `api`     | `services/api/**`, `lib/**`, `plus/**`                                                                        | API service changes   |
| `cli`     | `services/cli/**`, `lib/**`                                                                                   | CLI changes           |
| `docker`  | `services/api/Dockerfile`, `services/console/Dockerfile`, `docker/**`                                         | Docker file changes   |

### Usage in Jobs

```yaml
jobs:
  test:
    if: needs.changes.outputs.rust == 'true' || <always run conditions>
    ...
```

---

## Caching Strategy

### Cache Keys

Use consistent cache key patterns across all workflows:

| Purpose      | Cache Key                                                               | Contents                |
| ------------ | ----------------------------------------------------------------------- | ----------------------- |
| All features | `${{ runner.os }}-cargo-all-features-${{ hashFiles('**/Cargo.lock') }}` | Full cargo build cache  |
| API only     | `${{ runner.os }}-cargo-api-${{ hashFiles('**/Cargo.lock') }}`          | API build cache         |
| CLI only     | `${{ runner.os }}-cargo-cli-${{ hashFiles('**/Cargo.lock') }}`          | CLI build cache         |
| WASM         | `${{ runner.os }}-cargo-wasm-${{ hashFiles('**/Cargo.lock') }}`         | WASM build cache        |
| Nightly      | `${{ runner.os }}-cargo-nightly-${{ hashFiles('**/Cargo.lock') }}`      | Nightly toolchain cache |
| Audit        | `${{ runner.os }}-cargo-audit-${{ hashFiles('**/Cargo.lock') }}`        | Audit cache             |

### Docker Cache

Docker images use registry-based caching:

```yaml
cache-from: type=registry,ref=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.API_DOCKER_IMAGE }}:cache-${{ github.ref_name }}
cache-to: type=registry,ref=${{ env.GITHUB_REGISTRY }}/${{ github.repository_owner }}/${{ env.API_DOCKER_IMAGE }}:cache-${{ github.ref_name }},mode=max
```

### Cache Sharing Between Branches

Caches are shared via `restore-keys` fallbacks:

```yaml
key: ${{ runner.os }}-cargo-all-features-${{ hashFiles('**/Cargo.lock') }}
restore-keys: |
  ${{ runner.os }}-cargo-all-features
  ${{ runner.os }}-cargo
```

This allows PRs to benefit from caches built on `devel` or `cloud`.

---

## Testing Your Changes

### 1. Validate Workflow Syntax

```bash
# Install actionlint
brew install actionlint

# Validate all workflows
actionlint .github/workflows/*.yml
```

### 2. Test Locally with act

```bash
# Install act
brew install act

# Run a specific job
act -j lint --secret-file .secrets

# Run PR workflow
act pull_request
```

### 3. Create a Test Branch

```bash
# Create a feature branch
git checkout -b refactor/split-workflows

# Make your changes
# ...

# Push and create a PR to devel
git push origin refactor/split-workflows
```

### 4. Verify Workflow Behavior

Check that:
- [ ] PRs trigger CI workflow
- [ ] PRs without `deploy` label don't deploy
- [ ] PRs with `deploy` label deploy to test environment
- [ ] Push to `devel` triggers CI + deploy-devel
- [ ] Push to `cloud` triggers CI + deploy-cloud
- [ ] Tags trigger CI + release
- [ ] Caches are being shared effectively

---

## Migration Checklist

1. [ ] Create `.github/actions/setup-rust/action.yml`
2. [ ] Create `.github/actions/setup-node/action.yml`
3. [ ] Create `.github/workflows/lint.yml`
4. [ ] Create `.github/workflows/test.yml`
5. [ ] Create `.github/workflows/build.yml`
6. [ ] Create `.github/workflows/ci.yml`
7. [ ] Create `.github/workflows/deploy-devel.yml`
8. [ ] Create `.github/workflows/deploy-cloud.yml`
9. [ ] Create `.github/workflows/release.yml`
11. [ ] Rename old `bencher.yml` to `bencher.yml.bak`
12. [ ] Test all trigger scenarios
13. [ ] Delete backup file after verification
14. [ ] Update branch protection rules to use new workflow names

---

## Troubleshooting

### Common Issues

1. **Artifacts not found in workflow_run triggers**
   - Ensure you're using the `run-id` parameter with `github.event.workflow_run.id`
   - Artifacts must be uploaded in the triggering workflow

2. **Composite actions not found**
   - Composite actions in the same repo must use `uses: ./.github/actions/action-name`
   - The action.yml must be present at checkout time

3. **Secrets not available in reusable workflows**
   - Secrets must be explicitly passed to reusable workflows
   - Use `secrets: inherit` or pass individually

4. **Cache not being restored**
   - Check cache key patterns match exactly
   - Ensure restore-keys provide proper fallbacks
   - Verify the cache isn't being evicted due to size limits

### Debugging

```yaml
# Add debug step to any job
- name: Debug context
  run: |
    echo "Event: ${{ github.event_name }}"
    echo "Ref: ${{ github.ref }}"
    echo "SHA: ${{ github.sha }}"
    echo "Actor: ${{ github.actor }}"
```

---

## Summary

This refactoring:

1. **Splits ~1200 lines** into focused, maintainable workflow files
2. **Reduces duplication** through composite actions and reusable workflows
3. **Improves caching** with consistent keys and fallback patterns
4. **Optimizes execution** with path-based filtering
5. **Gates PR deployments** behind a `deploy` label
6. **Maintains all existing functionality** for `devel`, `cloud`, and tag releases

The new structure is more maintainable, faster for focused changes, and provides better visibility into what's running and why.
