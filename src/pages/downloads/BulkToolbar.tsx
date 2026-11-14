// SPDX-License-Identifier: GPL-3.0-or-later
import {Button} from '@astryxdesign/core/Button';
import {TableSelectionToolbar} from '@astryxdesign/core/Table';
import type {DownloadId, DownloadRecord} from '../../ipc/types';
import {useDownloads} from '../../state/downloads';
import {useUi} from '../../state/ui';
import {t} from '../../i18n';
import {downloadActions} from './RowContextMenu';

const isPausable = (record: DownloadRecord) => record.status === 'downloading' || record.status === 'queued';
const isResumable = (record: DownloadRecord) => record.status === 'paused' || record.status === 'error';
const isRestartable = (record: DownloadRecord) => record.status !== 'queued';

/** Runs an action on every checked download that accepts it. */
function forEachMatching(
  ids: DownloadId[],
  accepts: (record: DownloadRecord) => boolean,
  run: (id: DownloadId) => Promise<void>,
): void {
  const records = useDownloads.getState().records;
  for (const id of ids) {
    const record = records[id];
    if (record && accepts(record)) void run(id);
  }
}

/** Bulk action bar shown above the table while at least one row is checked. */
export function BulkToolbar({ids}: {ids: DownloadId[]}) {
  const records = useDownloads(state => state.records);
  const checked = ids.map(id => records[id]).filter((record): record is DownloadRecord => record !== undefined);
  const canPause = checked.some(isPausable);
  const canResume = checked.some(isResumable);
  const canRestart = checked.some(isRestartable);

  return (
    <TableSelectionToolbar
      selection={{
        selectedKeys: new Set(ids.map(String)),
        selectedCount: ids.length,
        hasSelection: ids.length > 0,
        clearSelection: () => useDownloads.getState().clearChecked(),
      }}
      label={t('bulk.label')}
      clearLabel={t('bulk.clear')}
      startContent={
        <>
          <Button
            label={t('common.resume')}
            variant="secondary"
            isDisabled={!canResume}
            clickAction={() => forEachMatching(ids, isResumable, downloadActions.resume)}
          />
          <Button
            label={t('common.pause')}
            variant="secondary"
            isDisabled={!canPause}
            clickAction={() => forEachMatching(ids, isPausable, downloadActions.pause)}
          />
          <Button
            label={t('common.restart')}
            variant="secondary"
            isDisabled={!canRestart}
            clickAction={() => forEachMatching(ids, isRestartable, downloadActions.restart)}
          />
          <Button label={t('common.remove')} variant="secondary" clickAction={() => useUi.getState().openRemove(ids)} />
        </>
      }
    />
  );
}
