#!/usr/bin/env python3
"""
Discover available MCP servers in a WeData workspace.

This script calls the ListMCPServerConfigs API to retrieve all MCP servers
configured in the current workspace, using TC3-HMAC-SHA256 authentication.

Authentication follows the official Tencent Cloud API v3 signing spec:
https://cloud.tencent.com/document/product/213/30654

Credentials and workspace config are read from .env.local via wedata.base.config.

Usage (run from project root):
    python <skill_dir>/scripts/discover_mcp_servers.py
    python <skill_dir>/scripts/discover_mcp_servers.py --format json
    python <skill_dir>/scripts/discover_mcp_servers.py --output tools.md
"""

import argparse
import hashlib
import hmac
import json
import sys
import time
from datetime import datetime, timezone
from http.client import HTTPSConnection
from pathlib import Path
from typing import Any, Dict, List

# ── Ensure the project root is on sys.path so we can import wedata.* ──
# The script should be executed from the project root (cwd).
# AI always cd's to the project directory before running this script.
_PROJECT_ROOT = Path.cwd()
sys.path.insert(0, str(_PROJECT_ROOT))

from wedata.base.config import AppConfig  # noqa: E402

# ── Constants ──
_API_ACTION = "ListMCPServerConfigs"
_API_VERSION = "2025-10-10"
_SERVICE = "wedata"
_ALGORITHM = "TC3-HMAC-SHA256"
_CONTENT_TYPE = "application/json; charset=utf-8"
_DEFAULT_PAGE_SIZE = 50


# ── TC3-HMAC-SHA256 Signing (official Tencent Cloud API v3 spec) ──

def _sign(key: bytes, msg: str) -> bytes:
    """HMAC-SHA256 sign a message with a key."""
    return hmac.new(key, msg.encode("utf-8"), hashlib.sha256).digest()


def build_tc3_headers(
    secret_id: str,
    secret_key: str,
    host: str,
    action: str,
    payload: str,
    region: str = "",
    token: str = "",
    version: str = _API_VERSION,
) -> dict:
    """Build complete request headers with TC3-HMAC-SHA256 authorization.

    Follows the official signing spec exactly:
    1. Canonical request includes content-type, host, x-tc-action in signed headers
    2. Content-Type is 'application/json; charset=utf-8'
    3. x-tc-action is lowercased in canonical headers

    Reference: https://cloud.tencent.com/document/product/213/30654
    """
    timestamp = int(time.time())
    date = datetime.fromtimestamp(timestamp, tz=timezone.utc).strftime("%Y-%m-%d")

    # ── Step 1: Build canonical request ──
    http_request_method = "POST"
    canonical_uri = "/"
    canonical_querystring = ""
    canonical_headers = (
        "content-type:%s\nhost:%s\nx-tc-action:%s\n"
        % (_CONTENT_TYPE, host, action.lower())
    )
    signed_headers = "content-type;host;x-tc-action"
    hashed_request_payload = hashlib.sha256(payload.encode("utf-8")).hexdigest()
    canonical_request = (
        http_request_method + "\n"
        + canonical_uri + "\n"
        + canonical_querystring + "\n"
        + canonical_headers + "\n"
        + signed_headers + "\n"
        + hashed_request_payload
    )

    # ── Step 2: Build string to sign ──
    credential_scope = date + "/" + _SERVICE + "/" + "tc3_request"
    hashed_canonical_request = hashlib.sha256(
        canonical_request.encode("utf-8")
    ).hexdigest()
    string_to_sign = (
        _ALGORITHM + "\n"
        + str(timestamp) + "\n"
        + credential_scope + "\n"
        + hashed_canonical_request
    )

    # ── Step 3: Calculate signature ──
    secret_date = _sign(("TC3" + secret_key).encode("utf-8"), date)
    secret_service = _sign(secret_date, _SERVICE)
    secret_signing = _sign(secret_service, "tc3_request")
    signature = hmac.new(
        secret_signing, string_to_sign.encode("utf-8"), hashlib.sha256
    ).hexdigest()

    # ── Step 4: Build Authorization header ──
    authorization = (
        _ALGORITHM + " "
        + "Credential=" + secret_id + "/" + credential_scope + ", "
        + "SignedHeaders=" + signed_headers + ", "
        + "Signature=" + signature
    )

    # ── Step 5: Assemble full headers ──
    headers = {
        "Authorization": authorization,
        "Content-Type": _CONTENT_TYPE,
        "Host": host,
        "X-TC-Action": action,
        "X-TC-Timestamp": str(timestamp),
        "X-TC-Version": version,
    }
    if region:
        headers["X-TC-Region"] = region
    if token:
        headers["X-TC-Token"] = token

    return headers


# ── API Call ──

def build_request_body(
    workspace_id: str,
    page_number: int = 1,
    page_size: int = _DEFAULT_PAGE_SIZE,
) -> dict:
    """Build the ListMCPServerConfigs request body."""
    return {
        "WorkspaceId": workspace_id,
        "PageRequest": {
            "PageNumber": page_number,
            "PageSize": page_size,
            "AllPage": False,
        },
    }


def call_list_mcp_servers(config: AppConfig) -> Dict[str, Any]:
    """Call ListMCPServerConfigs API and return all MCP servers.

    Automatically paginates to collect all items across pages.
    Uses standalone TC3-HMAC-SHA256 signing that follows the official
    Tencent Cloud API v3 spec (signed headers include content-type, host,
    x-tc-action), which differs from the LLM-specific signing in
    WedataWorkspaceClient.
    """
    # Read credentials
    secret_id, secret_key, token = config.get_local_credentials()
    workspace_id = config.workspace_id
    region = config.region
    host = config.get("WEDATA_SDK_ENDPOINT", "wedata.tencentcloudapi.com")

    # Collect all items across pages
    all_items: List[Dict[str, Any]] = []
    page_number = 1
    total_count = None

    while True:
        body = build_request_body(
            workspace_id=workspace_id,
            page_number=page_number,
        )
        payload = json.dumps(body, ensure_ascii=False)

        # Build signed headers (TC3-HMAC-SHA256)
        headers = build_tc3_headers(
            secret_id=secret_id,
            secret_key=secret_key,
            host=host,
            action=_API_ACTION,
            payload=payload,
            region=region,
            token=token or "",
        )

        # Make the request via HTTPS
        try:
            conn = HTTPSConnection(host)
            conn.request("POST", "/", headers=headers, body=payload.encode("utf-8"))
            resp = conn.getresponse()
            resp_body = resp.read().decode("utf-8")
            conn.close()
        except Exception as e:
            raise RuntimeError(f"HTTP request failed: {e}")

        result = json.loads(resp_body)

        # Check for API errors
        response = result.get("Response", {})
        error = response.get("Error")
        if error:
            raise RuntimeError(
                f"API Error: [{error.get('Code', 'Unknown')}] "
                f"{error.get('Message', 'No message')}"
            )

        data = response.get("Data", {})
        items = data.get("Items", [])
        all_items.extend(items)

        # Check pagination
        page_response = data.get("PageResponse", {})
        if total_count is None:
            total_count = page_response.get("TotalCount", 0)
        total_pages = page_response.get("TotalPageNumber", 1)

        if page_number >= total_pages:
            break
        page_number += 1

    return {
        "items": all_items,
        "total_count": total_count or len(all_items),
    }


# ── Output Formatting ──

def format_markdown(items: List[Dict[str, Any]]) -> str:
    """Format MCP server list as a markdown report."""
    lines = ["# WeData MCP Servers Discovery\n"]

    if not items:
        lines.append("No MCP servers found in this workspace.\n")
        lines.append(
            "To configure MCP servers, go to the WeData console "
            "→ Application Management → MCP Configuration."
        )
        return "\n".join(lines)

    lines.append(f"Found **{len(items)}** MCP server(s):\n")

    # Summary table
    lines.append("| # | Server Name | Type | Status | Transport | Description |")
    lines.append("|---|------------|------|--------|-----------|-------------|")

    for i, item in enumerate(items, 1):
        name = item.get("ServerName", "—")
        stype = item.get("ServerType", "—")
        status = item.get("Status", "—")
        transport = item.get("TransportType", "—")
        desc = item.get("Description", "—")
        # Truncate long descriptions for the table
        if len(desc) > 40:
            desc = desc[:37] + "..."
        lines.append(
            f"| {i} | {name} | {stype} | {status} | {transport} | {desc} |"
        )

    lines.append("")

    # Detailed list with URLs
    lines.append("## Server Details\n")
    for i, item in enumerate(items, 1):
        name = item.get("ServerName", "Unknown")
        lines.append(f"### {i}. {name}\n")
        lines.append(f"- **URL**: `{item.get('ServerUrl', '—')}`")
        lines.append(f"- **Type**: {item.get('ServerType', '—')}")
        lines.append(f"- **Status**: {item.get('Status', '—')}")
        lines.append(f"- **Transport**: {item.get('TransportType', '—')}")
        lines.append(f"- **Description**: {item.get('Description', '—')}")
        lines.append(f"- **Key**: `{item.get('Key', '—')}`")
        created_user = item.get("CreatedUser", "—")
        modified_user = item.get("ModifiedUser", "—")
        lines.append(f"- **Created by**: {created_user}")
        lines.append(f"- **Modified by**: {modified_user}")
        lines.append("")

    # Usage hint
    lines.append("## How to Use\n")
    lines.append("Add an MCP server to your agent in `agent_server/agent.py`:\n")
    lines.append("```python")
    lines.append("from agents.mcp import MCPServerStreamableHttp")
    lines.append("")
    lines.append("mcp_server = MCPServerStreamableHttp(")
    if items:
        lines.append(f'    url="{items[0].get("ServerUrl", "<ServerUrl>")}",')
        lines.append(f'    name="{items[0].get("ServerName", "<ServerName>")}",')
    else:
        lines.append('    url="<ServerUrl from above>",')
        lines.append('    name="<ServerName>",')
    lines.append(")")
    lines.append("```")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Discover available MCP servers in a WeData workspace"
    )
    parser.add_argument(
        "--format",
        choices=["json", "markdown"],
        default="markdown",
        help="Output format (default: markdown)",
    )
    parser.add_argument(
        "--output",
        help="Output file path (default: stdout)",
    )

    args = parser.parse_args()

    # Initialize config (reads .env.local automatically)
    print("Loading configuration from .env.local...", file=sys.stderr)
    config = AppConfig(project_root=str(_PROJECT_ROOT))

    if not config.is_local:
        print(
            "Warning: .env.local not found. "
            "Make sure credentials are available via environment variables.",
            file=sys.stderr,
        )

    print(f"Workspace ID: {config.workspace_id}", file=sys.stderr)
    print(f"Endpoint: {config.get('WEDATA_SDK_ENDPOINT', 'wedata.tencentcloudapi.com')}", file=sys.stderr)
    print("Discovering MCP servers...\n", file=sys.stderr)

    try:
        result = call_list_mcp_servers(config)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

    items = result["items"]
    total = result["total_count"]

    # Format output
    if args.format == "json":
        output = json.dumps(result, indent=2, ensure_ascii=False)
    else:
        output = format_markdown(items)

    # Write output
    if args.output:
        Path(args.output).write_text(output, encoding="utf-8")
        print(f"Results written to {args.output}", file=sys.stderr)
    else:
        print(output)

    # Summary
    print(f"\n=== Discovery Summary ===", file=sys.stderr)
    print(f"Total MCP Servers: {total}", file=sys.stderr)
    if items:
        active = sum(1 for i in items if i.get("Status") == "active")
        print(f"Active: {active}", file=sys.stderr)
        types = set(i.get("ServerType", "unknown") for i in items)
        print(f"Types: {', '.join(types)}", file=sys.stderr)


if __name__ == "__main__":
    main()
