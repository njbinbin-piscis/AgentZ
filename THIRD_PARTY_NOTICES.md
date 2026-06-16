# Third-Party Notices

## CodeBuddy Marketplace Preinstall (AgentZ v0.6.0)

Source: https://cnb.cool/codebuddy/marketplace.git

After compatibility audit, bundled preinstall contains:

| Category | Bundled | Excluded |
|----------|--------:|---------:|
| Skills | 253 | 3 |
| Agents | 233 | 103 |
| Slash commands | 336 | 36 |
| Project templates | 9 | — |

Excluded ids: `bundled/codebuddy/exclude.json`  
Audit report: `bundled/codebuddy/compatibility-report.json`

### Tool name mapping (CodeBuddy → AgentZ)

| CodeBuddy | AgentZ |
|-----------|--------|
| Read | file_read |
| Write | file_write |
| Edit | file_edit |
| Bash | shell |
| Grep | codebase_search |
| Glob | file_list |

### Regenerate

```bash
npm run import:codebuddy
npm run remediate:preinstall
npm run lint:preinstall -- --strict
npm run verify:preinstall -- --strict
```

### Security

- Preinstall copies are write-if-absent.
- Imported hooks default to `enabled: false`.
- Hub-locked skills (`source: codebuddy`) resist silent overwrite.
