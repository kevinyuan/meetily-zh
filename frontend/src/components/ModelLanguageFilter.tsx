'use client';

import { useTranslation } from 'react-i18next';
import { Languages } from 'lucide-react';

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Label } from '@/components/ui/label';
import { FOLLOW_UI_LANGUAGE, LANGUAGE_METADATA } from '@/i18n/languages';
import { ALL_LANGUAGES, LanguageFilter } from '@/lib/model-languages';

/**
 * Filters the transcription model list down to models that support a given language.
 *
 * Distinct from the *transcription* language (what the engine listens for) and the
 * *summary* language (what the LLM writes in) — this only decides which models are
 * offered. Defaults to following the UI language, so a user running the app in
 * Chinese is shown Chinese-capable models without having to know which engine does
 * what.
 */
export function ModelLanguageFilter({
  value,
  onChange,
}: {
  value: LanguageFilter;
  onChange: (value: LanguageFilter) => void;
}) {
  const { t, i18n } = useTranslation();

  const uiLanguageName = LANGUAGE_METADATA[i18n.language]?.nativeName ?? i18n.language;

  return (
    <div className="flex items-center gap-3">
      <Label className="flex items-center gap-1.5 whitespace-nowrap text-sm font-medium text-gray-700">
        <Languages className="h-4 w-4 text-gray-500" />
        {t('modelFilter.label')}
      </Label>

      <Select value={value} onValueChange={onChange}>
        <SelectTrigger className="w-64 focus:border-blue-500 focus:ring-1 focus:ring-blue-500">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={FOLLOW_UI_LANGUAGE}>
            {t('languagePreference.followUi', { language: uiLanguageName })}
          </SelectItem>
          <SelectItem value={ALL_LANGUAGES}>{t('modelFilter.all')}</SelectItem>
          <SelectItem value="en">English</SelectItem>
          <SelectItem value="zh">中文</SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
}

export default ModelLanguageFilter;
