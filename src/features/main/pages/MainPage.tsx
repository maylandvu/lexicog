import { useState, useEffect, Component, type ErrorInfo, type ReactNode } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { isTauri } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';
import { open } from '@tauri-apps/plugin-shell';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import { useWindowSelectionTracker } from '@/stores/selection';
import { WindowScaffold } from '@/layout/WindowScaffold';
import { MainSidebar, type MainTabKey } from '../components/MainSidebar';
import {
  HistoryBrowser,
  ReviewSession,
} from '../components';
import { ConfigureNav, ConfigureContent, type ConfigureSectionKey } from '../components/configure';
import { Button } from '@/shared/components/ui/button';
import { useNotification } from '@/shared/components/feedback/useNotification';
import {
  readConfigFromStore,
  resetTargetLangOfLexicalEntryLookup,
  resetTargetLangOfTranslation,
} from '@/services/config';
import { TARGET_LANGUAGE_CODES, type TargetLanguageCode } from '@/constants/languages';

interface ErrorBoundaryState {
  error: Error | null;
}

class TabErrorBoundary extends Component<{ children: ReactNode; activeTab: string }, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[TabErrorBoundary]', error, info.componentStack);
  }

  componentDidUpdate(prevProps: { activeTab: string }) {
    if (prevProps.activeTab !== this.props.activeTab && this.state.error) {
      this.setState({ error: null });
    }
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-full items-center justify-center p-6">
          <div className="max-w-md space-y-3 rounded-lg border border-[var(--color-error)] bg-[var(--color-bg-container)] p-6">
            <h3 className="text-sm font-semibold text-[var(--color-error)]">Rendering Error</h3>
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap text-xs text-[var(--color-text-secondary)]">
              {this.state.error.message}
              {'\n\n'}
              {this.state.error.stack}
            </pre>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

function detectDefaultTargetLanguage(): TargetLanguageCode {
  const browserLang = navigator.language || navigator.languages?.[0] || '';
  const langPrefix = browserLang.split('-')[0].toLowerCase();

  if ((TARGET_LANGUAGE_CODES as readonly string[]).includes(browserLang)) {
    return browserLang as TargetLanguageCode;
  }

  const prefixMap: Record<string, TargetLanguageCode> = {
    zh: 'zh-CN',
    en: 'en',
    ja: 'jp',
    es: 'es',
    fr: 'fr',
    de: 'de',
    it: 'it',
    ru: 'ru',
    pt: 'pt',
    ko: 'ko',
    vi: 'vi',
    th: 'th',
    el: 'el',
  };

  return prefixMap[langPrefix] || 'en';
}

export default function MainPage() {
  const [activeTab, setActiveTab] = useState<MainTabKey>('configure');
  const isMacOS = typeof navigator !== 'undefined' && navigator.userAgent.toLowerCase().includes('mac');
  useWindowSelectionTracker();

  useEffect(() => {
    async function initTargetLanguages() {
      const [lookupLang, translationLang] = await Promise.all([
        readConfigFromStore('targetLangOfLexicalEntryLookup'),
        readConfigFromStore('targetLangOfTranslation'),
      ]);

      const defaultLang = detectDefaultTargetLanguage();

      await Promise.all([
        !lookupLang ? resetTargetLangOfLexicalEntryLookup(defaultLang) : Promise.resolve(),
        !translationLang ? resetTargetLangOfTranslation(defaultLang) : Promise.resolve(),
      ]);
    }
    initTargetLanguages();
  }, []);

  return (
    <WindowScaffold variant="main" className="main-window">
      <div className="flex h-full w-full overflow-hidden bg-[var(--color-bg-base)]">
        <MainSidebar activeTab={activeTab} onTabChange={setActiveTab} isMacOS={isMacOS} />

        <div
          data-tauri-drag-region
          className={cn(
            'drag-region flex min-w-0 flex-1 flex-col overflow-hidden p-3 md:p-4',
            isMacOS && 'pt-4',
          )}
        >
          <div
            className="no-drag-region flex min-h-0 flex-1 flex-col overflow-hidden"
            data-tauri-drag-region="false"
          >
            <TabErrorBoundary activeTab={activeTab}>
              {activeTab === 'configure' && <ConfigureSection />}
              {activeTab === 'lookupHistory' && <LookupHistorySection />}
              {activeTab === 'review' && <ReviewSection />}
              {activeTab === 'about' && <AboutSection />}
            </TabErrorBoundary>
          </div>
        </div>
      </div>
    </WindowScaffold>
  );
}

function ConfigureSection() {
  const [activeSection, setActiveSection] = useState<ConfigureSectionKey>('vendorApi');

  return (
    <div className="flex h-full min-w-0 gap-3">
      <div className="w-52 shrink-0 overflow-hidden rounded-[24px] bg-[var(--color-bg-sidebar)] shadow-[var(--color-panel-shadow)]">
        <ConfigureNav activeSection={activeSection} onSectionChange={setActiveSection} />
      </div>
      <div className="min-w-0 flex-1 overflow-hidden rounded-[28px] bg-[var(--color-bg-container)] shadow-[var(--color-panel-shadow)]">
        <ConfigureContent activeSection={activeSection} />
      </div>
    </div>
  );
}

function LookupHistorySection() {
  return <HistoryBrowser className="h-full" />;
}

function ReviewSection() {
  return <ReviewSession className="h-full" />;
}

const RELEASE_NOTES_URL = 'https://github.com/maylandvu/lexicog/releases';

function formatUpdateVersion(version: string): string {
  return version.startsWith('v') ? version : `v${version}`;
}

function AboutSection() {
  const { t } = useTranslation();
  const { notify } = useNotification();
  const [appVersion, setAppVersion] = useState(import.meta.env.VITE_APP_VERSION || '0.1.0');
  const [isCheckingForUpdates, setIsCheckingForUpdates] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    void getVersion()
      .then(setAppVersion)
      .catch((error) => {
        console.error('[AboutSection] failed to read app version', error);
      });
  }, []);

  async function handleOpenReleaseNotes() {
    try {
      await open(RELEASE_NOTES_URL);
    } catch (error) {
      notify({
        type: 'error',
        message: t('about.releaseLogOpenFailed'),
        error,
      });
    }
  }

  async function handleCheckForUpdates() {
    if (isCheckingForUpdates) {
      return;
    }

    if (!isTauri()) {
      notify({
        type: 'warning',
        message: t('about.updaterUnavailable'),
      });
      return;
    }

    let update: Update | null = null;

    try {
      setIsCheckingForUpdates(true);
      setUpdateStatus(t('about.updateChecking'));

      update = await check({ timeout: 30_000 });

      if (!update) {
        const message = t('about.upToDate');
        setUpdateStatus(message);
        notify({ type: 'success', message });
        return;
      }

      const versionLabel = formatUpdateVersion(update.version);
      const availableMessage = t('about.updateAvailable', { version: versionLabel });
      setUpdateStatus(availableMessage);
      notify({ type: 'info', message: availableMessage });

      let downloaded = 0;
      let contentLength = 0;

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength ?? 0;
            setUpdateStatus(
              contentLength > 0
                ? t('about.updateDownloading', { version: versionLabel, progress: 0 })
                : t('about.updateDownloadingUnknown', { version: versionLabel }),
            );
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            if (contentLength > 0) {
              const progress = Math.min(100, Math.round((downloaded / contentLength) * 100));
              setUpdateStatus(
                t('about.updateDownloading', { version: versionLabel, progress }),
              );
            } else {
              setUpdateStatus(t('about.updateDownloadingUnknown', { version: versionLabel }));
            }
            break;
          case 'Finished':
            setUpdateStatus(t('about.updateInstalling', { version: versionLabel }));
            break;
        }
      });

      const installedMessage = t('about.updateInstalled', { version: versionLabel });
      setUpdateStatus(installedMessage);
      notify({
        type: 'success',
        message: installedMessage,
      });
      await relaunch();
    } catch (error) {
      const message = t('about.updateFailed');
      setUpdateStatus(message);
      notify({
        type: 'error',
        message,
        error,
      });
    } finally {
      setIsCheckingForUpdates(false);
      if (update) {
        await update.close().catch(() => undefined);
      }
    }
  }

  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="max-w-prose space-y-4 rounded-[28px] border border-[rgba(0,0,0,0.03)] bg-[var(--color-bg-container)] p-10 text-center shadow-[0_4px_6px_-1px_rgba(0,0,0,0.04),0_2px_4px_-1px_rgba(0,0,0,0.02)]">
        <h2 className="font-editorial text-2xl font-semibold text-[var(--color-text-primary)]">
          {t('about.appName')}
        </h2>
        <p className="text-sm text-[var(--color-text-secondary)]">
          {t('about.description')}
        </p>
        <p className="text-sm text-[var(--color-text-secondary)]">
          {t('about.version', { version: appVersion })}
        </p>
        <div className="flex items-center justify-center gap-4">
          <Button type="button" variant="link" onClick={() => void handleOpenReleaseNotes()}>
            {t('about.releaseLog')}
          </Button>
          <Button
            type="button"
            variant="link"
            disabled={isCheckingForUpdates}
            onClick={() => void handleCheckForUpdates()}
          >
            {isCheckingForUpdates ? t('about.checkingForUpdates') : t('about.checkForUpdates')}
          </Button>
        </div>
        {updateStatus ? (
          <p className="text-xs text-[var(--color-text-secondary)]">{updateStatus}</p>
        ) : null}
      </div>
    </div>
  );
}
