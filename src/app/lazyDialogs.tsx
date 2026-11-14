// SPDX-License-Identifier: GPL-3.0-or-later
import {lazy, Suspense} from 'react';
import {Spinner} from '@astryxdesign/core/Spinner';
import {useUi} from '../state/ui';
import {t} from '../i18n';

// Each piece loads its own chunk the first time it is shown, so the main bundle stays small.
const AddDownloadDialog = lazy(() =>
  import('../dialogs/AddDownloadDialog').then(module => ({default: module.AddDownloadDialog})),
);
const RemoveDownloadDialog = lazy(() =>
  import('../dialogs/RemoveDownloadDialog').then(module => ({default: module.RemoveDownloadDialog})),
);
const SpeedLimitDialog = lazy(() =>
  import('../dialogs/SpeedLimitDialog').then(module => ({default: module.SpeedLimitDialog})),
);
const BatchAddDialog = lazy(() =>
  import('../dialogs/BatchAddDialog').then(module => ({default: module.BatchAddDialog})),
);
const UpdateAddressDialog = lazy(() =>
  import('../dialogs/UpdateAddressDialog').then(module => ({default: module.UpdateAddressDialog})),
);
const VerifyChecksumDialog = lazy(() =>
  import('../dialogs/VerifyChecksumDialog').then(module => ({default: module.VerifyChecksumDialog})),
);
const AfterQueueDialog = lazy(() =>
  import('../dialogs/AfterQueueDialog').then(module => ({default: module.AfterQueueDialog})),
);
const SettingsPage = lazy(() =>
  import('../pages/settings/SettingsPage').then(module => ({default: module.SettingsPage})),
);
const StatsPage = lazy(() => import('../pages/stats/StatsPage').then(module => ({default: module.StatsPage})));

/** Batch add dialog. Its chunk loads on first open. */
export function LazyBatchAddDialog() {
  const isOpen = useUi(state => state.batchDialog.open);
  if (!isOpen) return null;
  return (
    <Suspense fallback={null}>
      <BatchAddDialog />
    </Suspense>
  );
}

/** Statistics view. Its chunk loads when the view is first opened. */
export function LazyStatsPage() {
  return (
    <Suspense fallback={<Spinner label={t('common.loadingStats')} />}>
      <StatsPage />
    </Suspense>
  );
}

/** Add download dialog. Its chunk loads on first open. */
export function LazyAddDownloadDialog() {
  const isOpen = useUi(state => state.addDialog.open);
  if (!isOpen) return null;
  return (
    <Suspense fallback={null}>
      <AddDownloadDialog />
    </Suspense>
  );
}

/** Remove confirmation. Its chunk loads on first open. */
export function LazyRemoveDownloadDialog() {
  const isOpen = useUi(state => state.removeDialog.open);
  if (!isOpen) return null;
  return (
    <Suspense fallback={null}>
      <RemoveDownloadDialog />
    </Suspense>
  );
}

/** Speed limit dialog. Its chunk loads on first open. */
export function LazySpeedLimitDialog() {
  const isOpen = useUi(state => state.speedDialog.open);
  if (!isOpen) return null;
  return (
    <Suspense fallback={null}>
      <SpeedLimitDialog />
    </Suspense>
  );
}

/** Update address dialog. Its chunk loads on first open. */
export function LazyUpdateAddressDialog() {
  const isOpen = useUi(state => state.addressDialog.open);
  if (!isOpen) return null;
  return (
    <Suspense fallback={null}>
      <UpdateAddressDialog />
    </Suspense>
  );
}

/** Checksum verification dialog. Its chunk loads on first open. */
export function LazyVerifyChecksumDialog() {
  const isOpen = useUi(state => state.verifyDialog.open);
  if (!isOpen) return null;
  return (
    <Suspense fallback={null}>
      <VerifyChecksumDialog />
    </Suspense>
  );
}

/** After-queue countdown. Its chunk loads when the countdown starts. */
export function LazyAfterQueueDialog() {
  const isOpen = useUi(state => state.countdown !== null);
  if (!isOpen) return null;
  return (
    <Suspense fallback={null}>
      <AfterQueueDialog />
    </Suspense>
  );
}

/** Settings view. Its chunk loads when the view is first opened. */
export function LazySettingsPage() {
  return (
    <Suspense fallback={<Spinner label={t('common.loadingSettings')} />}>
      <SettingsPage />
    </Suspense>
  );
}
