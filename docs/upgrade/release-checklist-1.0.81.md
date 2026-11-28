# AgentZ 1.0.81 release checklist

2026-11-28. Based on 1.0.80: built-in browser navigation settle fix; `robotz-browser` v0.1.15.

## Release gate

- [x] Version labels are 1.0.81.
- [x] `CHANGELOG.md` and release notes updated.
- [x] `robotz-browser` nav live test (`slow_subresources_do_not_block_navigation_live`) and AgentZ `navigate_slow_subresource_live` pass; `tsc --noEmit` passes.
- [ ] Hand-check: click an agent-generated HTTPS URL in chat; panel opens promptly and loads without a 30s `Navigation failed.` alert on pages with ads/iframes.
- [ ] Release workflow for `v1.0.81` (pending).
