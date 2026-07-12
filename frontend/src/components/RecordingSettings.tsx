import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '@/components/ui/switch';
import { FolderOpen } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { DeviceSelection, SelectedDevices } from '@/components/DeviceSelection';
import Analytics from '@/lib/analytics';
import { toast } from 'sonner';
import { useConfig } from '@/contexts/ConfigContext';

export interface RecordingPreferences {
  save_folder: string;
  auto_save: boolean;
  file_format: string;
  preferred_mic_device: string | null;
  preferred_system_device: string | null;
  /**
   * How long a silence must last before the transcript breaks to a new line, in ms.
   *
   * This is what decides sentence granularity. It was fixed at 400ms — longer than the
   * pause between sentences in fluent or professional speech, so several sentences
   * merged onto one line.
   */
  vad_redemption_ms: number;
  /**
   * 'punctuation' — the model's own punctuation decides where a line ends (SenseVoice only).
   * 'pause'       — a line is one VAD segment, i.e. speech between two silences.
   */
  segmentation_mode: SegmentationMode;
}

export type SegmentationMode = 'punctuation' | 'pause';

export const VAD_REDEMPTION_MIN_MS = 100;
export const VAD_REDEMPTION_MAX_MS = 400;
export const VAD_REDEMPTION_DEFAULT_MS = 200;

interface RecordingSettingsProps {
  onSave?: (preferences: RecordingPreferences) => void;
}

export function RecordingSettings({ onSave }: RecordingSettingsProps) {
  const { t } = useTranslation();
  const { transcriptModelConfig } = useConfig();

  // Only SenseVoice returns the punctuation and token alignment this mode needs.
  const supportsPunctuation = transcriptModelConfig.provider === 'senseVoice';
  const [preferences, setPreferences] = useState<RecordingPreferences>({
    save_folder: '',
    auto_save: true,
    file_format: 'mp4',
    preferred_mic_device: null,
    preferred_system_device: null,
    vad_redemption_ms: VAD_REDEMPTION_DEFAULT_MS,
    segmentation_mode: 'punctuation'
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [showRecordingNotification, setShowRecordingNotification] = useState(true);

  // Load recording preferences on component mount
  useEffect(() => {
    const loadPreferences = async () => {
      try {
        const prefs = await invoke<RecordingPreferences>('get_recording_preferences');
        setPreferences(prefs);
      } catch (error) {
        console.error('Failed to load recording preferences:', error);
        // If loading fails, get default folder path
        try {
          const defaultPath = await invoke<string>('get_default_recordings_folder_path');
          setPreferences(prev => ({ ...prev, save_folder: defaultPath }));
        } catch (defaultError) {
          console.error('Failed to get default folder path:', defaultError);
        }
      } finally {
        setLoading(false);
      }
    };

    loadPreferences();
  }, []);

  // Load recording notification preference
  useEffect(() => {
    const loadNotificationPref = async () => {
      try {
        const { Store } = await import('@tauri-apps/plugin-store');
        const store = await Store.load('preferences.json');
        const show = await store.get<boolean>('show_recording_notification') ?? true;
        setShowRecordingNotification(show);
      } catch (error) {
        console.error('Failed to load notification preference:', error);
      }
    };
    loadNotificationPref();
  }, []);

  const handleAutoSaveToggle = async (enabled: boolean) => {
    const newPreferences = { ...preferences, auto_save: enabled };
    setPreferences(newPreferences);
    await savePreferences(newPreferences);

    // Track auto-save setting change
    await Analytics.track('auto_save_recording_toggled', {
      enabled: enabled.toString()
    });
  };

  // Dragging updates local state only — the value must track the thumb without a
  // round-trip to the backend on every tick.
  const handleVadRedemptionInput = (ms: number) => {
    const clamped = Math.min(VAD_REDEMPTION_MAX_MS, Math.max(VAD_REDEMPTION_MIN_MS, ms));
    setPreferences((prev) => ({ ...prev, vad_redemption_ms: clamped }));
  };

  // Persist once, when the drag ends. Silently: a slider does not warrant a toast.
  const handleVadRedemptionCommit = async () => {
    await savePreferences(preferences, { silent: true });
  };

  const handleSegmentationModeChange = async (mode: SegmentationMode) => {
    const newPreferences = { ...preferences, segmentation_mode: mode };
    setPreferences(newPreferences);
    await savePreferences(newPreferences, { silent: true });
  };

  const handleDeviceChange = async (devices: SelectedDevices) => {
    const newPreferences = {
      ...preferences,
      preferred_mic_device: devices.micDevice,
      preferred_system_device: devices.systemDevice
    };
    setPreferences(newPreferences);
    await savePreferences(newPreferences);

    // Track default device preference changes
    // Note: Individual device selection analytics are tracked in DeviceSelection component
    await Analytics.track('default_devices_changed', {
      has_preferred_microphone: (!!devices.micDevice).toString(),
      has_preferred_system_audio: (!!devices.systemDevice).toString()
    });
  };

  const handleOpenFolder = async () => {
    try {
      await invoke('open_recordings_folder');
    } catch (error) {
      console.error('Failed to open recordings folder:', error);
    }
  };

  const handleNotificationToggle = async (enabled: boolean) => {
    try {
      setShowRecordingNotification(enabled);
      const { Store } = await import('@tauri-apps/plugin-store');
      const store = await Store.load('preferences.json');
      await store.set('show_recording_notification', enabled);
      await store.save();
      toast.success(t('settingsArea.recording.toast.preferenceSaved'));
      await Analytics.track('recording_notification_preference_changed', {
        enabled: enabled.toString()
      });
    } catch (error) {
      console.error('Failed to save notification preference:', error);
      toast.error(t('settingsArea.recording.toast.preferenceSaveFailed'));
    }
  };

  const savePreferences = async (prefs: RecordingPreferences, options?: { silent?: boolean }) => {
    // `silent` saves do not flip `saving`. Disabling a control mid-drag makes the
    // browser abort the drag and drop focus, which scrolled the settings page back to
    // the top the moment you touched the slider.
    if (!options?.silent) setSaving(true);
    try {
      await invoke('set_recording_preferences', { preferences: prefs });
      onSave?.(prefs);

      if (options?.silent) return;

      // Show success toast with device details
      const micDevice = prefs.preferred_mic_device || t('settingsArea.common.defaultDevice');
      const systemDevice = prefs.preferred_system_device || t('settingsArea.common.defaultDevice');
      toast.success(t('settingsArea.recording.toast.devicePreferencesSaved'), {
        description: t('settingsArea.recording.toast.devicePreferencesDescription', {
          microphone: micDevice,
          systemAudio: systemDevice
        })
      });
    } catch (error) {
      console.error('Failed to save recording preferences:', error);
      toast.error(t('settingsArea.recording.toast.devicePreferencesFailed'), {
        description: error instanceof Error ? error.message : String(error)
      });
    } finally {
      if (!options?.silent) setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="animate-pulse">
        <div className="h-4 bg-gray-200 rounded w-1/4 mb-4"></div>
        <div className="h-8 bg-gray-200 rounded mb-4"></div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-semibold mb-4">{t('settingsArea.recording.title')}</h3>
        <p className="text-sm text-gray-600 mb-6">
          {t('settingsArea.recording.description')}
        </p>
      </div>

      {/* Auto Save Toggle */}
      <div className="flex items-center justify-between p-4 border rounded-lg">
        <div className="flex-1">
          <div className="font-medium">{t('settingsArea.recording.autoSave.title')}</div>
          <div className="text-sm text-gray-600">
            {t('settingsArea.recording.autoSave.description')}
          </div>
        </div>
        <Switch
          checked={preferences.auto_save}
          onCheckedChange={handleAutoSaveToggle}
          disabled={saving}
        />
      </div>

      {/* Folder Location - Only shown when auto_save is enabled */}
      {preferences.auto_save && (
        <div className="space-y-4">
          <div className="p-4 border rounded-lg bg-gray-50">
            <div className="font-medium mb-2">{t('settingsArea.recording.saveLocation')}</div>
            <div className="text-sm text-gray-600 mb-3 break-all">
              {preferences.save_folder || t('settingsArea.recording.defaultFolder')}
            </div>
            <button
              onClick={handleOpenFolder}
              className="flex items-center gap-2 px-3 py-2 text-sm border border-gray-300 rounded-md hover:bg-gray-50 transition-colors"
            >
              <FolderOpen className="w-4 h-4" />
              {t('settingsArea.common.openFolder')}
            </button>
          </div>

          <div className="p-4 border rounded-lg bg-blue-50">
            <div className="text-sm text-blue-800">
              <strong>{t('settingsArea.recording.fileFormatLabel')}</strong>{' '}
              {t('settingsArea.recording.fileFormatValue', { format: preferences.file_format.toUpperCase() })}
            </div>
            <div className="text-xs text-blue-600 mt-1">
              {t('settingsArea.recording.timestampHint', { extension: preferences.file_format })}
            </div>
          </div>
        </div>
      )}

      {/* Info when auto_save is disabled */}
      {!preferences.auto_save && (
        <div className="p-4 border rounded-lg bg-yellow-50">
          <div className="text-sm text-yellow-800">
            {t('settingsArea.recording.disabledNotice')}
          </div>
        </div>
      )}

      {/* Recording Notification Toggle */}
      <div className="flex items-center justify-between p-4 border rounded-lg">
        <div className="flex-1">
          <div className="font-medium">{t('settingsArea.recording.startNotification.title')}</div>
          <div className="text-sm text-gray-600">
            {t('settingsArea.recording.startNotification.description')}
          </div>
        </div>
        <Switch
          checked={showRecordingNotification}
          onCheckedChange={handleNotificationToggle}
        />
      </div>

      {/* Device Preferences */}
      <div className="space-y-4">
        <div className="border-t pt-6">
          <h4 className="text-base font-medium text-gray-900 mb-4">{t('settingsArea.recording.devices.title')}</h4>
          <p className="text-sm text-gray-600 mb-4">
            {t('settingsArea.recording.devices.description')}
          </p>

          <div className="border rounded-lg p-4 bg-gray-50">
            <DeviceSelection
              selectedDevices={{
                micDevice: preferences.preferred_mic_device,
                systemDevice: preferences.preferred_system_device
              }}
              onDeviceChange={handleDeviceChange}
              disabled={saving}
            />
          </div>
        </div>
      </div>

      {/* How the transcript is broken into lines */}
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <h3 className="text-lg font-semibold text-gray-900 mb-2">
          {t('settingsArea.recording.segmentation.title')}
        </h3>
        <p className="text-sm text-gray-600 mb-4">
          {t('settingsArea.recording.segmentation.description')}
        </p>

        <div className="space-y-3">
          {/* Mode 2 — the model's punctuation. Only SenseVoice can do this. */}
          <label
            className={`flex cursor-pointer items-start gap-3 rounded-lg border p-3 ${
              preferences.segmentation_mode === 'punctuation'
                ? 'border-blue-500 bg-blue-50'
                : 'border-gray-200 hover:border-gray-300'
            } ${!supportsPunctuation ? 'cursor-not-allowed opacity-50' : ''}`}
          >
            <input
              type="radio"
              name="segmentation_mode"
              className="mt-1"
              checked={preferences.segmentation_mode === 'punctuation'}
              disabled={!supportsPunctuation}
              onChange={() => handleSegmentationModeChange('punctuation')}
            />
            <div>
              <div className="text-sm font-medium text-gray-900">
                {t('settingsArea.recording.segmentation.punctuation.label')}
              </div>
              <div className="mt-0.5 text-xs text-gray-500">
                {supportsPunctuation
                  ? t('settingsArea.recording.segmentation.punctuation.help')
                  : t('settingsArea.recording.segmentation.punctuation.unavailable')}
              </div>
            </div>
          </label>

          {/* Mode 1 — VAD pauses. Every engine can do this. */}
          <label
            className={`flex cursor-pointer items-start gap-3 rounded-lg border p-3 ${
              preferences.segmentation_mode === 'pause'
                ? 'border-blue-500 bg-blue-50'
                : 'border-gray-200 hover:border-gray-300'
            }`}
          >
            <input
              type="radio"
              name="segmentation_mode"
              className="mt-1"
              checked={preferences.segmentation_mode === 'pause'}
              onChange={() => handleSegmentationModeChange('pause')}
            />
            <div className="flex-1">
              <div className="text-sm font-medium text-gray-900">
                {t('settingsArea.recording.segmentation.pause.label')}
              </div>
              <div className="mt-0.5 text-xs text-gray-500">
                {t('settingsArea.recording.segmentation.pause.help')}
              </div>

              {/* The pause length only means anything in this mode. */}
              {preferences.segmentation_mode === 'pause' && (
                <div className="mt-3">
                  <div className="flex items-center gap-4">
                    <input
                      type="range"
                      min={VAD_REDEMPTION_MIN_MS}
                      max={VAD_REDEMPTION_MAX_MS}
                      step={10}
                      value={preferences.vad_redemption_ms}
                      onChange={(e) => handleVadRedemptionInput(Number(e.target.value))}
                      onPointerUp={handleVadRedemptionCommit}
                      onKeyUp={handleVadRedemptionCommit}
                      onClick={(e) => e.preventDefault()}
                      className="h-2 flex-1 cursor-pointer appearance-none rounded-lg bg-gray-200 accent-blue-600"
                    />
                    <span className="w-20 shrink-0 text-right text-sm font-medium tabular-nums text-gray-900">
                      {preferences.vad_redemption_ms} ms
                    </span>
                  </div>
                  <div className="mt-1 flex justify-between text-xs text-gray-400">
                    <span>{t('settingsArea.recording.segmentation.shorter')}</span>
                    <span>{t('settingsArea.recording.segmentation.longer')}</span>
                  </div>
                </div>
              )}
            </div>
          </label>
        </div>
      </div>
    </div>
  );
}