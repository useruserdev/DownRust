// SPDX-License-Identifier: GPL-3.0-or-later
import {Badge} from '@astryxdesign/core/Badge';
import type {BadgeVariant} from '@astryxdesign/core/Badge';
import {Heading} from '@astryxdesign/core/Heading';
import {HStack} from '@astryxdesign/core/HStack';
import {Icon} from '@astryxdesign/core/Icon';
import {IconButton} from '@astryxdesign/core/IconButton';
import {StackItem} from '@astryxdesign/core/Stack';
import {VStack} from '@astryxdesign/core/VStack';
import {ExternalLink, FolderOpen, Copy} from 'lucide-react';
import {Button} from '@astryxdesign/core/Button';
import {DropdownMenu} from '@astryxdesign/core/DropdownMenu';
import {CategoryIcon} from '../../../lib/categories';
import {copyText} from '../../../lib/clipboard';
import {notify, errorText} from '../../../lib/notify';
import {statusLabel} from '../../../lib/status';
import {kindBadgeLabel} from '../../../lib/kind';
import {api} from '../../../ipc/commands';
import type {DownloadRecord, DownloadStatus} from '../../../ipc/types';
import {useUi} from '../../../state/ui';
import {t} from '../../../i18n';
import {downloadActions} from '../RowContextMenu';
import {canUpdateAddress, QUEUE_MOVES} from '../queueActions';

const BADGE_VARIANT: Record<DownloadStatus, BadgeVariant> = {
  queued: 'neutral',
  downloading: 'info',
  paused: 'warning',
  completed: 'success',
  error: 'error',
};

/** Title, status badge and file actions at the top of the details panel. */
export function InspectorHeader({record}: {record: DownloadRecord}) {
  const openFile = () => {
    api.openFile(record.id).catch(err => notify('error', errorText(err)));
  };
  const showInFolder = () => {
    api.showInFolder(record.id).catch(err => notify('error', errorText(err)));
  };
  const copyAddress = () => {
    copyText(record.url)
      .then(() => notify('info', t('toast.addressCopied')))
      .catch(err => notify('error', errorText(err)));
  };

  return (
    <VStack gap={2}>
      <HStack gap={2} align="center">
        <CategoryIcon category={record.category} size="md" />
        <StackItem size="fill">
          <Heading level={2} maxLines={2}>
            {record.fileName}
          </Heading>
        </StackItem>
        {kindBadgeLabel(record.kind) !== null && <Badge variant="info" label={kindBadgeLabel(record.kind) ?? ''} />}
      </HStack>
      <HStack gap={1} align="center">
        <Badge variant={BADGE_VARIANT[record.status]} label={statusLabel(record.status)} />
        <StackItem size="fill" />
        <IconButton
          label={t('common.openFile')}
          tooltip={t('common.openFile')}
          variant="ghost"
          size="sm"
          isDisabled={record.status !== 'completed'}
          onClick={openFile}
          icon={<Icon icon={ExternalLink} />}
        />
        <IconButton
          label={t('common.showInFolder')}
          tooltip={t('common.showInFolder')}
          variant="ghost"
          size="sm"
          onClick={showInFolder}
          icon={<Icon icon={FolderOpen} />}
        />
        <IconButton
          label={t('common.copyAddress')}
          tooltip={t('common.copyAddress')}
          variant="ghost"
          size="sm"
          onClick={copyAddress}
          icon={<Icon icon={Copy} />}
        />
      </HStack>
      {(record.status === 'queued' || canUpdateAddress(record.status)) && (
        <HStack gap={2} align="center">
          {record.status === 'queued' && (
            <DropdownMenu
              button={{label: t('queue.label'), variant: 'secondary', size: 'sm'}}
              items={QUEUE_MOVES.map(move => ({
                label: t(move.labelKey),
                onClick: () => downloadActions.moveInQueue(record.id, move.to),
              }))}
            />
          )}
          {canUpdateAddress(record.status) && (
            <Button
              label={t('common.updateAddressMore')}
              variant="secondary"
              size="sm"
              clickAction={() => useUi.getState().openAddress(record.id)}
            />
          )}
        </HStack>
      )}
    </VStack>
  );
}
