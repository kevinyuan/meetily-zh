import React, { useState, useEffect } from "react";
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { useTranslation } from 'react-i18next';
import Image from 'next/image';
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch";
import { UpdateDialog } from "./UpdateDialog";
import { updateService, UpdateInfo } from '@/services/updateService';
import { Button } from './ui/button';
import { Loader2, CheckCircle2 } from 'lucide-react';
import { toast } from 'sonner';


export function About() {
    const { t } = useTranslation();
    const [currentVersion, setCurrentVersion] = useState<string>('0.4.0');
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const [isChecking, setIsChecking] = useState(false);
    const [showUpdateDialog, setShowUpdateDialog] = useState(false);

    useEffect(() => {
        // Get current version on mount
        getVersion().then(setCurrentVersion).catch(console.error);
    }, []);

    // Upstream's About page sent people to meetily.zackriya.com to talk to their sales
    // team. That is their product and their release line, not this one — this build is a
    // fork with a different transcription engine, a different version, and a different
    // update channel, so pointing users there could only mislead them. It links to this
    // fork's own source instead.
    const handleSourceClick = async () => {
        try {
            await invoke('open_external_url', { url: 'https://github.com/kevinyuan/meetily-zh' });
        } catch (error) {
            console.error('Failed to open link:', error);
        }
    };

    const handleCheckForUpdates = async () => {
        setIsChecking(true);
        try {
            const info = await updateService.checkForUpdates(true);
            setUpdateInfo(info);
            if (info.available) {
                setShowUpdateDialog(true);
            } else {
                toast.success(t('settingsArea.about.latestVersion'));
            }
        } catch (error: any) {
            console.error('Failed to check for updates:', error);
            toast.error(t('settingsArea.about.checkFailed', {
                error: error.message || t('settingsArea.about.unknownError'),
            }));
        } finally {
            setIsChecking(false);
        }
    };

    return (
        <div className="p-4 space-y-4 h-[80vh] overflow-y-auto">
            {/* Compact Header */}
            <div className="text-center">
                <div className="mb-3">
                    <Image
                        src="icon_128x128.png"
                        alt={t('settingsArea.about.logoAlt')}
                        width={64}
                        height={64}
                        className="mx-auto"
                    />
                </div>
                {/* <h1 className="text-xl font-bold text-gray-900">Meetily</h1> */}
                <span className="text-sm text-gray-500"> v{currentVersion}</span>
                <p className="text-medium text-gray-600 mt-1">
                    {t('settingsArea.about.tagline')}
                </p>
                <div className="mt-3">
                    <Button
                        onClick={handleCheckForUpdates}
                        disabled={isChecking}
                        variant="outline"
                        size="sm"
                        className="text-xs"
                    >
                        {isChecking ? (
                            <>
                                <Loader2 className="h-3 w-3 mr-2 animate-spin" />
                                {t('settingsArea.about.checking')}
                            </>
                        ) : (
                            <>
                                <CheckCircle2 className="h-3 w-3 mr-2" />
                                {t('settingsArea.about.checkForUpdates')}
                            </>
                        )}
                    </Button>
                    {updateInfo?.available && (
                        <div className="mt-2 text-xs text-blue-600">
                            {t('settingsArea.about.updateAvailable', { version: updateInfo.version })}
                        </div>
                    )}
                </div>
            </div>

            {/* Features Grid - Compact */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-gray-800">{t('settingsArea.about.featuresTitle')}</h2>
                <div className="grid grid-cols-2 gap-2">
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">{t('settingsArea.about.features.privacy.title')}</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">{t('settingsArea.about.features.privacy.description')}</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">{t('settingsArea.about.features.anyModel.title')}</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">{t('settingsArea.about.features.anyModel.description')}</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">{t('settingsArea.about.features.costSmart.title')}</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">{t('settingsArea.about.features.costSmart.description')}</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">{t('settingsArea.about.features.everywhere.title')}</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">{t('settingsArea.about.features.everywhere.description')}</p>
                    </div>
                </div>
            </div>

            {/* What this fork is.
                This replaced upstream's "coming soon" roadmap and their sales pitch. Both
                described a product this build is not: the roadmap is a promise upstream
                made, not one this fork can keep, and the pitch invited users to contact a
                vendor who did not ship the binary they are running. */}
            <div className="text-center space-y-2">
                <h3 className="text-medium font-semibold text-gray-800">{t('settingsArea.about.fork.title')}</h3>
                <p className="text-s text-gray-600">
                    {t('settingsArea.about.fork.text')}
                </p>
                <button
                    onClick={handleSourceClick}
                    className="inline-flex items-center px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded transition-colors duration-200 shadow-sm hover:shadow-md"
                >
                    {t('settingsArea.about.fork.button')}
                </button>
            </div>

            {/* Attribution to upstream, as plain text.
                Credit belongs to them, but a link would invite users into a different
                release line than the one they are running. The README carries the link,
                with the explanation it needs. */}
            <div className="pt-2 border-t border-gray-200 text-center">
                <p className="text-xs text-gray-400">
                    {t('settingsArea.about.footer')}
                </p>
            </div>
            <AnalyticsConsentSwitch />

            {/* Update Dialog */}
            <UpdateDialog
                open={showUpdateDialog}
                onOpenChange={setShowUpdateDialog}
                updateInfo={updateInfo}
            />
        </div>

    )
}