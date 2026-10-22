# CI Integration

In CI, set `BENCHER_API_KEY` to a **project** API key (`bencher_run_*`) for least
privilege. Create one in the Console and store it as a CI secret.

## GitHub Actions (Native)

Bencher has native GitHub Actions support for posting benchmark results
as PR comments.

### Target Branch (runs on push to main)

```yaml
- uses: bencherdev/bencher@main
- run: |
    bencher run \
      --project my-project \
      --branch main \
      --testbed github-actions \
      --threshold-measure latency \
      --threshold-test t_test \
      --threshold-upper-boundary 0.99 \
      --error-on-alert \
      "cargo bench"
  env:
    BENCHER_API_KEY: ${{ secrets.BENCHER_API_KEY }}
```

### Pull Request (runs on PR)

Run the PR workflow on `pull_request` for same-repository PRs only, and never on
`pull_request_target` with secrets.

```yaml
# on: pull_request
benchmark_pr:
  if: github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository
  permissions:
    checks: write
    contents: read
    pull-requests: write
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v6
    - uses: bencherdev/bencher@main
    - run: |
        bencher run \
          --project my-project \
          --branch "$GITHUB_HEAD_REF" \
          --hash "$HEAD_SHA" \
          --start-point "$GITHUB_BASE_REF" \
          --start-point-hash "$BASE_SHA" \
          --start-point-clone-thresholds \
          --start-point-reset \
          --testbed github-actions \
          --error-on-alert \
          --github-actions "$GITHUB_TOKEN" \
          "cargo bench"
      env:
        BASE_SHA: ${{ github.event.pull_request.base.sha }}
        BENCHER_API_KEY: ${{ secrets.BENCHER_API_KEY }}
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        HEAD_SHA: ${{ github.event.pull_request.head.sha }}
```

### PR Comment Options

| Flag | Purpose |
|------|---------|
| `--github-actions <token>` | Enable PR comments (pass `secrets.GITHUB_TOKEN` through `env:`) |
| `--ci-only-thresholds` | Only post if a threshold exists for the branch/testbed/measure |
| `--ci-only-on-alert` | Only post when an alert is generated |
| `--ci-public-links` | Use public URLs (no login required to view) |
| `--ci-id <id>` | Names the job's section in the Project's PR comment, used in place of the project name in the GitHub Check name (`Bencher Report (<id>)`) |
| `--ci-number <n>` | Issue/PR number to post on |
| `--ci-callback-token <token>` | Fine-grained personal access token for this one repository with only contents write, for the `repository_dispatch` a detached bare metal run sends (requires `--github-actions` and `--detach`) |

### Bare Metal Without a Waiting Runner (Bencher Plus)

`--detach` with `--github-actions` starts the GitHub Check, submits the job, and exits.
When the job finishes, Bencher sends a `repository_dispatch` (event type `bencher_run`) with
`--ci-callback-token`, and a second workflow on the default branch attaches to the job and
completes the check and the PR comment:

```yaml
# Workflow 1, on pull_request
submit_pr_benchmarks:
  if: github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository
  permissions:
    checks: write
    contents: read
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v6
    - uses: bencherdev/bencher@main
    - run: |
        bencher run \
          --project my-project \
          --branch "$GITHUB_HEAD_REF" \
          --hash "$HEAD_SHA" \
          --image my-bench:latest \
          --detach \
          --github-actions "$GITHUB_TOKEN" \
          --ci-callback-token "$BENCHER_CALLBACK_TOKEN"
      env:
        BENCHER_API_KEY: ${{ secrets.BENCHER_API_KEY }}
        BENCHER_CALLBACK_TOKEN: ${{ secrets.BENCHER_CALLBACK_TOKEN }}
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        HEAD_SHA: ${{ github.event.pull_request.head.sha }}
```

```yaml
# Workflow 2, on repository_dispatch with types [bencher_run]
attach_pr_benchmarks:
  permissions:
    checks: write
    pull-requests: write
  runs-on: ubuntu-latest
  steps:
    - uses: bencherdev/bencher@main
    - run: |
        bencher run \
          --project my-project \
          --job "$JOB_UUID" \
          --github-actions "$GITHUB_TOKEN" \
          --error-on-alert
      env:
        BENCHER_API_KEY: ${{ secrets.BENCHER_API_KEY }}
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        JOB_UUID: ${{ github.event.client_payload.bencher.job }}
```

The payload's `bencher` object holds the Job UUID (`job`) and the Project slug (`project`), and the attach reads the rest of it.
`--detach` with `--github-actions` requires `--ci-callback-token`, and `--callback-url` conflicts with `--github-actions`.
Give each detached job of one PR its own `--ci-id`, so each keeps its own section of the PR comment.
The dispatch to `api.github.com`, which carries `--ci-callback-token` as a `Bearer` token and the `bencher_run` event, goes out on any plan for a claimed project. On GitHub Enterprise Server it also needs a Bencher Plus plan. Otherwise the callback is skipped and the check completes as neutral.

### On-the-Fly Project Creation

For repos that want benchmarks without pre-creating the Bencher project:
```bash
bencher run --ci-on-the-fly "cargo bench"
```

With a user API key, the new project goes under the user's personal organization.
Without a key, it is a public `unclaimed` project that anyone can view and post results to.

## GitLab CI/CD

GitLab uses `--format html` to capture a report, then posts it as an MR note.

### Target Branch Job

```yaml
benchmark_main:
  stage: benchmark
  rules:
    - if: $CI_COMMIT_BRANCH == "main"
  script:
    - bencher run
        --project my-project
        --branch main
        --testbed gitlab-ci
        --threshold-measure latency
        --threshold-test t_test
        --threshold-upper-boundary 0.99
        --error-on-alert
        "cargo bench"
```

### Merge Request Jobs

The job that runs MR code saves the report as an artifact; a separate job that
never checks out the MR posts it.
Add `report` after `benchmark` in your `stages:`.

```yaml
benchmark_mr:
  stage: benchmark
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  script:
    - |
      bencher run \
        --project my-project \
        --branch "$CI_COMMIT_REF_NAME" \
        --start-point "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" \
        --start-point-hash "$CI_MERGE_REQUEST_TARGET_BRANCH_SHA" \
        --start-point-clone-thresholds \
        --start-point-reset \
        --start-point-idempotency-key "$CI_PIPELINE_ID" \
        --testbed gitlab-ci \
        --error-on-alert \
        --quiet \
        --format html \
        "cargo bench" > report.html
  artifacts:
    when: always
    paths:
      - report.html

post_mr_report:
  stage: report
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
      when: always
  dependencies:
    - benchmark_mr
  environment:
    name: mr-report
    action: prepare
  variables:
    GIT_STRATEGY: none
  script:
    - '[ -s report.html ] || exit 0'
    - |
      curl --fail --request POST \
        --header "PRIVATE-TOKEN: $GITLAB_ACCESS_TOKEN" \
        --data-urlencode "body@report.html" \
        "https://gitlab.com/api/v4/projects/$CI_PROJECT_ID/merge_requests/$CI_MERGE_REQUEST_IID/notes"
```

`$GITLAB_ACCESS_TOKEN` is a project access token with the `api` scope, stored as a
masked CI/CD variable scoped to the `mr-report` environment, so the job that runs
MR code does not receive it. The built-in `CI_JOB_TOKEN` cannot post MR notes.

## Generic CI (Any Platform)

The pattern works on any CI platform:

1. Run benchmarks on the target branch (push to main) with thresholds
2. Run benchmarks on PRs/MRs using `--start-point` to inherit thresholds
3. Use `--format html` and post results via the platform's API

Key environment variables to map from your CI platform:
- Current branch name -> `--branch`
- Target/base branch name -> `--start-point`
- Target/base branch commit hash -> `--start-point-hash`

## Branch Management for PRs

When benchmarking a PR/MR branch:
```bash
--branch feature-branch \
--start-point main \
--start-point-hash abc123... \
--start-point-clone-thresholds \
--start-point-reset
```

- `--start-point-clone-thresholds`: Copy threshold configuration from the base branch
- `--start-point-reset`: Reset the branch head so only PR data is included
- `--start-point-idempotency-key`: Share one reset branch head between the parallel jobs of one CI run. GitHub Actions sets it to the workflow run attempt. Elsewhere, use a pipeline ID (ex: `BENCHER_START_POINT_IDEMPOTENCY_KEY="$CI_PIPELINE_ID"`)

## Branch Cleanup

Archive stale branches (e.g., when a PR is closed):
```bash
bencher archive --project my-project --branch old-feature-branch
```

Unarchive if needed:
```bash
bencher unarchive --project my-project --branch old-feature-branch
```
