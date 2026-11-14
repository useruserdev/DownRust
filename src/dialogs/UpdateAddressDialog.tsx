// SPDX-License-Identifier: GPL-3.0-or-later
import {useState} from 'react';
import {Banner} from '@astryxdesign/core/Banner';
import {Button} from '@astryxdesign/core/Button';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {HStack, Layout, LayoutContent, LayoutFooter, VStack} from '@astryxdesign/core/Layout';
import {Text} from '@astryxdesign/core/Text';
import {TextInput} from '@astryxdesign/core/TextInput';
import {api} from '../ipc/commands';
import type {DownloadId} from '../ipc/types';
import {errorText, notify} from '../lib/notify';
import {useDownloads} from '../state/downloads';
import {useUi} from '../state/ui';
import {isProbeableUrl} from './useProbe';
import {t} from '../i18n';

/** Update address dialog, driven by `useUi().addressDialog`. The address field starts with the current link. */
export function UpdateAddressDialog() {
  const {open, id} = useUi(state => state.addressDialog);
  const closeAddress = useUi(state => state.closeAddress);
  const record = useDownloads(state => (id === null ? undefined : state.records[id]));

  if (!open || id === null || !record) return null;

  return (
    <Dialog isOpen onOpenChange={isOpen => !isOpen && closeAddress()} purpose="form" width={520}>
      <AddressForm id={id} current={record.url} />
    </Dialog>
  );
}

function AddressForm({id, current}: {id: DownloadId; current: string}) {
  const closeAddress = useUi(state => state.closeAddress);
  const upsert = useDownloads(state => state.upsert);
  const [url, setUrl] = useState(current);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const canSubmit = isProbeableUrl(url) && !isSaving;
  const showFormatError = url.trim() !== '' && !isProbeableUrl(url);

  async function save() {
    setIsSaving(true);
    setError(null);
    try {
      const record = await api.refreshAddress(id, url.trim());
      upsert(record);
      notify('info', t('address.updated'));
      closeAddress();
    } catch (err) {
      setError(errorText(err));
      setIsSaving(false);
    }
  }

  return (
    <Layout
      header={<DialogHeader title={t('address.title')} onOpenChange={closeAddress} />}
      content={
        <LayoutContent>
          <VStack gap={4}>
            <Text type="supporting" color="secondary">
              {t('address.hint')}
            </Text>
            <TextInput
              label={t('common.address')}
              value={url}
              onChange={setUrl}
              placeholder="https://"
              hasAutoFocus
              hasClear
              onEnter={() => {
                if (canSubmit) void save();
              }}
              status={showFormatError ? {type: 'error', message: t('errors.fullAddress')} : undefined}
            />
            {error !== null && <Banner status="error" title={error} />}
          </VStack>
        </LayoutContent>
      }
      footer={
        <LayoutFooter>
          <HStack gap={2} hAlign="end">
            <Button label={t('common.cancel')} variant="secondary" clickAction={closeAddress} />
            <Button
              label={t('address.submit')}
              variant="primary"
              isDisabled={!canSubmit}
              isLoading={isSaving}
              clickAction={save}
            />
          </HStack>
        </LayoutFooter>
      }
    />
  );
}
