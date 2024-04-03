# Release Checklist

- [ ] Update the changelog and remove the `Pending` prefix for the release version
- [ ] Update the `Cargo.toml` file `workspace.package.version` number
- [ ] Run the `./scripts/tag.sh` script to tag the branch and cut a release
- [ ] Once the production build is complete
  - [ ] Manually edit the release
  - [ ] Make sure that it is set to update the Bencher CLI GitHub Action default version
  - [ ] Save this version of the release
- [ ] Once the release has been updated, run the `./scripts/push.sh` script to push to production
