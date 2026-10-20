# Structured Test Engineering Rules (`tsmart`)

When designing tests or adding features/fixes in this project:

1. **Test Matrix Coverage**:
   - Cover four dimensions for every non-trivial function or endpoint:
     1. **Happy path**: standard valid inputs and expected state transitions.
     2. **Boundary conditions**: empty inputs, zero/max limits, Unicode/CRLF edge cases.
     3. **Negative & error paths**: invalid parameters, missing files/permissions, timeouts, and clean error recovery.
     4. **Regression protection**: a dedicated test case for every fixed bug.
2. **Deterministic & Isolated Tests**:
   - Tests must clean up temporary files/directories, avoid hardcoded sleep delays or external network dependencies, and pass reliably via `code_run` / `shell`.
