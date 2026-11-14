// SPDX-License-Identifier: GPL-3.0-or-later
import {useMemo} from 'react';
import type {ReactNode} from 'react';
import {ContextMenu} from '@astryxdesign/core/ContextMenu';
import type {ContextMenuOption} from '@astryxdesign/core/ContextMenu';
import {Icon} from '@astryxdesign/core/Icon';
import {ArrowDown, ArrowDownToLine, ArrowUp, ArrowUpToLine, Copy, FolderOpen, Gauge, Link, Pause, Play, RotateCw, FileSymlink, Trash2} from 'lucide-react';
import {api} from '../../ipc/commands';
import type {DownloadId, DownloadRecord, DownloadStatus, QueueMove} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import {useDownloads} from '../../state/downloads';
import {useUi} from '../../state/ui';
import {t} from '../../i18n';
import {canUpdateAddress, QUEUE_MOVES} from './queueActions';

/** Runs a backend command and turns any failure into an error toast. */
async function attempt(task: () => Promise<unknown>): Promise<void> {
  try {
    await task();
  } catch (err) {
    notify('error', errorText(err));
  }
}

export const downloadActions = {
  pause: (id: DownloadId) => attempt(() => api.pauseDownload(id)),
  resume: (id: DownloadId) => attempt(() => api.resumeDownload(id)),
  restart: (id: DownloadId) => attempt(() => api.restartDownload(id)),
  openFile: (id: DownloadId) => attempt(() => api.openFile(id)),
  showInFolder: (id: DownloadId) => attempt(() => api.showInFolder(id)),
  pauseAll: () => attempt(() => api.pauseAll()),
  resumeAll: () => attempt(() => api.resumeAll()),
  moveInQueue: (id: DownloadId, to: QueueMove) => attempt(() => api.moveInQueue(id, to)),
  copyAddress: (url: string) =>
    attempt(async () => {
      await navigator.clipboard.writeText(url);
      notify('info', t('toast.addressCopied'));
    }),
  openRemove: (id: DownloadId) => useUi.getState().openRemove(id),
  openAddress: (id: DownloadId) => useUi.getState().openAddress(id),
};

/** Double-click or Enter: open a finished file, otherwise pause or resume the transfer. */
export function activateDownload(record: DownloadRecord): void {
  const status: DownloadStatus = record.status;
  if (status === 'completed') return void downloadActions.openFile(record.id);
  if (status === 'downloading' || status === 'queued') return void downloadActions.pause(record.id);
  if (status === 'paused' || status === 'error') void downloadActions.resume(record.id);
}

const QUEUE_ICONS: Record<QueueMove, typeof ArrowUp> = {
  top: ArrowUpToLine,
  up: ArrowUp,
  down: ArrowDown,
  bottom: ArrowDownToLine,
};

function buildItems(record: DownloadRecord): ContextMenuOption[] {
  const {id, status} = record;
  const canPause = status === 'downloading' || status === 'queued';
  const canResume = status === 'paused' || status === 'error';
  const canRestart = status !== 'queued';

  const transfer: ContextMenuOption[] = [];
  if (canPause) transfer.push({label: t('common.pause'), icon: <Icon icon={Pause} size="sm" />, onClick: () => downloadActions.pause(id)});
  if (canResume) transfer.push({label: t('common.resume'), icon: <Icon icon={Play} size="sm" />, onClick: () => downloadActions.resume(id)});
  if (canRestart) transfer.push({label: t('common.restart'), icon: <Icon icon={RotateCw} size="sm" />, onClick: () => downloadActions.restart(id)});
  transfer.push({label: t('menu.setSpeedLimit'), icon: <Icon icon={Gauge} size="sm" />, onClick: () => useUi.getState().openSpeedLimit(id)});
  if (canUpdateAddress(status)) {
    transfer.push({label: t('common.updateAddressMore'), icon: <Icon icon={Link} size="sm" />, onClick: () => downloadActions.openAddress(id)});
  }

  const queue: ContextMenuOption[] =
    status === 'queued'
      ? QUEUE_MOVES.map(item => ({
          label: t(item.labelKey),
          icon: <Icon icon={QUEUE_ICONS[item.to]} size="sm" />,
          onClick: () => downloadActions.moveInQueue(id, item.to),
        }))
      : [];

  const files: ContextMenuOption[] = [];
  if (status === 'completed') {
    files.push({label: t('common.openFile'), icon: <Icon icon={FileSymlink} size="sm" />, onClick: () => downloadActions.openFile(id)});
  }
  files.push({label: t('common.showInFolder'), icon: <Icon icon={FolderOpen} size="sm" />, onClick: () => downloadActions.showInFolder(id)});
  files.push({label: t('common.copyAddress'), icon: <Icon icon={Copy} size="sm" />, onClick: () => downloadActions.copyAddress(record.url)});

  const removal: ContextMenuOption[] = [
    {label: t('common.remove'), icon: <Icon icon={Trash2} size="sm" />, variant: 'destructive', onClick: () => downloadActions.openRemove(id)},
  ];

  const groups = [transfer, queue, files, removal].filter(group => group.length > 0);
  return groups.flatMap((group, index) => (index === 0 ? group : [{type: 'divider' as const}, ...group]));
}

/** Context menu for the selected row. The table selects a row on right-click before the menu opens. */
export function RowContextMenu({children}: {children: ReactNode}) {
  const record = useDownloads(state => (state.selectedId === null ? undefined : state.records[state.selectedId]));
  const items = useMemo(() => (record ? buildItems(record) : []), [record]);

  return (
    <ContextMenu items={items} label={t('menu.actions')} isDisabled={!record}>
      {children}
    </ContextMenu>
  );
}
