---
name: "playwright"
description: "Automate browsers through a user-configured Playwright MCP server for scraping, testing and screenshots. Use when Playwright MCP tools are available."
description_zh: "通过用户配置的 Playwright MCP 服务器自动化浏览器：抓取、测试与截图"
version: "1.0.0"
source: agentz
---

# Playwright Browser Automation

Browser automation through a Playwright MCP server that the user has configured in **Settings → MCP Servers**. If no Playwright MCP tools are available, use AgentZ's builtin `browser` tool instead, or the `playwright-cli` skill.

## When to Use

- Web scraping and data extraction
- Automated browser testing
- Taking screenshots of web pages
- Form filling and user interaction simulation
- Monitoring and validation of web applications
- DOM element inspection

## Capabilities

### Navigation and page control
- Navigate to URLs
- Take screenshots (full page or specific elements)
- Execute JavaScript in the page context
- Get page content and HTML

### Element interaction
- Click elements, fill in forms and inputs
- Select dropdown options, hover, press keyboard keys

### Inspection and validation
- Get element attributes and properties, extract text
- Verify element existence and visibility, validate page state

### Advanced features
- Handle multiple tabs and windows
- Wait for specific conditions
- Capture network requests, manage cookies
- Handle file uploads and downloads

## Usage Examples

### Navigation and screenshot
Navigate to https://example.com and take a screenshot.

### Form automation
1. Navigate to https://example.com/login
2. Fill in the username field with "test@example.com"
3. Fill in the password field
4. Click the submit button
5. Take a screenshot of the result

### Data extraction
Scrape product prices from https://example.com/products.

### Testing
Test the checkout flow on the staging site.

## Setup

Add an MCP server in **Settings → MCP Servers** with command `npx` and arguments `@playwright/mcp@latest`. Requirements:

- Node.js 18+ (for `npx`)
- Network access on first use to download the server and a Chromium browser
- Enough disk space for Chromium

## Notes

- Browser runs headless by default unless the server is configured otherwise
- Works with SPA frameworks (React, Vue, Angular) and modern web standards

## Troubleshooting

If the MCP server fails to start:
1. Verify Node.js is installed: `node --version`
2. Check network connectivity
3. Try installing manually: `npm install -g @playwright/mcp`
4. Check system resources (RAM, disk space)
