# Licence and additional permissions

## Software

Muster is licensed under the GNU Affero General Public License, version 3 or (at your option) any later version. The full text goes in `LICENSE`. Fetch it from https://www.gnu.org/licenses/agpl-3.0.txt; do not retype it.

Each source file carries the header (in the comment syntax of its language):

```
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception
```

SPDX 3 introduces `AdditionRef-` for custom licence additions, but the REUSE 3.3 specification does not yet accept it, so the project uses `LicenseRef-Muster-app-store-exception` after `WITH`. Switch to `AdditionRef-` once REUSE supports it. The text is in `LICENSES/LicenseRef-Muster-app-store-exception.txt`. Files that cannot carry a header are covered by `REUSE.toml`.

## Additional permission: app store distribution (DRAFT)

> **Draft for legal review before the first external contribution is accepted.** It is modelled on additional permissions used by other GPL-family projects that ship mobile apps. Have it checked by someone with open-source licensing expertise, and settle the final wording before anyone outside OHTF contributes. Adding it later requires agreement from every copyright holder.

Proposed text:

> **Additional permission under GNU AGPL version 3 section 7**
>
> As an additional permission under section 7 of the GNU Affero General Public License, version 3, you may distribute covered works, in object code form, through an application store or similar distribution platform (for example the Apple App Store or Google Play), even if the terms and conditions of that platform impose restrictions on recipients that would otherwise conflict with this License. This permission applies only if:
>
> 1. the Corresponding Source of the covered work is made available to all recipients, free of charge, under the terms of this License (with or without this additional permission), through a channel that does not impose those restrictions; and
> 2. the distribution makes clear to recipients where that Corresponding Source can be obtained.
>
> You may remove this additional permission from your copy of the covered work. If you modify the covered work, you may extend this additional permission to your version, but you are not required to.

## Network use (AGPL section 13)

The relay and any hosted component must offer users a way to get the Corresponding Source of the running version. The relay serves `GET /source`, which redirects to the configured source URL (by default the upstream repository at the matching version tag).

## Trade marks

The AGPL does not grant rights to the project name or logo, or to OHTF branding. OHTF will publish a trade mark policy. Forks must use a different name. Only OHTF-approved hardware may carry OHTF branding.

## Documentation

Documentation, specification, ADRs and safety documents (`spec/`, `safety/`, `docs/` and the root Markdown files) are licensed CC-BY-SA-4.0 (confirmed by Marcus Baw, 2026-11-14). Generated alarm sounds under `sounds/` are CC0-1.0.

## Hardware

Hardware designs under `hardware/` (post-v1) will be licensed CERN-OHL-S-2.0.
