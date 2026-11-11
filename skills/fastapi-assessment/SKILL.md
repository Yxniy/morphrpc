---
name: fastapi-assessment
description: Analyze FastAPI source for potential MorphRPC migration candidates and produce a non-destructive review plan.
---

# FastAPI migration assessment

Inspect the selected project's source without importing or executing it. Run `python3 scripts/analyze.py PATH` from this skill's directory, or pass the absolute path to `scripts/analyze.py` from elsewhere. The script writes a deterministic JSON assessment to standard output. It reports file and line locations, recognized routes, candidate field types, unresolved registrations, and review blockers.

Read [the assessment format and limitations](references/assessment.md) before interpreting the result. Check dependencies, middleware, authentication and authorization, validators, errors, response models, external router mounts, and side effects against the source and deployment context. Treat unresolved or unsupported behavior as unknown. Preserve the existing HTTP service while proposing any optional adapter. Assign operation IDs and finalize a compiler contract only after the application semantics and compatibility are verified.

Prepare an analysis-only plan with specific evidence, uncertainties, representative test cases, and a staged rollout and rollback proposal. For differential validation, use isolated fixtures for operations that may have side effects. Use shadow execution only after confirming it cannot repeat production side effects or impact users. Do not present generated plans as working adapters.
