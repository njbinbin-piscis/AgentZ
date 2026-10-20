# Iterative Self-Correcting Loop Rules (`ralph-loop`)

Follow an iterative, self-referential convergence loop on every multi-step task in this project:

1. **Explicit Task Ledger (`plan_todo`)**:
   - Decompose the user's goal into atomic, verifiable items in `plan_todo` at the start of the task. Keep exactly one item `in_progress` at a time.
2. **Edit → Verify → Self-Correct Loop**:
   - For each item: read target code (`file_read`), apply minimal edits (`file_edit`), and immediately verify (`read_lints` + targeted test/check).
   - If verification fails, inspect the diagnostic or test output, refine the implementation, and re-verify in the same loop.
3. **No Premature Exit**:
   - Do not stop after a partial summary while `pending` or `in_progress` items remain in `plan_todo`, unless blocked on a decision only the user can make.
