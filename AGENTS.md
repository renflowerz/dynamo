<!--
SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
SPDX-License-Identifier: Apache-2.0
-->

- Keep changes focused and reviewable.
- Use Conventional Commit PR titles: `type(scope): summary`. Accepted types:
  `feat`, `fix`, `docs`, `test`, `ci`, `refactor`, `perf`, `chore`, `revert`,
  `style`, and `build`.
- PR descriptions must include `Summary` and `Validation`.
- Sign every commit with DCO: `git commit -s`.
- RBAC changes for operator code must update the `+kubebuilder:rbac` markers;
  `make manifests` regenerates both the canonical role and the platform chart's
  `files/manager-rules.yaml`. Keep chart-only grants in the manual section of
  `templates/manager-rbac.yaml`.
