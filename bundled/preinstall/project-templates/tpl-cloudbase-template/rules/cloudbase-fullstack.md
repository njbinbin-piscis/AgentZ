# Full-Stack Cloud Application Rules (`cloudbase`)

When developing frontend, backend/serverless functions, and database layers in this project:

1. **End-to-End Contract Alignment**:
   - Keep frontend request/response types, backend API schemas, and database models strictly synchronized. Use `symbol_search` and `impact` before changing shared DTOs or route signatures.
2. **Database & State Safety**:
   - Ensure schema migrations and queries are idempotent, parameterized, and indexed for primary access patterns.
   - Never hardcode environment-specific endpoints, credentials, or keys in source files; load them from environment variables or connector settings.
3. **Incremental Verification**:
   - After editing frontend or backend modules, run `read_lints` on modified files to catch type mismatches immediately before running full builds.
