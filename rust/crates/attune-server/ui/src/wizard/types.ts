/** Wizard 类型 */

export type WizardStep = 1 | 2 | 3 | 4 | 5;

export type WizardContext = {
  step: WizardStep;
  /** 已完成的 steps（允许 stepper 回跳） */
  completedSteps: Set<WizardStep>;
  /** Step 3 选择的 LLM 后端类型 */
  llmMode: 'ollama' | 'cloud' | 'skip' | null;
  /** 硬件推荐的默认模型（Step 4 应用后） */
  chatModel: string | null;
  embeddingModel: string | null;
  summaryModel: string | null;
  /** Step 5 选择的数据源类型 */
  dataMode: 'folder' | 'import' | 'skip' | null;
  /** 所选文件夹路径 */
  boundFolder: string | null;
  /** 导入的 profile 文件名 */
  importedProfile: string | null;
};

export const initialWizardContext: WizardContext = {
  step: 1,
  completedSteps: new Set(),
  llmMode: null,
  chatModel: null,
  embeddingModel: null,
  summaryModel: null,
  dataMode: null,
  boundFolder: null,
  importedProfile: null,
};
