// SPDX-License-Identifier: GPL-3.0-or-later
import {Banner} from '@astryxdesign/core/Banner';
import {Button} from '@astryxdesign/core/Button';
import {ScrollableArea} from '@astryxdesign/core/ScrollableArea';
import {VStack} from '@astryxdesign/core/VStack';
import {api} from '../../../ipc/commands';
import type {DownloadId} from '../../../ipc/types';
import {errorText, notify} from '../../../lib/notify';
import {useDownloads} from '../../../state/downloads';
import {t} from '../../../i18n';
import {ConnectionsList} from './ConnectionsList';
import {InspectorEmpty} from './InspectorEmpty';
import {InspectorHeader} from './InspectorHeader';
import {TransferFacts} from './TransferFacts';
import {TransferReadout} from './TransferReadout';

/** Details panel for one download: header, readout, connections and facts. */
export function DownloadInspector({id}: {id: DownloadId}) {
  const record = useDownloads(state => state.records[id]);
  if (!record) return <InspectorEmpty />;

  const retry = () => {
    api.resumeDownload(record.id).catch(err => notify('error', errorText(err)));
  };

  return (
    <ScrollableArea axis="block" label={t('inspector.label')}>
      <VStack gap={4} padding={4}>
        <InspectorHeader record={record} />
        {record.status === 'error' ? (
          <Banner
            status="error"
            title={t('inspector.failedTitle')}
            description={record.error ?? t('inspector.failedHint')}
            collapsible={false}
            endContent={<Button label={t('common.retry')} variant="secondary" size="sm" onClick={retry} />}
          />
        ) : null}
        <TransferReadout record={record} />
        <ConnectionsList id={record.id} />
        <TransferFacts record={record} />
      </VStack>
    </ScrollableArea>
  );
}
