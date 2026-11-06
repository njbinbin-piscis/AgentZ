import type { UiBlock, UiDefinition, UiWizardStep } from "./protocol";

/** Recursively collect leaf blocks from layout containers for validation/init. */
export function flattenBlocks(blocks: UiBlock[]): UiBlock[] {
  const list = Array.isArray(blocks) ? blocks : [];
  const out: UiBlock[] = [];
  for (const block of list) {
    if (!block || typeof block !== "object" || typeof block.type !== "string") continue;
    switch (block.type) {
      case "row":
      case "column":
      case "card":
      case "section":
        if (Array.isArray(block.blocks) && block.blocks.length) out.push(...flattenBlocks(block.blocks));
        break;
      default:
        out.push(block);
    }
  }
  return out;
}

/** All value-bearing blocks for the current wizard step (or full form). */
export function collectValueBlocks(def: UiDefinition, wizardStep: number): UiBlock[] {
  if (def.mode === "wizard" && Array.isArray(def.steps) && def.steps.length) {
    const step = def.steps[Math.min(wizardStep, def.steps.length - 1)];
    const stepBlocks = Array.isArray(step?.blocks) ? step.blocks : [];
    const footer = Array.isArray(def.blocks) ? def.blocks : [];
    return flattenBlocks([...stepBlocks, ...footer]);
  }
  return flattenBlocks(Array.isArray(def.blocks) ? def.blocks : []);
}

export function wizardStepCount(def: UiDefinition): number {
  return def.mode === "wizard" && Array.isArray(def.steps) && def.steps.length ? def.steps.length : 1;
}

export function wizardStepLabel(step: UiWizardStep, index: number): string {
  return step.label || step.id || `Step ${index + 1}`;
}
