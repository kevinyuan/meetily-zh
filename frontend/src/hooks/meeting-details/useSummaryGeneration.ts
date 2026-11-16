import { useState, useCallback, useRef, useEffect } from 'react';
import {
  CancelSummaryResponse,
  MeetingSummary,
  ProcessTranscriptResponse,
  SummaryProcessResponse,
  Transcript,
} from '@/types';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { invoke as invokeTauri } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import i18n from '@/i18n';
import Analytics from '@/lib/analytics';

import {
  readMeetingSummaryLanguage,
  resolvePinnedSummaryLanguageDefault,
} from '@/lib/summary-language-preferences';
import { uiLanguageToEngineCode } from '@/i18n/languages';
import { parseSummaryContent, readSummaryMetadata } from '@/lib/summary-content';

/**
 * The language a summary is written in.
 *
 * Follows the display language. Detecting it from the transcript was tried and got it
 * wrong often enough to be worse than useless — a meeting with any code-switching, or
 * a short one, would be misclassified and the summary would come back in a language
 * the user did not ask for. The display language is something the user actually chose,
 * so it is the honest default.
 *
 * An explicit choice still wins: per-meeting first, then the pinned default from
 * settings (which itself may be the "same as display language" sentinel).
 */
async function resolveSummaryLanguage(meetingId: string): Promise<string | null> {
  try {
    const perMeeting = await readMeetingSummaryLanguage(meetingId);
    if (perMeeting.language) return perMeeting.language;
  } catch (err) {
    // Non-actionable: we fall through to the pinned default / display language below,
    // so the generation still happens in a sensible language. Log it, don't toast it.
    console.warn('Failed to load meeting summary language:', err);
  }

  const pinned = resolvePinnedSummaryLanguageDefault();
  if (pinned) return pinned;

  return uiLanguageToEngineCode(i18n.language);
}

type SummaryStatus = 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error';

function restoredSummaryStatus(response?: SummaryProcessResponse | null): SummaryStatus {
  if (!response) return 'idle';
  if (response.status === 'pending' || response.status === 'processing') return 'processing';
  if (response.status === 'error' || response.status === 'failed') return 'error';
  if (parseSummaryContent(response.data)) return 'completed';
  if (response.status === 'completed') return 'error';
  return 'idle';
}

interface UseSummaryGenerationProps {
  initialSummary?: SummaryProcessResponse | null;
  meeting: {
    id: string;
    created_at: string;
  };
  transcripts: Transcript[];
  modelConfig: ModelConfig;
  isModelConfigLoading: boolean;
  selectedTemplate: string;
  onMeetingUpdated?: () => Promise<void>;
  updateMeetingTitle: (title: string) => void;
  setAiSummary: (summary: MeetingSummary | null) => void;
  onOpenModelSettings?: () => void;
}

export function useSummaryGeneration({
  initialSummary,
  meeting,
  transcripts,
  modelConfig,
  isModelConfigLoading,
  selectedTemplate,
  onMeetingUpdated,
  updateMeetingTitle,
  setAiSummary,
  onOpenModelSettings,
}: UseSummaryGenerationProps) {
  const restored = initialSummary?.meeting_id === meeting.id ? initialSummary : null;
  const [summaryStatus, setSummaryStatus] = useState<SummaryStatus>(() => restoredSummaryStatus(restored));
  const [summaryError, setSummaryError] = useState<string | null>(() =>
    restoredSummaryStatus(restored) === 'error'
      ? restored?.error || i18n.t('meetingArea.toasts.summaryFailedRetry')
      : null,
  );
  const mountedRef = useRef(true);
  const visibleMeetingIdRef = useRef(meeting.id);
  visibleMeetingIdRef.current = meeting.id;
  const generationIdRef = useRef(0);
  const activeProcessIdRef = useRef<string | null>(null);
  const trackedAttemptRef = useRef<{
    generationId: number;
    startedAt: number;
    provider: ModelConfig['provider'];
    model: ModelConfig['model'];
    finished: boolean;
  } | null>(null);
  const { startSummaryPolling, stopSummaryPolling } = useSidebar();

  const getSummaryStatusMessage = useCallback((status: SummaryStatus) => {
    switch (status) {
      case 'processing':
        return i18n.t('meetingArea.summary.status.processing');
      case 'summarizing':
        return i18n.t('meetingArea.summary.status.summarizing');
      case 'regenerating':
        return i18n.t('meetingArea.summary.status.regenerating');
      case 'completed':
        return i18n.t('meetingArea.summary.status.completed');
      case 'error':
        return i18n.t('meetingArea.summary.status.error');
      default:
        return '';
    }
  }, []);

  const finishGeneration = useCallback(async (
    generationId: number,
    outcome: 'ok' | 'fallback' | 'generation_error' | 'empty_result' | 'cancelled'
  ) => {
    const attempt = trackedAttemptRef.current;
    if (!attempt || attempt.generationId !== generationId || attempt.finished) {
      return;
    }
    attempt.finished = true;
    await Analytics.trackSummaryGenerationCompleted(
      attempt.provider,
      attempt.model,
      outcome === 'ok' || outcome === 'fallback',
      (Date.now() - attempt.startedAt) / 1000,
      outcome === 'ok' ? undefined : outcome,
    );
  }, []);

  const failGeneration = useCallback(async (
    generationId: number,
    isRegeneration: boolean,
    message: string,
    outcome: 'generation_error' | 'empty_result' = 'generation_error',
  ) => {
    if (!mountedRef.current || visibleMeetingIdRef.current !== meeting.id || generationId !== generationIdRef.current) {
      return;
    }
    activeProcessIdRef.current = null;
    setSummaryError(message);
    setSummaryStatus('error');
    toast.error(
      isRegeneration
        ? i18n.t('meetingArea.toasts.regenerateFailed')
        : i18n.t('meetingArea.toasts.generateFailed'),
      { description: message },
    );
    await finishGeneration(generationId, outcome);
  }, [finishGeneration, meeting.id]);

  const handlePollingResult = useCallback(async (
    pollingResult: SummaryProcessResponse,
    generationId: number,
    isRegeneration: boolean,
  ) => {
    if (!mountedRef.current || visibleMeetingIdRef.current !== meeting.id || generationId !== generationIdRef.current) {
      return;
    }
    if (pollingResult.status === 'cancelled') {
      let existing: SummaryProcessResponse;
      try {
        existing = await invokeTauri<SummaryProcessResponse>('api_get_summary', {
          meetingId: meeting.id,
        });
      } catch (error) {
        console.error('Failed to reload summary after cancellation:', error);
        await failGeneration(generationId, isRegeneration,
          i18n.t('meetingArea.toasts.cancelReloadFailed'));
        return;
      }
      if (!mountedRef.current || visibleMeetingIdRef.current !== meeting.id || generationId !== generationIdRef.current) {
        return;
      }
      const restoredSummary = parseSummaryContent(existing.data);
      setAiSummary(restoredSummary);
      setSummaryStatus(restoredSummary ? 'completed' : 'idle');
      setSummaryError(null);
      activeProcessIdRef.current = null;
      await finishGeneration(generationId, 'cancelled');
      return;
    }

    if (pollingResult.status === 'error' || pollingResult.status === 'failed') {
      const errorMessage = pollingResult.error
        || `Summary ${isRegeneration ? 'regeneration' : 'generation'} failed`;
      if (isRegeneration) {
        let existing: SummaryProcessResponse;
        try {
          existing = await invokeTauri<SummaryProcessResponse>('api_get_summary', {
            meetingId: meeting.id,
          });
        } catch (error) {
          console.error('Failed to reload previous summary after generation failure:', error);
          await failGeneration(generationId, isRegeneration,
            `${errorMessage}. The saved summary could not be reloaded. Please reopen this meeting.`);
          return;
        }
        if (!mountedRef.current || visibleMeetingIdRef.current !== meeting.id || generationId !== generationIdRef.current) {
          return;
        }
        const restoredSummary = parseSummaryContent(existing.data);
        if (restoredSummary) {
          setAiSummary(restoredSummary);
          setSummaryStatus('completed');
          setSummaryError(null);
          toast.error(i18n.t('meetingArea.toasts.regenerateFailed'), {
            description: i18n.t('meetingArea.toasts.regenerateFailedRestored', { error: errorMessage }),
          });
          activeProcessIdRef.current = null;
          await finishGeneration(generationId, 'generation_error');
          return;
        }
      }
      await failGeneration(generationId, isRegeneration, errorMessage);
      return;
    }

    if (pollingResult.status === 'completed') {
      const summary = parseSummaryContent(pollingResult.data);
      if (!summary) {
        await failGeneration(generationId, isRegeneration,
          i18n.t('meetingArea.toasts.completedNoContent'),
          'empty_result',
        );
        return;
      }
      const metadata = readSummaryMetadata(pollingResult.data);
      const meetingName = metadata.meetingName || pollingResult.meetingName;
      if (meetingName) {
        updateMeetingTitle(meetingName);
      }
      setAiSummary(summary);
      setSummaryStatus('completed');
      activeProcessIdRef.current = null;
      setSummaryError(null);
      if (metadata.normalizationFallback) {
        toast.warning(i18n.t('meetingArea.toasts.fallbackTitle'), {
          description: i18n.t('meetingArea.toasts.fallbackDescription'),
        });
      } else {
        toast.success(i18n.t('meetingArea.toasts.summarySuccess'), {
          description: metadata.reasoningStripped
            ? i18n.t('meetingArea.toasts.summarySuccessReasoningStripped')
            : i18n.t('meetingArea.toasts.summarySuccessDescription'),
          duration: 4000,
        });
      }
      if (meetingName && onMeetingUpdated) {
        await onMeetingUpdated();
      }
      await finishGeneration(
        generationId,
        metadata.normalizationFallback ? 'fallback' : 'ok',
      );
    }
  }, [failGeneration, finishGeneration, meeting.id, onMeetingUpdated, setAiSummary, updateMeetingTitle]);

  // Keep polling attached across ordinary rerenders without retaining stale view callbacks.
  const pollingResultRef = useRef(handlePollingResult);
  pollingResultRef.current = handlePollingResult;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (activeProcessIdRef.current) {
        stopSummaryPolling(meeting.id, activeProcessIdRef.current);
      }
    };
  }, [meeting.id, stopSummaryPolling]);

  useEffect(() => {
    if (!initialSummary || initialSummary.meeting_id !== meeting.id) return;
    const status = restoredSummaryStatus(initialSummary);
    setSummaryStatus(status);
    setSummaryError(status === 'error'
      ? initialSummary.error || i18n.t('meetingArea.toasts.summaryFailedRetry')
      : null);
    if (status !== 'processing' || !initialSummary.start) return;

    const generationId = ++generationIdRef.current;
    activeProcessIdRef.current = initialSummary.start;
    const isRegeneration = !!parseSummaryContent(initialSummary.data);
    startSummaryPolling(meeting.id, initialSummary.start, result =>
      pollingResultRef.current(result, generationId, isRegeneration));
  }, [initialSummary, meeting.id, startSummaryPolling]);

  const processSummary = useCallback(async ({
    transcriptText,
    transcriptTexts,
    customPrompt = '',
    isRegeneration = false,
  }: {
    transcriptText: string;
    transcriptTexts?: string[];
    customPrompt?: string;
    isRegeneration?: boolean;
  }) => {
    const previousAttempt = trackedAttemptRef.current;
    if (previousAttempt && !previousAttempt.finished) {
      await finishGeneration(previousAttempt.generationId, 'cancelled');
    }

    const generationId = ++generationIdRef.current;
    activeProcessIdRef.current = null;
    setSummaryStatus(isRegeneration ? 'regenerating' : 'processing');
    setSummaryError(null);

    try {
      if (!transcriptText.trim()) {
        await failGeneration(generationId, isRegeneration, i18n.t('meetingArea.toasts.noTranscriptText'), 'empty_result');
        return;
      }

      const timeSinceRecording = (Date.now() - new Date(meeting.created_at).getTime()) / 60000;
      trackedAttemptRef.current = {
        generationId,
        startedAt: Date.now(),
        provider: modelConfig.provider,
        model: modelConfig.model,
        finished: false,
      };
      await Analytics.trackSummaryGenerationStarted(
        modelConfig.provider,
        modelConfig.model,
        transcriptText.length,
        timeSinceRecording,
      );
      if (customPrompt.trim()) {
        await Analytics.trackCustomPromptUsed(customPrompt.trim().length);
      }
      // No "generating…" toast: summaryStatus drives a visible in-panel status message
      // and the button flips to "Stop", so the toast was pure narration.
      console.log(
        `${isRegeneration ? 'Regenerating' : 'Generating'} summary with ${modelConfig.provider}/${modelConfig.model}`
      );

      // Explicit per-meeting choice, else the pinned default, else the display language.
      const summaryLanguage = await resolveSummaryLanguage(meeting.id);
      if (!mountedRef.current || visibleMeetingIdRef.current !== meeting.id || generationId !== generationIdRef.current) {
        return;
      }

      const result = await invokeTauri<ProcessTranscriptResponse>('api_process_transcript', {
        text: transcriptText,
        model: modelConfig.provider,
        modelName: modelConfig.model,
        meetingId: meeting.id,
        chunkSize: 40000,
        overlap: 1000,
        customPrompt,
        templateId: selectedTemplate,
        summaryLanguage,
      });
      const processId = result.process_id;
      if (!mountedRef.current || visibleMeetingIdRef.current !== meeting.id) return;
      if (generationId !== generationIdRef.current) {
        await invokeTauri('api_cancel_summary', { meetingId: meeting.id, processId }).catch(() => {});
        return;
      }
      activeProcessIdRef.current = processId;

      startSummaryPolling(meeting.id, processId, result =>
        pollingResultRef.current(result, generationId, isRegeneration));
    } catch (error) {
      await failGeneration(generationId, isRegeneration, error instanceof Error ? error.message : i18n.t('meetingArea.toasts.generationFailedShort'));
    }
  }, [
    failGeneration,
    finishGeneration,
    meeting.created_at,
    meeting.id,
    modelConfig,
    onMeetingUpdated,
    selectedTemplate,
    setAiSummary,
    startSummaryPolling,
    updateMeetingTitle,
  ]);

  // Helper function to fetch ALL transcripts for summary generation
  const fetchAllTranscripts = useCallback(async (meetingId: string): Promise<Transcript[]> => {
    try {
      console.log('📊 Fetching all transcripts for meeting:', meetingId);

      // First, get total count by fetching first page
      const firstPage = await invokeTauri('api_get_meeting_transcripts', {
        meetingId,
        limit: 1,
        offset: 0,
      }) as { transcripts: Transcript[]; total_count: number; has_more: boolean };

      const totalCount = firstPage.total_count;
      console.log(`📊 Total transcripts in database: ${totalCount}`);

      if (totalCount === 0) {
        return [];
      }

      // Fetch all transcripts in one call
      const allData = await invokeTauri('api_get_meeting_transcripts', {
        meetingId,
        limit: totalCount,
        offset: 0,
      }) as { transcripts: Transcript[]; total_count: number; has_more: boolean };

      console.log(`✅ Fetched ${allData.transcripts.length} transcripts from database`);
      return allData.transcripts;
    } catch (error) {
      console.error('❌ Error fetching all transcripts:', error);
      toast.error(i18n.t('meetingArea.toasts.fetchTranscriptsFailed'));
      return [];
    }
  }, []);

  const buildSummaryTranscriptPayload = useCallback((allTranscripts: Transcript[]) => {
    const formatTime = (seconds: number | undefined, fallbackTimestamp: string): string => {
      if (seconds === undefined) {
        return fallbackTimestamp;
      }
      const totalSecs = Math.floor(seconds);
      return `[${Math.floor(totalSecs / 60).toString().padStart(2, '0')}:${(totalSecs % 60).toString().padStart(2, '0')}]`;
    };

    return {
      transcriptText: allTranscripts
        .map((transcript) => `${formatTime(transcript.audio_start_time, transcript.timestamp)} ${transcript.text}`)
        .join('\n'),
      transcriptTexts: allTranscripts.map((transcript) => transcript.text),
    };
  }, []);

  const showPreflightError = useCallback((message: string) => {
    setSummaryError(message);
    setSummaryStatus('error');
    toast.error(message);
  }, []);

  const handleGenerateSummary = useCallback(async (customPrompt: string = '') => {
    if (isModelConfigLoading) {
      console.log('⏳ Model configuration is still loading, please wait...');
      return;
    }
    const allTranscripts = await fetchAllTranscripts(meeting.id);
    if (!allTranscripts.length) {
      showPreflightError(i18n.t('meetingArea.toasts.noTranscripts'));
      return;
    }

    try {
      if (modelConfig.provider === 'ollama') {
        const models = await invokeTauri<unknown[]>('get_ollama_models', {
          endpoint: modelConfig.ollamaEndpoint || null,
        });
        if (models.length === 0) {
          showPreflightError(i18n.t('meetingArea.models.ollamaNoModelsSettings'));
          return;
        }
      }
      if (modelConfig.provider === 'builtin-ai') {
        if (!modelConfig.model) {
          showPreflightError(i18n.t('meetingArea.models.noBuiltInSelectedSettings'));
          onOpenModelSettings?.();
          return;
        }
        const isReady = await invokeTauri<boolean>('builtin_ai_is_model_ready', {
          modelName: modelConfig.model,
          refresh: true,
        });
        if (!isReady) {
          showPreflightError(i18n.t('meetingArea.models.builtInNotReadyCheck'));
          onOpenModelSettings?.();
          return;
        }
      }
    } catch (error) {
      console.error('Failed to validate summary model:', error);
      showPreflightError(i18n.t('meetingArea.models.validateFailedCheck'));
      return;
    }

    await processSummary({
      ...buildSummaryTranscriptPayload(allTranscripts),
      customPrompt,
    });
  }, [
    buildSummaryTranscriptPayload,
    fetchAllTranscripts,
    isModelConfigLoading,
    meeting.id,
    modelConfig,
    onOpenModelSettings,
    processSummary,
    showPreflightError,
  ]);

  // Public API: Regenerate summary from the current saved transcript
  const handleRegenerateSummary = useCallback(async () => {
    const allTranscripts = await fetchAllTranscripts(meeting.id);

    if (!allTranscripts.length) {
      console.error('No transcripts available for regeneration');
      toast.error(i18n.t('meetingArea.toasts.noTranscriptsRegenerate'));
      return;
    }

    await processSummary({
      ...buildSummaryTranscriptPayload(allTranscripts),
      isRegeneration: true
    });
  }, [meeting.id, fetchAllTranscripts, buildSummaryTranscriptPayload, processSummary]);

  // Public API: Stop ongoing summary generation
  const handleStopGeneration = useCallback(async () => {
    const generationId = generationIdRef.current;
    const processId = activeProcessIdRef.current;
    if (!processId) {
      generationIdRef.current += 1;
      activeProcessIdRef.current = null;
      setSummaryStatus('idle');
      setSummaryError(null);
      // No toast: the user pressed Stop, and the button flipping back from "Stop" to
      // "Generate" is the confirmation.
      await finishGeneration(generationId, 'cancelled');
      return;
    }

    try {
      const result = await invokeTauri<CancelSummaryResponse>('api_cancel_summary', {
        meetingId: meeting.id,
        processId,
      });
      if (!mountedRef.current || visibleMeetingIdRef.current !== meeting.id || generationId !== generationIdRef.current) {
        return;
      }
      if (result.cancelled) {
        generationIdRef.current += 1;
        activeProcessIdRef.current = null;
        stopSummaryPolling(meeting.id, processId);
        setSummaryStatus('idle');
        setSummaryError(null);
        await finishGeneration(generationId, 'cancelled');
      } else if (activeProcessIdRef.current === processId) {
        toast.info(i18n.t('meetingArea.toasts.alreadyFinishingTitle'), {
          description: i18n.t('meetingArea.toasts.alreadyFinishingDescription'),
        });
      }
    } catch (error) {
      console.error('Failed to cancel summary generation:', error);
      if (
        generationId === generationIdRef.current
        && activeProcessIdRef.current === processId
      ) {
        toast.error(i18n.t('meetingArea.toasts.stopFailedTitle'), {
          description: i18n.t('meetingArea.toasts.stopFailedDescription'),
        });
      }
    }
  }, [finishGeneration, meeting.id, stopSummaryPolling]);

  return {
    summaryStatus,
    summaryError,
    handleGenerateSummary,
    handleRegenerateSummary,
    handleStopGeneration,
    getSummaryStatusMessage,
  };
}
