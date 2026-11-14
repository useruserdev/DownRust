// SPDX-License-Identifier: GPL-3.0-or-later
import {Badge} from '@astryxdesign/core/Badge';
import type {BadgeVariant} from '@astryxdesign/core/Badge';
import {Button} from '@astryxdesign/core/Button';
import {HStack} from '@astryxdesign/core/HStack';
import {MetadataList, MetadataListItem} from '@astryxdesign/core/MetadataList';
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {Text} from '@astryxdesign/core/Text';
import {Timestamp} from '@astryxdesign/core/Timestamp';
import {api} from '../../../ipc/commands';
import type {ChecksumState, DownloadRecord, ScanState} from '../../../ipc/types';
import {useUi} from '../../../state/ui';
import {humanBytes} from '../../../lib/format';
import {errorText, notify} from '../../../lib/notify';
import {t} from '../../../i18n';
import type {MessageKey} from '../../../i18n';

const BYTES_PER_KB = 1024;

const CHECKSUM_BADGE: Record<ChecksumState, {variant: BadgeVariant; labelKey: MessageKey}> = {
  verifying: {variant: 'info', labelKey: 'inspector.checksum.verifying'},
  ok: {variant: 'success', labelKey: 'inspector.checksum.ok'},
  mismatch: {variant: 'error', labelKey: 'inspector.checksum.mismatch'},
};

const SCAN_BADGE: Record<ScanState, {variant: BadgeVariant; labelKey: MessageKey}> = {
  scanning: {variant: 'info', labelKey: 'inspector.scan.scanning'},
  clean: {variant: 'success', labelKey: 'inspector.scan.clean'},
  threat: {variant: 'error', labelKey: 'inspector.scan.threat'},
  failed: {variant: 'warning', labelKey: 'inspector.scan.failed'},
};

/** Address, target folder, size, resume support and editable limits for one download. */
export function TransferFacts({record}: {record: DownloadRecord}) {
  const setConnections = (connections: number) => {
    api.setDownloadConnections(record.id, connections).catch(err => notify('error', errorText(err)));
  };
  const setSpeedLimit = (limitKb: number | null) => {
    const limit = limitKb === null ? null : Math.round(limitKb * BYTES_PER_KB);
    api.setDownloadSpeedLimit(record.id, limit).catch(err => notify('error', errorText(err)));
  };

  return (
    <MetadataList>
      <MetadataListItem label={t('common.address')}>
        <Text type="supporting" maxLines={2}>
          {record.url}
        </Text>
      </MetadataListItem>
      <MetadataListItem label={t('inspector.savedTo')}>
        <Text type="supporting" maxLines={2}>
          {record.saveDir}
        </Text>
      </MetadataListItem>
      <MetadataListItem label={t('inspector.size')}>
        <Text type="supporting" hasTabularNumbers>
          {record.totalSize === null ? t('common.unknown') : humanBytes(record.totalSize)}
        </Text>
      </MetadataListItem>
      <MetadataListItem label={t('inspector.resume')}>
        <Text type="supporting">
          {record.resumable ? t('common.supported') : t('inspector.resumeNo')}
        </Text>
      </MetadataListItem>
      <MetadataListItem label={t('table.added')}>
        <Timestamp value={Math.floor(record.createdAt / 1000)} format="date_time" type="supporting" />
      </MetadataListItem>
      <MetadataListItem label={t('common.connections')}>
        <NumberInput
          label={t('common.connections')}
          isLabelHidden
          size="sm"
          min={1}
          max={32}
          step={1}
          hasNumberSteppers
          isWheelEnabled={false}
          value={record.connections}
          onChange={setConnections}
        />
      </MetadataListItem>
      <MetadataListItem label={t('inspector.speedLimit')}>
        <NumberInput
          label={t('inspector.speedLimitAria')}
          isLabelHidden
          size="sm"
          min={1}
          step={64}
          units={t('units.kbps')}
          placeholder={t('inspector.noLimitPlaceholder')}
          hasClear
          isWheelEnabled={false}
          value={record.speedLimit === null ? null : Math.round(record.speedLimit / BYTES_PER_KB)}
          onChange={setSpeedLimit}
        />
      </MetadataListItem>
      <MetadataListItem label="SHA-256">
        <HStack gap={2} align="center">
          {record.checksum ? (
            <Badge variant={CHECKSUM_BADGE[record.checksum].variant} label={t(CHECKSUM_BADGE[record.checksum].labelKey)} />
          ) : (
            <Text type="supporting">{t('inspector.notChecked')}</Text>
          )}
          {record.status === 'completed' && (
            <Button
              label={t('inspector.verify')}
              variant="ghost"
              size="sm"
              clickAction={() => useUi.getState().openVerify(record.id)}
            />
          )}
        </HStack>
      </MetadataListItem>
      {record.scan ? (
        <MetadataListItem label={t('inspector.virusScan')}>
          <Badge variant={SCAN_BADGE[record.scan].variant} label={t(SCAN_BADGE[record.scan].labelKey)} />
        </MetadataListItem>
      ) : null}
    </MetadataList>
  );
}
