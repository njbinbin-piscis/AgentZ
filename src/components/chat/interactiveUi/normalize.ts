import type { UiBlock, UiDefinition, UiOption, UiWizardStep } from "./protocol";

/**
 * Agent-written forms do not always follow the option shape
 * `{ value: string, label: string }`: values arrive as numbers, under another
 * key (`id`, `key`, `name`), or are left out so only a label exists; options
 * can be bare strings; two options can share a value. The fields compare the
 * selected value against `option.value` as a string, so any of those shapes
 * rendered an option that could be seen but never selected.
 *
 * Every definition is passed through here before it is rendered, initialised,
 * validated or submitted. The Rust side (`chat_ui_schema::normalize_ui_definition`)
 * applies the same rules before it emits the card; keep the two in step.
 */

const SINGLE_CHOICE_TYPES = new Set(["radio", "select", "link_list", "project_picker"]);
const MULTI_CHOICE_TYPES = new Set(["checkbox", "tags", "koi_picker"]);

const VALUE_KEYS = ["value", "id", "key", "name"];
const LABEL_KEYS = ["label", "text", "title", "name"];

function scalarText(value: unknown): string | null {
  if (typeof value === "string") return value.trim() ? value : null;
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : null;
  if (typeof value === "boolean") return String(value);
  return null;
}

function firstText(source: Record<string, unknown>, keys: string[]): string | null {
  for (const key of keys) {
    const text = scalarText(source[key]);
    if (text !== null) return text;
  }
  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

/** Options as a list, whatever container the agent used. */
function optionEntries(raw: unknown): unknown[] {
  if (Array.isArray(raw)) return raw;
  // `{ "a": "Option A", "b": "Option B" }`
  if (isRecord(raw)) return Object.entries(raw).map(([value, label]) => ({ value, label }));
  return [];
}

export function normalizeOptions(raw: unknown): UiOption[] {
  const out: UiOption[] = [];
  const used = new Set<string>();
  optionEntries(raw).forEach((entry, index) => {
    let value: string | null;
    let label: string | null;
    let rest: Record<string, unknown> = {};
    if (isRecord(entry)) {
      rest = entry;
      value = firstText(entry, VALUE_KEYS);
      label = firstText(entry, LABEL_KEYS) ?? scalarText(entry.description);
    } else {
      value = scalarText(entry);
      label = value;
    }
    // Nothing to show and nothing to submit.
    if (value === null && label === null) return;
    let resolved = value ?? label ?? `opt-${index + 1}`;
    if (used.has(resolved)) {
      let n = 2;
      while (used.has(`${resolved}-${n}`)) n += 1;
      resolved = `${resolved}-${n}`;
    }
    used.add(resolved);
    out.push({ ...(rest as Partial<UiOption>), value: resolved, label: label ?? resolved });
  });
  return out;
}

/** A default/data entry written as a label or a number, mapped onto an option value. */
function resolveChoice(raw: unknown, options: UiOption[]): unknown {
  const text = scalarText(raw);
  if (text === null) return raw;
  if (options.some((opt) => opt.value === text)) return text;
  const byLabel = options.find((opt) => opt.label === text);
  return byLabel ? byLabel.value : text;
}

function resolveChoiceList(raw: unknown, options: UiOption[]): unknown {
  if (raw === undefined || raw === null) return raw;
  const list = Array.isArray(raw) ? raw : [raw];
  return list
    .map((item) => resolveChoice(item, options))
    .filter((item): item is string => typeof item === "string");
}

function normalizeBlock(block: UiBlock, fields: Map<string, UiBlock>): UiBlock {
  if (!isRecord(block)) return block;
  const next: UiBlock = { ...block };
  if (Array.isArray(block.blocks)) {
    next.blocks = block.blocks.map((child) => normalizeBlock(child, fields));
  }
  if (block.options !== undefined) {
    next.options = normalizeOptions(block.options);
  }
  const options = next.options ?? [];
  if (SINGLE_CHOICE_TYPES.has(block.type)) {
    if (block.default !== undefined) next.default = resolveChoice(block.default, options);
  } else if (MULTI_CHOICE_TYPES.has(block.type)) {
    if (block.default !== undefined) next.default = resolveChoiceList(block.default, options);
  }
  if (typeof next.id === "string" && next.id) fields.set(next.id, next);
  return next;
}

function normalizeBlocks(blocks: unknown, fields: Map<string, UiBlock>): UiBlock[] {
  return Array.isArray(blocks) ? blocks.map((block) => normalizeBlock(block as UiBlock, fields)) : [];
}

export function normalizeUiDefinition(def: UiDefinition): UiDefinition {
  if (!isRecord(def)) return { blocks: [] };
  const fields = new Map<string, UiBlock>();
  const next: UiDefinition = { ...def, blocks: normalizeBlocks(def.blocks, fields) };
  if (Array.isArray(def.steps)) {
    next.steps = def.steps.map(
      (step): UiWizardStep =>
        isRecord(step) ? { ...step, blocks: normalizeBlocks(step.blocks, fields) } : { blocks: [] },
    );
  }
  if (isRecord(def.data)) {
    const data: Record<string, unknown> = { ...def.data };
    for (const [id, block] of fields) {
      if (data[id] === undefined) continue;
      const options = block.options ?? [];
      if (SINGLE_CHOICE_TYPES.has(block.type)) data[id] = resolveChoice(data[id], options);
      else if (MULTI_CHOICE_TYPES.has(block.type)) data[id] = resolveChoiceList(data[id], options);
    }
    next.data = data;
  }
  return next;
}
