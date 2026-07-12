'use client';

import { useTranslation } from 'react-i18next';
import { Globe } from 'lucide-react';

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { setUiLanguage } from '@/i18n';
import { LANGUAGE_METADATA, SUPPORTED_UI_LANGUAGES } from '@/i18n/languages';

/**
 * Picks the language of the app interface itself.
 *
 * This is deliberately distinct from the transcription language (what the speech
 * engine listens for) and the summary language (what the LLM writes). Those two can
 * be set to follow whatever is chosen here.
 */
export function AppLanguageSelector() {
  const { t, i18n } = useTranslation();

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6">
      <div className="flex items-start justify-between gap-6">
        <div>
          <h3 className="flex items-center gap-2 text-lg font-medium text-gray-900">
            <Globe className="h-5 w-5 text-gray-500" />
            {t('appLanguage.title')}
          </h3>
          <p className="mt-1 text-sm text-gray-500">{t('appLanguage.description')}</p>
        </div>

        <Select value={i18n.language} onValueChange={(value) => void setUiLanguage(value)}>
          <SelectTrigger className="w-56 shrink-0">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SUPPORTED_UI_LANGUAGES.map((code) => {
              const meta = LANGUAGE_METADATA[code];
              return (
                <SelectItem key={code} value={code}>
                  {meta.nativeName === meta.name
                    ? meta.nativeName
                    : `${meta.nativeName} (${meta.name})`}
                </SelectItem>
              );
            })}
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}

export default AppLanguageSelector;
