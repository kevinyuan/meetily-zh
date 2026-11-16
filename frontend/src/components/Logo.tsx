import React from "react";
import Image from "next/image";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./ui/dialog";
import { VisuallyHidden } from "./ui/visually-hidden";
import { About } from "./About";

interface LogoProps {
  isCollapsed: boolean;
  doodlePath?: string | null;
  doodleDate?: string | null;
}

const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(
  ({ isCollapsed, doodlePath, doodleDate }, ref) => {
    const { t } = useTranslation();
    const [failedSrc, setFailedSrc] = React.useState<string | null>(null);
    const doodleSrc = doodlePath && doodleDate
      ? `${convertFileSrc(doodlePath)}?d=${doodleDate}`
      : null;
    return (
      <Dialog aria-describedby={undefined}>
        {isCollapsed ? (
          <DialogTrigger asChild>
            <button
              ref={ref}
              type="button"
              className="flex items-center justify-center mb-2 cursor-pointer bg-transparent border-none p-0 hover:opacity-80 transition-opacity"
              aria-label={t('recordingArea.nav.aboutMeetily')}
            >
              {doodleSrc && doodleSrc !== failedSrc ? (
                // eslint-disable-next-line @next/next/no-img-element
                <img
                  src={doodleSrc}
                  alt={t('recordingArea.nav.logoAlt')}
                  width={40}
                  height={40}
                  className="w-10 h-10 object-contain"
                  onError={() => setFailedSrc(doodleSrc)}
                />
              ) : (
                <Image
                  src="/logo-collapsed.png"
                  alt="Meetily"
                  width={40}
                  height={40}
                  className="object-contain"
                  priority
                />
              )}
            </button>
          </DialogTrigger>
        ) : (
          <DialogTrigger asChild>
            <button
              ref={ref}
              type="button"
              className="w-full text-lg text-center border rounded-full bg-blue-50 border-white font-semibold text-gray-700 mb-2 block items-center cursor-pointer hover:opacity-80 transition-opacity focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              aria-label={t('recordingArea.nav.aboutMeetily')}
            >
              <span>Meetily</span>
            </button>
          </DialogTrigger>
        )}
        <DialogContent>
          <VisuallyHidden>
            <DialogTitle>{t('recordingArea.nav.aboutMeetily')}</DialogTitle>
          </VisuallyHidden>
          <About />
        </DialogContent>
      </Dialog>
    );
  },
);

Logo.displayName = "Logo";

export default Logo;
