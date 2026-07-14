# Release Model and Support

## Two products, one repository

This repository builds two related but distinct products, and they have
different release and support expectations even though they share code and
history:

- **OpenVMM** is a cross-platform Virtual Machine Monitor. Its consumers build
  or take binaries directly from this repository and generally want to track
  recent development.
- **OpenHCL** is a paravisor built on top of OpenVMM. It ships as a component of
  the Azure and Windows host, and its release cadence and in-market servicing
  are governed by a Microsoft-internal process.

Because the two products serve very different consumers, do not assume a single
cadence or support window applies to both. When a release question comes up, the
first thing to establish is *which product* it is about.

The core principle of this model is: **one branch family, two products,
differentiated by tags and support, not by branches.** OpenVMM follows a
**rolling release** with no backporting during bringup (tags off `main`,
latest-only support). OpenHCL is serviced through a single family of release
branches (`release/<MAJOR>.<MINOR>.<YYMM>`). OpenVMM releases are tags, not
branches, so OpenVMM adds no branch or backport surface.

## OpenVMM: rolling release

OpenVMM currently follows a **rolling release** model. Development happens
continuously on `main`, and we publish periodic tags as reproducibility anchors.
This is the right posture while the project is in bringup: it keeps process
overhead near zero and matches how OpenVMM's consumers actually use it (tracking
recent development).

- **`main` is the supported line.** All work lands in `main` first, and we hold
  it to a high quality bar so that it is safe to build from.
- **Tagged `0.x.y` releases.** We periodically tag a snapshot of `main` as
  `0.x.y` to give consumers a stable point to name and reproduce. A tag is an
  anchor, not a branch, and carries no support obligation of its own.
- **Support policy: latest only.** The most recent tag (and `main`) is what we
  support. The way to pick up any fix is to move forward to a newer tag or to
  `main`.
- **No backporting during bringup.** This is a deliberate, named policy, not an
  oversight. We do not stand up OpenVMM servicing branches and do not backport
  fixes (including security fixes) to older tags. See
  [No backporting, and the security caveat](#no-backporting-and-the-security-caveat).
- **Cross-version compatibility** (save state, live migration) is not guaranteed
  across tags. See [Save State](./save-state.md).

### No backporting, and the security caveat

Backporting is the single largest ongoing cost of a release process, so during
bringup we do not do it for OpenVMM. The trade-off is explicit: if a critical
fix (including a security fix) lands and you are pinned to an older tag, the
answer is **upgrade to the latest tag or `main`**. There are no exceptions while
OpenVMM is pre-1.0.

This is an acceptable posture during bringup because OpenVMM has few if any
consumers who cannot roll forward. It is also the main motivation for the
eventual graduation described in [Future direction](#future-direction).

Note that "no backporting" is a statement about OpenVMM's *own* releases. Fixes
to code shared with OpenHCL are still cherry-picked into the OpenHCL
`release/*.YYMM` servicing branches, because OpenHCL has real in-market servicing
obligations. That is OpenHCL servicing, not OpenVMM backporting. See
[OpenHCL: shared servicing branches](#openhcl-shared-servicing-branches).

### Pre-1.0 versioning

While OpenVMM is on `0.x.y`, it makes **no backward-compatibility guarantee**.
That is the reason for staying on `0`: it signals to consumers that the API,
CLI, device model, and on-disk formats may change.

- Bump `x` (the minor field) for any release that may include breaking changes.
- Bump `y` (the patch field) for fixes released on top of an existing `x`.
- Patch releases are also tags off `main` (roll forward). They are not a
  branched servicing line.
- **Cargo note:** Cargo treats the first non-zero version field as the breaking
  coordinate, so `0.3.0` to `0.4.0` is a breaking bump and a `^0.3` dependency
  will not automatically move to `0.4`. Keep version bumps aligned with actual
  breakage so downstream version ranges behave as expected.

The move to `1.0` is the graduation gate. See [Future direction](#future-direction).

## OpenHCL: shared servicing branches

OpenHCL is serviced through the `release/<MAJOR>.<MINOR>.<YYMM>` branches and the
phase, approval, and backport process described in
[Releases & Code Flow](./release.md). These are the only long-lived, actively
maintained release branches in the repository, and their cadence and support
tails are driven by the OpenHCL and host servicing lifecycle.

OpenVMM code is present in these branches because it is the same repository. A
fix landing in a `release/*` branch is taken for OpenHCL's benefit. OpenVMM's
own support policy remains latest-only regardless of what lands on those
branches.

## Relationship between OpenVMM tags and OpenHCL branches

OpenVMM does not maintain a parallel branch family. OpenVMM tags are cut off
`main`; the OpenHCL `release/*.YYMM` branches exist for OpenHCL servicing. The
two intersect only incidentally:

- When OpenHCL forks a `release/*.YYMM` branch, that fork point is a fine commit
  to also tag as an OpenVMM `0.x.y` release.
- Because OpenHCL continues to service shared code on that branch, an OpenVMM
  build taken from it may in practice receive fixes. This is an incidental
  benefit of the shared repository, **not** an OpenVMM support commitment.

During bringup, do not present OpenHCL branch servicing as an OpenVMM support
tier. OpenVMM's promise is latest-only, and the guidance for any OpenVMM
consumer needing a fix is to roll forward.

## Handling fixes

The fix flow is the same for both products, which is what keeps this model cheap
to run:

1. **Fix in `main` first**, always, regardless of product.
2. **For OpenHCL**, cherry-pick to the live `release/*.YYMM` branches that need
   it, using the `backport_<RELEASE>` label process in
   [Releases & Code Flow](./release.md).
3. **For OpenVMM**, do nothing further: the fix is in `main` and rolls into the
   next `0.x.y` tag. There is no OpenVMM backport.

The result is one fix, one forward-only development line for OpenVMM, and
cherry-picks driven purely by OpenHCL's servicing needs. OpenVMM adds zero
backport surface.

## Future direction

The rolling, no-backport model is a starting posture for bringup, not the
destination. As OpenVMM matures we expect to adopt a model similar to the
[Cloud Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor)
project: time-based major releases on a regular cadence (Cloud Hypervisor cuts a
major roughly every six weeks), a defined support window with a small number of
releases in service at once (Cloud Hypervisor supports each major for about
twelve weeks, so roughly two trains overlap), and point releases carrying bug
and security fixes. Cloud Hypervisor has no long-term support (LTS) line; in
practice few consumers ask for one, and those that need it maintain their own.
We do not plan an LTS line either.

Adopting that model means standing up a supported OpenVMM release line, with the
associated branches and backporting. We take that step only when one of these is
true:

- OpenVMM reaches API stability (`1.0`) and commits to semantic versioning and a
  deprecation contract (for example, a notice of at least two releases before
  any breaking change to the API, CLI, or device model).
- An external consumer formally needs a supported OpenVMM release with fixes
  serviced on an older version.

### Proposed branch structure

When we are ready to stand up that line, the proposal is to unify both products
on a single `YYMM` branch scheme:

- **Cut `YYMM` branches that ship OpenVMM** on a regular cadence, for example
  monthly (`2607`, `2608`, `2609`, ...).
- **OpenHCL snaps to a chosen subset of those branches** on its own schedule
  (for example semi-annually), rather than forking its own separate line. The
  branch OpenHCL picks becomes its `release/<MAJOR>.<MINOR>.<YYMM>` servicing
  branch.
- This unifies the branch structure across both products and lets OpenVMM's
  "major" simply be a monthly branch, avoiding a separate versioning scheme.
- Longer term, these same `YYMM` branches are the natural place for any LTS
  designation, should we ever decide to offer one.

Until we adopt this, we deliberately keep the rolling model and do not create
OpenVMM release branches.

## Taking a dependency on a release

We welcome feedback, especially if you would like to depend on a reliable
release. Please reach out to the maintainers.
