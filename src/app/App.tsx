// SPDX-License-Identifier: GPL-3.0-or-later
import {Fragment} from 'react';
import {Theme} from '@astryxdesign/core/theme';
import {defaultThemeSlug, themes} from '../astryx-themes';
import {AstryxLocale} from '../i18n/astryx';
import {useT} from '../i18n';
import {DownloadsPage} from '../pages/downloads/DownloadsPage';
import {useSettings} from '../state/settings';
import {useUi} from '../state/ui';
import {AppFrame} from './AppFrame';
import {
  LazyAddDownloadDialog,
  LazyAfterQueueDialog,
  LazyBatchAddDialog,
  LazyRemoveDownloadDialog,
  LazySettingsPage,
  LazySpeedLimitDialog,
  LazyStatsPage,
  LazyUpdateAddressDialog,
  LazyVerifyChecksumDialog,
} from './lazyDialogs';
import {ToastBridge} from './ToastBridge';
import {useBootstrap} from './useBootstrap';
import {useKeyboardShortcuts} from './useKeyboardShortcuts';
import {useUpdateCheck} from './useUpdateCheck';

export function App() {
  useBootstrap();
  useKeyboardShortcuts();
  useUpdateCheck();
  const mode = useSettings(state => state.value?.theme ?? 'system');
  const view = useUi(state => state.view);
  // Keying the tree by language remounts it, so every label is read again in the new language.
  const {lang} = useT();

  return (
    <AstryxLocale>
      <Theme theme={themes[defaultThemeSlug]} mode={mode}>
        <Fragment key={lang}>
          <AppFrame>
            {view === 'settings' ? <LazySettingsPage /> : view === 'stats' ? <LazyStatsPage /> : <DownloadsPage />}
          </AppFrame>
          <LazyAddDownloadDialog />
          <LazyBatchAddDialog />
          <LazyRemoveDownloadDialog />
          <LazySpeedLimitDialog />
          <LazyUpdateAddressDialog />
          <LazyVerifyChecksumDialog />
          <LazyAfterQueueDialog />
          <ToastBridge />
        </Fragment>
      </Theme>
    </AstryxLocale>
  );
}
