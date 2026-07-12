import i18n from '@/i18n';

// Types for SenseVoice (sherpa-onnx) integration
export interface SenseVoiceModelInfo {
  name: string;
  path: string;
  size_mb: number;
  speed: ProcessingSpeed;
  status: ModelStatus;
  description?: string;
  languages: string[];
}

export type ProcessingSpeed = 'Slow' | 'Medium' | 'Fast' | 'Very Fast' | 'Ultra Fast';

export type ModelStatus =
  | 'Available'
  | 'Missing'
  | { Downloading: { progress: number } }
  | { Error: string }
  | { Corrupted: { file_size: number; expected_min_size: number } };

export interface SenseVoiceEngineState {
  currentModel: string | null;
  availableModels: SenseVoiceModelInfo[];
  isLoading: boolean;
  error: string | null;
}

// User-friendly model display configuration
export interface ModelDisplayInfo {
  friendlyName: string;
  tagline: string;
  recommended?: boolean;
  tier: 'fastest' | 'balanced' | 'precise';
}

export const MODEL_DISPLAY_CONFIG: Record<string, ModelDisplayInfo> = {
  'sense-voice-small-int8': {
    friendlyName: 'SenseVoice Small',
    tagline: '60x real-time • Chinese-optimised, multilingual',
    recommended: true,
    tier: 'fastest'
  }
};

// Model configuration for SenseVoice models (matching Rust implementation)
// Source: https://github.com/k2-fsa/sherpa-onnx/releases (asr-models)
export const SENSEVOICE_MODEL_CONFIGS: Record<string, Partial<SenseVoiceModelInfo>> = {
  'sense-voice-small-int8': {
    description:
      'Chinese-optimised. Also English, Japanese, Korean and Cantonese. ~60x realtime on CPU, with punctuation.',
    size_mb: 236,
    speed: 'Very Fast',
    languages: ['zh', 'en', 'ja', 'ko', 'yue']
  }
};

// Display labels for the languages SenseVoice supports
export const LANGUAGE_LABELS: Record<string, string> = {
  zh: '中文',
  en: 'EN',
  ja: '日本語',
  ko: '한국어',
  yue: '粵語'
};

// Helper functions

// Get user-friendly display name for a model
export function getModelDisplayName(modelName: string): string {
  const displayInfo = MODEL_DISPLAY_CONFIG[modelName];
  return displayInfo?.friendlyName || modelName;
}

// Model ids contain dots, which i18next would treat as key separators, so the
// tagline keys are mapped explicitly.
const MODEL_TAGLINE_KEYS: Record<string, string> = {
  'sense-voice-small-int8': 'modelsArea.sensevoice.tagline'
};

// Get model display info (icon, tagline, etc.)
export function getModelDisplayInfo(modelName: string): ModelDisplayInfo | null {
  const displayInfo = MODEL_DISPLAY_CONFIG[modelName];
  if (!displayInfo) return null;

  const taglineKey = MODEL_TAGLINE_KEYS[modelName];
  return taglineKey
    ? { ...displayInfo, tagline: i18n.t(taglineKey, { defaultValue: displayInfo.tagline }) }
    : displayInfo;
}

// Get the badge label for a language code (falls back to the raw code)
export function getLanguageLabel(language: string): string {
  return LANGUAGE_LABELS[language] || language.toUpperCase();
}

export function getStatusColor(status: ModelStatus): string {
  if (status === 'Available') return 'green';
  if (status === 'Missing') return 'gray';
  if (typeof status === 'object' && 'Downloading' in status) return 'blue';
  if (typeof status === 'object' && 'Error' in status) return 'red';
  return 'gray';
}

export function formatFileSize(sizeMb: number): string {
  if (sizeMb >= 1000) {
    return `${(sizeMb / 1000).toFixed(1)}GB`;
  }
  return `${sizeMb}MB`;
}

// Whether a model supports the given language ('all' matches every model)
export function modelSupportsLanguage(model: SenseVoiceModelInfo, language?: string): boolean {
  if (!language || language === 'all') return true;
  return (model.languages || []).includes(language);
}

export function getRecommendedModel(): string {
  return 'sense-voice-small-int8';
}

// Tauri command wrappers for SenseVoice backend
import { invoke } from '@tauri-apps/api/core';

export class SenseVoiceAPI {
  static async init(): Promise<void> {
    await invoke('sensevoice_init');
  }

  static async getAvailableModels(): Promise<SenseVoiceModelInfo[]> {
    return await invoke('sensevoice_get_available_models');
  }

  static async loadModel(modelName: string): Promise<void> {
    await invoke('sensevoice_load_model', { modelName });
  }

  static async getCurrentModel(): Promise<string | null> {
    return await invoke('sensevoice_get_current_model');
  }

  static async isModelLoaded(): Promise<boolean> {
    return await invoke('sensevoice_is_model_loaded');
  }

  static async hasAvailableModels(): Promise<boolean> {
    return await invoke('sensevoice_has_available_models');
  }

  static async validateModelReady(): Promise<string> {
    return await invoke('sensevoice_validate_model_ready');
  }

  static async getModelsDirectory(): Promise<string> {
    return await invoke('sensevoice_get_models_directory');
  }

  static async downloadModel(modelName: string): Promise<void> {
    await invoke('sensevoice_download_model', { modelName });
  }

  static async cancelDownload(modelName: string): Promise<void> {
    await invoke('sensevoice_cancel_download', { modelName });
  }

  static async deleteModel(modelName: string): Promise<string> {
    return await invoke('sensevoice_delete_model', { modelName });
  }

  static async openModelsFolder(): Promise<void> {
    await invoke('open_sensevoice_models_folder');
  }
}
