// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {Button} from '@astryxdesign/core/Button';
import {CheckboxInput} from '@astryxdesign/core/CheckboxInput';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {HStack, VStack} from '@astryxdesign/core/Layout';
import {Text} from '@astryxdesign/core/Text';
import {api} from '../ipc/commands';
import {errorText, notify} from '../lib/notify';
import {useDownloads} from '../state/downloads';
import {useUi} from '../state/ui';
import {t} from '../i18n';

/** Confirms removal of one or more downloads. Offers to delete files only when a selected download is completed. */
export function RemoveDownloadDialog() {
  const isOpen = useUi(state => state.removeDialog.open);
  const ids = useUi(state => state.removeDialog.ids);
  const closeRemove = useUi(state => state.closeRemove);
  const records = useDownloads(state => state.records);
  const [deleteFile, setDeleteFile] = useState(false);

  useEffect(() => {
    if (isOpen) setDeleteFile(false);
  }, [isOpen, ids]);

  const targets = ids.filter(id => id in records);
  const single = targets.length === 1 ? records[targets[0]] : undefined;
  const completedCount = targets.filter(id => records[id].status === 'completed').length;
  const canDeleteFile = completedCount > 0;
  const many = targets.length > 1;

  async function remove(): Promise<void> {
    const failures: string[] = [];
    for (const id of targets) {
      const shouldDeleteFile = deleteFile && records[id].status === 'completed';
      try {
        await api.removeDownload(id, shouldDeleteFile);
        useDownloads.getState().remove(id);
      } catch (err) {
        failures.push(errorText(err));
      }
    }
    if (failures.length > 0) notify('error', failures[0]);
    closeRemove();
  }

  const title = many ? t('remove.titleMany', {count: targets.length}) : t('remove.title');
  const body = many
    ? t('remove.bodyMany', {count: targets.length})
    : t('remove.body', {name: single?.fileName ?? t('remove.thisDownload')});

  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={open => {
        if (!open) closeRemove();
      }}
      width={440}
      purpose="form"
      padding={4}>
      <DialogHeader title={title} onOpenChange={() => closeRemove()} />
      <VStack gap={4}>
        <Text>{body}</Text>
        {canDeleteFile && (
          <CheckboxInput
            label={many ? t('remove.deleteFiles') : t('remove.deleteFile')}
            value={deleteFile}
            onChange={checked => setDeleteFile(checked)}
          />
        )}
        <HStack gap={2} hAlign="end">
          <Button label={t('common.cancel')} variant="secondary" onClick={() => closeRemove()} />
          <Button label={t('common.remove')} variant="destructive" clickAction={remove} />
        </HStack>
      </VStack>
    </Dialog>
  );
}
