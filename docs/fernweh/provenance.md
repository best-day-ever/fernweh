# Source provenance and licensing

Fernweh keeps upstream PLANK history and the maintained BDE enhancement history.
The review is seeded from the exact commits in [source-manifest.json](source-manifest.json).
Client and Linux Host stay separate Git submodules; the Mac Host, broker and
transport stay in the parent. Public dependency URLs and their gitlinks remain
intact. The original BDE repositories are separate: they are not renamed, rewritten,
republished or deployed as part of Fernweh source publication.

The manifest records **import** commits and comparison merge bases; the parent
Git tree records the final Client/Host gitlinks. A later upstream observation
is recorded for transparency, not silently merged into the reviewed build.
To compare code, fetch the relevant upstream and use `git diff <merge-base>
<component-tip>` inside that component. A parent diff shows gitlink movement,
not every nested source change.

| Component | License evidence retained |
| --- | --- |
| Client, derived from Moonlight Qt | `apps/client/LICENSE`, original notices/history |
| Linux Host, derived from Sunshine | `apps/host/linux/LICENSE`, original notices/history |
| Broker | `apps/broker/Cargo.toml`: AGPL-3.0-or-later |
| PLANK transport | `protocol/plank-transport/Cargo.toml`: AGPL-3.0-or-later |
| Quinn and other vendored/dependent libraries | Their license files and pinned manifests; e.g. `third_party/quinn-proto-0.11.17/LICENSE-*` |

Original PLANK, Sunshine, Moonlight, Kyber/Kymux, Quinn and other contributors
retain their copyrights. The Fernweh name is downstream branding and implies no
upstream endorsement. A repository-wide license label cannot replace component
and dependency obligations. Review distributions and modified network-service
source obligations under their actual licenses before publishing; this file
does not relicense inherited source or claim ownership of upstream work.

The imported histories were already shared across several repositories. Their
presence in a private review is not a certification that every historic log,
screenshot, filename, commit message or metadata field is safe to make public.
Publication review includes all reachable histories, nested components and
hosting metadata, not just the latest README. Findings and private audit output
stay outside public Git. Any necessary sanitization uses a separate reviewed
copy; never rewrite original deployment history in place.
