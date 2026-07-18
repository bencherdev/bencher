# CI Integration

In CI, set `BENCHER_API_KEY` to a **project** API key (`bencher_run_*`) for least
privilege. Create one with `bencher project key create <project> --name ci` (or in the
Console) and store it as a CI secret.

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

```yaml
- uses: bencherdev/bencher@main
- run: |
    bencher run \
      --project my-project \
      --branch "${{ github.head_ref }}" \
      --start-point "${{ github.base_ref }}" \
      --start-point-hash "${{ github.event.pull_request.base.sha }}" \
      --start-point-clone-thresholds \
      --start-point-reset \
      --testbed github-actions \
      --error-on-alert \
      --github-actions "${{ secrets.GITHUB_TOKEN }}" \
      "cargo bench"
  env:
    BENCHER_API_KEY: ${{ secrets.BENCHER_API_KEY }}
```

### PR Comment Options

| Flag | Purpose |
|------|---------|
| `--github-actions <token>` | Enable PR comments (pass `${{ secrets.GITHUB_TOKEN }}`) |
| `--ci-only-thresholds` | Only post if a threshold exists for the branch/testbed/measure |
| `--ci-only-on-alert` | Only post when an alert is generated |
| `--ci-public-links` | Use public URLs (no login required to view) |
| `--ci-id <id>` | Custom identifier for the CI comment |
| `--ci-number <n>` | Issue/PR number to post on |

### On-the-Fly Project Creation

For repos that want benchmarks without pre-creating the Bencher project:
```bash
bencher run --ci-on-the-fly "cargo bench"
```

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

### Merge Request Job

```yaml
benchmark_mr:
  stage: benchmark
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  script:
    - |
      REPORT=$(bencher run \
        --project my-project \
        --branch "$CI_COMMIT_REF_NAME" \
        --start-point "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" \
        --start-point-hash "$CI_MERGE_REQUEST_TARGET_BRANCH_SHA" \
        --start-point-clone-thresholds \
        --start-point-reset \
        --testbed gitlab-ci \
        --error-on-alert \
        --format html \
        "cargo bench")
    - |
      curl --request POST \
        --header "PRIVATE-TOKEN: $GITLAB_ACCESS_TOKEN" \
        --data-urlencode "body=$REPORT" \
        "https://gitlab.com/api/v4/projects/$CI_PROJECT_ID/merge_requests/$CI_MERGE_REQUEST_IID/notes"
```

`$GITLAB_ACCESS_TOKEN` is a project access token with the `api` scope, stored as a
masked CI/CD variable. The built-in `CI_JOB_TOKEN` cannot post MR notes.

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

## Branch Cleanup

Archive stale branches (e.g., when a PR is closed):
```bash
bencher archive --project my-project --branch old-feature-branch
```

Unarchive if needed:
```bash
bencher unarchive --project my-project --branch old-feature-branch
```
