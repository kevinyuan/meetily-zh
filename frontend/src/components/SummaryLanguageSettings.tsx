'use client';

import { useState } from 'react';
import { Globe, Pin } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Popover, PopoverTrigger, PopoverContent } from '@/components/ui/popover';
import { LanguagePickerPopover } from '@/components/LanguagePickerPopover';
import { useRecentLanguages } from '@/hooks/useRecentLanguages';
import { labelForCode } from '@/lib/summary-languages';
import { FOLLOW_UI_LANGUAGE, LANGUAGE_METADATA } from '@/i18n/languages';

export function SummaryLanguageSettings() {
  const { t, i18n } = useTranslation();
  const { recents, pinned, addRecent, removeRecent, setPinned } = useRecentLanguages();
  const [pickerOpen, setPickerOpen] = useState(false);

  const togglePin = (code: string) => {
    setPinned(pinned === code ? null : code);
  };

  const followsUi = pinned === FOLLOW_UI_LANGUAGE;
  const uiLanguageName = LANGUAGE_METADATA[i18n.language]?.nativeName ?? i18n.language;

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm relative">
      <div className="flex items-center gap-2 mb-2">
        <Globe size={18} className="text-gray-500" />
        <h3 className="text-lg font-semibold text-gray-900">{t('settingsArea.summaryLanguage.title')}</h3>
      </div>
      <p className="text-sm text-gray-600 mb-4">
        {t('settingsArea.summaryLanguage.description')}
      </p>

      <div className="flex flex-wrap items-center gap-2">
        {/* Follows the display language instead of storing a copy of it, so changing
            the app language changes this too. */}
        <button
          type="button"
          aria-pressed={followsUi}
          onClick={() => setPinned(followsUi ? null : FOLLOW_UI_LANGUAGE)}
          className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-sm hover:brightness-95 active:brightness-90 ${
            followsUi
              ? 'bg-blue-50 border-blue-200 text-blue-800'
              : 'bg-gray-100 border-gray-200 text-gray-800'
          }`}
        >
          {followsUi && <Pin size={13} className="fill-current" />}
          {t('languagePreference.followUi', { language: uiLanguageName })}
        </button>

        {recents.map((code) => {
          const isPinned = pinned === code;
          return (
            <span
              key={code}
              className={`inline-flex items-center rounded-full border text-sm overflow-hidden ${
                isPinned
                  ? 'bg-blue-50 border-blue-200 text-blue-800'
                  : 'bg-gray-100 border-gray-200 text-gray-800'
              }`}
            >
              <button
                type="button"
                aria-label={isPinned
                  ? t('settingsArea.summaryLanguage.unpinAsDefault', { language: labelForCode(code) })
                  : t('settingsArea.summaryLanguage.pinAsDefault', { language: labelForCode(code) })}
                aria-pressed={isPinned}
                title={isPinned
                  ? t('settingsArea.summaryLanguage.clickToUnsetDefault')
                  : t('settingsArea.summaryLanguage.clickToSetDefault')}
                onClick={() => togglePin(code)}
                className={`flex items-center gap-1.5 pl-3 pr-2 py-1 hover:brightness-95 active:brightness-90 ${
                  isPinned ? 'text-blue-800' : 'text-gray-800'
                }`}
              >
                <Pin
                  size={14}
                  className={isPinned ? 'text-blue-600' : 'text-gray-400'}
                  fill={isPinned ? 'currentColor' : 'none'}
                />
                {labelForCode(code)}
              </button>
              <button
                type="button"
                aria-label={t('settingsArea.summaryLanguage.remove', { language: labelForCode(code) })}
                onClick={() => removeRecent(code)}
                className={`pr-2.5 pl-0.5 py-1 leading-none ${isPinned ? 'text-blue-400 hover:text-blue-700' : 'text-gray-400 hover:text-gray-700'}`}
              >
                ×
              </button>
            </span>
          );
        })}

        <Popover open={pickerOpen} onOpenChange={setPickerOpen}>
          <PopoverTrigger asChild>
            <button
              type="button"
              disabled={recents.length >= 5}
              className="inline-flex items-center gap-1 rounded-full border border-dashed border-gray-300 px-3 py-1 text-sm text-gray-600 hover:border-gray-400 hover:text-gray-800 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {t('settingsArea.summaryLanguage.addLanguage')}
            </button>
          </PopoverTrigger>
          <PopoverContent align="start" className="w-auto p-0 border-0 shadow-none bg-transparent">
            <LanguagePickerPopover
              mode="settings"
              value={null}
              onChange={(code) => {
                if (code) addRecent(code);
                setPickerOpen(false);
              }}
              onClose={() => setPickerOpen(false)}
            />
          </PopoverContent>
        </Popover>
      </div>

      <p className="text-xs text-gray-400 mt-3">
        {pinned
          ? t('settingsArea.summaryLanguage.defaultHint', { language: labelForCode(pinned) })
          : t('settingsArea.summaryLanguage.noDefaultHint')}
      </p>
    </div>
  );
}
