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
differentiated by tags and support tiers, not by branches.** There is a single
family of release branches (`release/<MAJOR>.<MINOR>.<YYMM>`) driven by OpenHCL
servicing. OpenVMM releases are tags, not branches, so OpenVMM adds no additional
branch or backport surface.

## OpenVMM: live at head, with `0.x.y` tags

OpenVMM is pre-1.0 and does not maintain release branches of its own.

- **`main` is the supported line.** All work lands in `main` first, and we hold
  it to a high quality bar so that it is safe to build from.
- **Tagged `0.x.y` releases.** We periodically tag a snapshot of `main` as
  `0.x.y` to give consumers a stable point to name and reproduce.
- **Support policy: latest only.** The most recent tag (and `main`) is what we
  support. The recommended way to pick up a fix is to move forward to a newer
  tag or to `main`, not to request a backport.
- **No dedicated OpenVMM backports, no LTS.** We do not stand up OpenVMM
  servicing branches, and we make no guarantee of security or bug-fix backports
  to older tags.
- **Cross-version compatibility** (save state, live migration) is not guaranteed
  across tags. See [Save State](./save-state.md).

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

The move to `1.0` is the graduation gate: it is where OpenVMM commits to
semantic versioning, a deprecation contract, and a supported release line. See
[If OpenVMM ever needs its own line](#if-openvmm-ever-needs-its-own-line-break-glass).

## OpenHCL: shared servicing branches

OpenHCL is serviced through the `release/<MAJOR>.<MINOR>.<YYMM>` branches and the
phase, approval, and backport process described in
[Releases & Code Flow](./release.md). These are the only long-lived, actively
maintained release branches in the repository, and their cadence and support
tails are driven by the OpenHCL and host servicing lifecycle.

OpenVMM code is present in these branches because it is the same repository. A
fix landing in a `release/*` branch is taken for OpenHCL's benefit, but OpenVMM
consumers benefit from it too, as described in the next section.

## How OpenVMM releases ride the shared branches

Rather than maintain a parallel branch family, OpenVMM releases reuse the
OpenHCL release branches as shared infrastructure:

- **When OpenHCL forks a `release/*.YYMM` branch, that same commit is also an
  OpenVMM release.** Tag it `0.x.y` off that branch point. Every OpenHCL fork
  therefore gives OpenVMM a *serviced* release for free, because the branch is
  already being maintained for OpenHCL.
- **Between forks, OpenVMM stays live at head**, with interim `0.x.y` tags cut
  off `main` for consumers who want something fresher than the last fork.

This produces two natural support tiers at no extra cost:

| OpenVMM tag sits on...                     | Support                                   |
| ------------------------------------------ | ----------------------------------------- |
| An in-service OpenHCL `release/*.YYMM` branch | Inherits that branch's servicing.       |
| An interim `main` snapshot, or an out-of-service branch | Roll-forward only.             |

The tradeoff is that OpenVMM's *serviced* releases arrive only as often as
OpenHCL forks a branch, on a schedule driven by OpenHCL. Consumers who want a
faster cadence track `main` or the interim tags.

## Handling fixes and backports

The fix flow is the same for both products, which is what keeps this model cheap
to run:

1. **Fix in `main` first**, always, regardless of product.
2. **Cherry-pick to the live OpenHCL `release/*.YYMM` branches** that need it,
   using the `backport_<RELEASE>` label process in
   [Releases & Code Flow](./release.md).
3. **OpenVMM consumers get the same fix automatically** because it is in `main`;
   it rolls into the next `0.x.y` tag. No separate OpenVMM backport.

The result is one fix, one forward-only development line, and N cherry-picks
driven purely by OpenHCL's support needs. OpenVMM adds zero backport surface.

## If OpenVMM ever needs its own line (break glass)

Standing up a parallel `release/openvmm-X.Y` branch family is deliberately *not*
part of this model. Separate maintained branches double the backport, CI, and
branch-management work, and the shared codebase means most commits would simply
be re-applied to a second branch for little gain. Prefer to add a branch later,
lazily, only if genuinely forced, rather than carry the process indefinitely.

Consider a dedicated OpenVMM release line only when one of these is true:

- OpenVMM reaches API stability (`1.0`) and commits to a semantic-versioning and
  deprecation contract (for example, a notice of at least two releases before
  any breaking change to the API, CLI, or device model).
- An external consumer formally needs a supported OpenVMM release on a faster
  cadence than OpenHCL forks provide.

Until one of those holds, resist creating OpenVMM release branches.

## Taking a dependency on a release

We welcome feedback, especially if you would like to depend on a reliable
release. Please reach out to the maintainers.
