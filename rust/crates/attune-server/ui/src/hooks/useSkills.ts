/** useSkills · Skills 列表 + 启用/禁用 持久化 */
import { api } from '../store/api';

export type SkillSummary = {
  id: string;
  name: string;
  description: string;
  version: string;
  keywords: string[];
  patterns: string[];
  enabled_in_plugin: boolean;
  disabled_by_user: boolean;
};

type ListResponse = { skills: SkillSummary[] };

export async function listSkills(): Promise<SkillSummary[]> {
  try {
    const res = await api.get<ListResponse>('/skills');
    return res.skills ?? [];
  } catch {
    return [];
  }
}

/** 切换 skill 启用/禁用 — 通过 PATCH /settings.skills.disabled 持久化 */
export async function setSkillDisabled(id: string, disabled: boolean): Promise<boolean> {
  try {
    const settings = await api.get<{ skills?: { disabled?: string[] } }>('/settings');
    const cur = settings.skills?.disabled ?? [];
    const next = disabled
      ? Array.from(new Set([...cur, id]))
      : cur.filter((s) => s !== id);
    await api.patch('/settings', { skills: { disabled: next } });
    return true;
  } catch {
    return false;
  }
}
