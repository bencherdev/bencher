# CI Integration

In CI, set `BENCHER_API_KEY` to a **project** API key (`bencher_run_*`) for least
privilege. Create one in the Console, store it as a CI secret, and have the job set
`BENCHER_API_KEY` from that secret in its environment rather than passing `--key`.
In the GitHub Actions examples, `<token>` stands for the workflow's `GITHUB_TOKEN`,
which the step passes to `--github-actions` through its `env:`.
See https://bencher.dev/docs/how-to/github-actions/ for the full workflow.

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
          --github-actions <token> \
          "cargo bench"
      env:
        BASE_SHA: ${{ github.event.pull_request.base.sha }}
        HEAD_SHA: ${{ github.event.pull_request.head.sha }}
```

### PR Comment Options

| Flag | Purpose |
|------|---------|
| `--github-actions <token>` | Enable PR comments (`<token>` is the workflow's `GITHUB_TOKEN`, passed through the step's `env:`) |
| `--ci-only-thresholds` | Only post if a threshold exists for the branch/testbed/measure |
| `--ci-only-on-alert` | Only post when an alert is generated |
| `--ci-id <id>` | Labels the job's Report row in the Project's PR comment, used in place of the project name in the GitHub Check name (`Bencher Report (<id>)`) |
| `--ci-number <n>` | Issue/PR number to post on |

### Bare Metal Without a Waiting Runner (Bencher Plus)

`--detach` with `--github-actions` submits the job with the PR context (repository, head SHA, PR
number, `--ci-id`, `--ci-only-thresholds`, and `--ci-only-on-alert`) and exits. Bencher starts the
GitHub Check, and when the job finishes, completes it and writes the PR comment itself, with its
GitHub App on the project's linked repository. There is no second workflow, and the workflow's
token needs no write access. A user with the Maintainer role on the project, which Organization
Leaders and server admins hold, and push access to the repository links it once in the project's
Settings, under Connections.

```yaml
# on: pull_request
submit_pr_benchmarks:
  if: github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository
  permissions:
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
          --github-actions <token>
      env:
        HEAD_SHA: ${{ github.event.pull_request.head.sha }}
```

The submit is refused when the server has no GitHub App, the project is unclaimed, or the project
has no connected linked repository for the workflow's repository. The CLI refuses the run against a
server older than v0.6.18.
`--error-on-alert` conflicts with `--detach`: an alert fails the GitHub Check instead.
The check belongs to Bencher's GitHub App, so a required status check must name the app as its source.
If the app's installation has not approved Checks and Pull requests write, the post fails and the
CLI prints the approval link when it sees the failure.
Give each detached job of one PR its own `--ci-id`, so each keeps its own Report row in the PR comment.
Anyone with the project's API key can make Bencher post a check or comment to the linked
repository, so keep the key secret.

### On-the-Fly Project Creation

For repos that want benchmarks without pre-creating the Bencher project:
```bash
bencher run --ci-on-the-fly "cargo bench"
```

With a user API key, the new project goes under the user's personal organization.
Without a key, it is a public `unclaimed` project that anyone can view and post results to.

## GitLab CI/CD

GitLab uses `--format html` to capture a report, then posts it as an MR note.
See https://bencher.dev/docs/how-to/gitlab-ci-cd/ for the full benchmark jobs.

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
```

Post it from a `post_mr_report` job in the `report` stage that runs on
`merge_request_event` with `when: always`, takes `report.html` from `benchmark_mr`
through `dependencies:`, sets `GIT_STRATEGY: none` so it never checks out the MR,
and runs in the `mr-report` environment with `action: prepare`. It exits early when
`report.html` is missing or empty, and otherwise POSTs `report.html` as the note body
to the merge request notes endpoint,
`/api/v4/projects/<project id>/merge_requests/<MR iid>/notes` (`CI_PROJECT_ID` and
`CI_MERGE_REQUEST_IID`, under `CI_API_V4_URL`), with a `PRIVATE-TOKEN` header
carrying `GITLAB_ACCESS_TOKEN`, and fails the job on an HTTP error.

`GITLAB_ACCESS_TOKEN` is a project access token with the `api` scope, stored as a
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
- `--start-point-idempotency-key`: Share one reset branch head between the parallel jobs of one CI run. GitHub Actions sets it to the workflow run attempt. Elsewhere, use a pipeline ID (ex: `--start-point-idempotency-key "$CI_PIPELINE_ID"`)

## Branch Cleanup

Archive stale branches (e.g., when a PR is closed):
```bash
bencher archive --project my-project --branch old-feature-branch
```

Unarchive if needed:
```bash
bencher unarchive --project my-project --branch old-feature-branch
```
