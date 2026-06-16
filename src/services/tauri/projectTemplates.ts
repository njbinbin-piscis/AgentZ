import { invoke } from "@tauri-apps/api/core";

export interface ProjectTemplateInfo {
  id: string;
  name: string;
  name_zh: string;
  description: string;
  description_zh: string;
  source_plugin: string;
}

export function listProjectTemplates(): Promise<ProjectTemplateInfo[]> {
  return invoke<ProjectTemplateInfo[]>("project_templates_list");
}

export function applyProjectTemplate(projectDir: string, templateId: string): Promise<void> {
  return invoke<void>("project_apply_template", { projectDir, templateId: templateId });
}

export function projectHasAgentz(projectDir: string): Promise<boolean> {
  return invoke<boolean>("project_has_agentz", { projectDir });
}
