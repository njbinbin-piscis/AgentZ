const MAX_WEBVIEW_MESSAGE_CHARS = 1_048_576;

function attributes(text: string): { name: string; value: string; start: number; end: number }[] {
  const result = [];
  const pattern = /\s*([^\s=/>]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/y;
  let offset = 0;
  while (offset < text.length) {
    pattern.lastIndex = offset;
    const match = pattern.exec(text);
    if (!match) break;
    result.push({ name: match[1].toLowerCase(), value: match[2] ?? match[3] ?? match[4] ?? "", start: offset, end: pattern.lastIndex });
    offset = pattern.lastIndex;
  }
  return result;
}

export function isBoundedWebviewMessage(value: unknown): boolean {
  try {
    const encoded = JSON.stringify(value);
    return encoded !== undefined && encoded.length <= MAX_WEBVIEW_MESSAGE_CHARS;
  } catch {
    return false;
  }
}

function setInlineNonces(
  html: string,
  nonce: string | undefined,
  scriptsEnabled: boolean,
  stylesEnabled: boolean,
  resourceToken: string | undefined,
): string {
  let output = "";
  let offset = 0;
  while (offset < html.length) {
    const start = html.indexOf("<", offset);
    if (start < 0) return output + html.slice(offset);
    output += html.slice(offset, start);
    if (html.startsWith("<!--", start)) {
      const endComment = html.indexOf("-->", start + 4);
      const end = endComment < 0 ? html.length : endComment + 3;
      output += html.slice(start, end);
      offset = end;
      continue;
    }

    const opening = html.slice(start).match(/^<(script|style)\b/i);
    if (!opening) {
      output += "<";
      offset = start + 1;
      continue;
    }
    const tag = opening[1].toLowerCase();
    let end = start + opening[0].length;
    let quote = "";
    while (end < html.length) {
      const char = html[end++];
      if (quote) {
        if (char === quote) quote = "";
      } else if (char === "\"" || char === "'") {
        quote = char;
      } else if (char === ">") {
        break;
      }
    }
    let element = html.slice(start, end);
    const attrs = element.slice(opening[0].length, -1);
    const closing = /^\s*\//.test(attrs);
    const parsedAttrs = attributes(attrs);
    const srcMatch = tag === "script" ? parsedAttrs.find(attr => attr.name === "src") : undefined;
    const externalScript = Boolean(srcMatch);
    const scriptSrc = srcMatch?.value ?? "";
    const scopedExternalScript = Boolean(resourceToken)
      && scriptSrc.startsWith(`agentz-resource://localhost/${resourceToken}/`)
      && /^[A-Za-z0-9_-]+$/.test(scriptSrc.slice(`agentz-resource://localhost/${resourceToken}/`.length));
    const shouldNonce = !closing && ((tag === "style" && stylesEnabled)
      || (tag === "script" && scriptsEnabled && (!externalScript || scopedExternalScript)));
    if (shouldNonce || externalScript) {
      let cleanAttrs = attrs;
      for (const attr of parsedAttrs.slice().reverse()) {
        if (attr.name === "nonce") cleanAttrs = cleanAttrs.slice(0, attr.start) + cleanAttrs.slice(attr.end);
      }
      element = element.slice(0, opening[0].length) + cleanAttrs + ">";
    }
    if (shouldNonce) {
      element = element.slice(0, -1) + ` nonce="${nonce}">`;
    }
    output += element;
    offset = end;

    // Script/style contents are raw text. Do not treat tag-shaped source text
    // as HTML elements while adding the host nonce.
    if ((tag === "script" || tag === "style") && !closing) {
      const close = html.toLowerCase().indexOf(`</${tag}`, offset);
      if (close < 0) return output + html.slice(offset);
      output += html.slice(offset, close);
      offset = close;
    }
  }
  return output;
}

export function buildWebviewSrcDoc(html: string, enableScripts: boolean, nonce?: string, resourceToken?: string): string {
  const validNonce = nonce && /^[A-Za-z0-9+/_=-]{16,128}$/.test(nonce) ? nonce : undefined;
  const scriptsAllowed = enableScripts && Boolean(validNonce);

  const csp = [
    "default-src 'none'",
    `script-src ${scriptsAllowed ? `'nonce-${validNonce}'` : "'none'"}`,
    `style-src ${validNonce ? `'nonce-${validNonce}'` : "'none'"} agentz-resource: http://agentz-resource.localhost`,
    "img-src data: blob: agentz-resource: http://agentz-resource.localhost",
    "font-src data: agentz-resource: http://agentz-resource.localhost",
    "media-src agentz-resource: http://agentz-resource.localhost",
    "connect-src 'none'",
    "frame-src 'none'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
  ].join("; ");
  const policy = `<meta http-equiv="Content-Security-Policy" content="${csp}">`;
  const stateShim = scriptsAllowed
    ? `<script nonce="${validNonce}">(function(){let state=null;let acquired=false;const api=Object.freeze({postMessage:(message)=>parent.postMessage({__agentzWebview:true,message},'*'),getState:()=>state,setState:(value)=>{try{const copy=structuredClone(value);if(JSON.stringify(copy).length>1048576)return undefined;state=copy;return state}catch{return undefined}}});Object.defineProperty(window,'acquireVsCodeApi',{value:()=>{if(acquired)throw new Error('acquireVsCodeApi may only be called once');acquired=true;return api},writable:false,configurable:false});})();</script>`
    : "";
  const validResourceToken = resourceToken && /^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/i.test(resourceToken)
    ? resourceToken
    : undefined;
  const withPanelNonce = setInlineNonces(html, validNonce, scriptsAllowed, Boolean(validNonce), validResourceToken);
  const injection = `${policy}${stateShim}`;
  // Policy must precede every extension byte, including comments, raw text,
  // malformed document tags and scripts placed before an extension's head.
  const doctype = withPanelNonce.match(/^\s*<!doctype\b[^>]*>/i)?.[0] ?? "";
  const body = doctype ? withPanelNonce.slice(doctype.length) : withPanelNonce;
  return `${doctype}<html><head>${injection}</head><body>${body}</body></html>`;
}

