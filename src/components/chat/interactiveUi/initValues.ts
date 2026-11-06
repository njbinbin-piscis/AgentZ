import type { UiBlock, UiDefinition } from "./protocol";
import { CUSTOM_OPTION_VALUE } from "./protocol";
import { collectValueBlocks, flattenBlocks } from "./flatten";
import { mergeDataModel } from "./patch";

function defaultForBlock(block: UiBlock): unknown | undefined {
  if (block.default !== undefined) return block.default;
  switch (block.type) {
    case "checkbox":
    case "tags":
    case "koi_picker":
      return Array.isArray(block.suggestions) ? [...block.suggestions] : [];
    case "switch":
      return false;
    case "number_input":
    case "slider":
    case "progress":
      if (typeof block.min === "number") return block.min;
      if (block.type === "progress") return 0;
      return 0;
    case "link_list":
      return "";
    case "file_picker":
      return "";
    default:
      return undefined;
  }
}

/** Resolve initial form state from definition (not submitted snapshot). */
export function buildInitialValues(
  blocks: UiBlock[],
  data?: Record<string, unknown>,
): Record<string, unknown> {
  const values: Record<string, unknown> = data ? { ...data } : {};
  for (const block of flattenBlocks(blocks)) {
    if (!block.id) continue;
    if (values[block.id] !== undefined) continue;
    const d = defaultForBlock(block);
    if (d !== undefined) values[block.id] = d;
  }
  return coerceFieldValues(blocks, values);
}

export function buildInitialValuesFromDefinition(
  def: UiDefinition,
  wizardStep = 0,
): Record<string, unknown> {
  const blocks = collectValueBlocks(def, wizardStep);
  return buildInitialValues(blocks, def.data);
}

const LIST_FIELD_TYPES = new Set(["checkbox", "tags", "koi_picker"]);
const TEXT_FIELD_TYPES = new Set([
  "text_input",
  "select",
  "radio",
  "date",
  "time",
  "datetime",
  "link_list",
  "project_picker",
  "file_picker",
]);

function coerceFieldValues(blocks: UiBlock[], values: Record<string, unknown>): Record<string, unknown> {
  for (const block of flattenBlocks(blocks)) {
    if (!block.id || values[block.id] === undefined) continue;
    if (LIST_FIELD_TYPES.has(block.type)) values[block.id] = asStringList(values[block.id]);
    else if (TEXT_FIELD_TYPES.has(block.type) && typeof values[block.id] !== "string") values[block.id] = asString(values[block.id]);
  }
  return values;
}

/** Multi-select fields persist as string arrays. Older cards stored a boolean, number, or object. */
export function asStringList(value: unknown): string[] {
  if (Array.isArray(value)) return value.filter((item): item is string => typeof item === "string");
  if (typeof value === "string") return value ? [value] : [];
  return [];
}

export function asString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function asBlockList(value: unknown): UiBlock[] {
  if (!Array.isArray(value)) return [];
  return value.filter(
    (item): item is UiBlock => !!item && typeof item === "object" && typeof (item as UiBlock).type === "string",
  );
}

export function asOptionList(value: unknown): { value: string; label: string; description?: string; href?: string }[] {
  if (!Array.isArray(value)) return [];
  return value.filter(
    (item): item is { value: string; label: string; description?: string; href?: string } =>
      !!item && typeof item === "object" && typeof (item as { value?: unknown }).value === "string",
  );
}
export function normalizeSubmittedValues(
  def: UiDefinition,
  submitted: Record<string, unknown>,
): Record<string, unknown> {
  const out = { ...submitted };
  const allBlocks = def.mode === "wizard" && Array.isArray(def.steps) && def.steps.length
    ? def.steps.flatMap((s) => asBlockList(s?.blocks))
    : asBlockList(def.blocks);
  for (const block of flattenBlocks(allBlocks)) {
    if (!block.id || !block.allow_custom) continue;
    const v = out[block.id];
    if (v === CUSTOM_OPTION_VALUE) out[block.id] = "";
  }
  return coerceFieldValues(allBlocks, mergeDataModel(def, out));
}
