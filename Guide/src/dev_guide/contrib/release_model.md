# Release Model and Support

We ship two products from this repository, and they release differently:

- **OpenVMM**, a cross-platform VMM whose consumers build from the repo.
- **OpenHCL**, a paravisor shipped in the Azure and Windows host, with servicing
  driven by a Microsoft-internal process.

## OpenVMM: rolling release

OpenVMM is pre-1.0 and follows a rolling release: development happens on `main`,
and we tag periodic `0.x.y` snapshots as reproducibility anchors.

- **Support is latest-only.** To get any fix, move forward to a newer tag or
  `main`.
- **No backporting during bringup.** We do not maintain OpenVMM servicing
  branches or backport fixes, *including security fixes*, to older tags. If you
  are pinned to an old tag and need a fix, upgrade. No exceptions while pre-1.0.
- **`0.x.y` means no compatibility guarantee.** Bump `x` for releases that may
  break, `y` for fixes. (Cargo treats the first non-zero field as breaking, so
  `0.3` to `0.4` is a breaking bump.)

## OpenHCL: servicing branches

OpenHCL is serviced through the `release/<MAJOR>.<MINOR>.<YYMM>` branches (see
[Releases & Code Flow](./release.md)), the only long-lived maintained branches
in the repo. Fixes to shared code are cherry-picked into these branches for
OpenHCL. That is OpenHCL servicing, not OpenVMM backporting; OpenVMM stays
latest-only.

## Fix flow

1. Fix in `main` first, always.
2. For OpenHCL, cherry-pick to the live `release/*.YYMM` branches that need it.
3. For OpenVMM, nothing more: the fix rolls into the next `0.x.y` tag.

## Future direction

The rolling, no-backport model is for bringup, not forever. As OpenVMM matures we
expect to move to a [Cloud Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor)-style
model: time-based major releases (~6 weeks), a short support window with ~2
trains in service, and point releases for bug and security fixes. No LTS planned.

We take that step at `1.0` (with a semver and deprecation commitment), or sooner
if an external consumer needs fixes serviced on an older version.
