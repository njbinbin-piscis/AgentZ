# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d
