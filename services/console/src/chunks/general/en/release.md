# Release Checklist

- [ ] Cut the release with `gh workflow run release_version.yml --repo bencherdev/bencher -f bump=<patch|minor>`
- [ ] Once the production build is complete
  - [ ] Manually edit the release
  - [ ] Make sure that it is set to update the Bencher CLI GitHub Action default version
  - [ ] Save this version of the release
- [ ] Once the release has been updated, deploy to production with `gh workflow run rebase_cloud.yml --repo bencherdev/bencher`
