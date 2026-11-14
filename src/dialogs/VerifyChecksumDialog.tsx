// SPDX-License-Identifier: GPL-3.0-or-later
import {useState} from 'react';
import {Banner} from '@astryxdesign/core/Banner';
import {Button} from '@astryxdesign/core/Button';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {HStack, Layout, LayoutContent, LayoutFooter, VStack} from '@astryxdesign/core/Layout';
import {Text} from '@astryxdesign/core/Text';
import {TextInput} from '@astryxdesign/core/TextInput';
import {api} from '../ipc/commands';
import type {ChecksumReport, DownloadId} from '../ipc/types';
import {errorText} from '../lib/notify';
import {useDownloads} from '../state/downloads';
import {useUi} from '../state/ui';
import {t} from '../i18n';

const SHA256_PATTERN = /^[0-9a-fA-F]{64}$/;

/** Verify dialog: compares a pasted SHA-256 hash with the finished file. Driven by `useUi().verifyDialog`. */
export function VerifyChecksumDialog() {
  const {open, id} = useUi(state => state.verifyDialog);
  const closeVerify = useUi(state => state.closeVerify);
  const hasRecord = useDownloads(state => (id === null ? false : id in state.records));

  if (!open || id === null || !hasRecord) return null;

  return (
    <Dialog isOpen onOpenChange={isOpen => !isOpen && closeVerify()} purpose="form" width={520}>
      <VerifyForm id={id} />
    </Dialog>
  );
}

function resultTitle(report: ChecksumReport): string {
  return report.matches ? t('verify.matches') : t('verify.mismatch', {actual: report.actual});
}

function VerifyForm({id}: {id: DownloadId}) {
  const closeVerify = useUi(state => state.closeVerify);
  const [hash, setHash] = useState('');
  const [report, setReport] = useState<ChecksumReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isChecking, setIsChecking] = useState(false);

  const trimmed = hash.trim();
  const isValid = SHA256_PATTERN.test(trimmed);
  const showFormatError = trimmed !== '' && !isValid;

  async function verify() {
    setIsChecking(true);
    setError(null);
    setReport(null);
    try {
      setReport(await api.verifyChecksum(id, trimmed));
    } catch (err) {
      setError(errorText(err));
    } finally {
      setIsChecking(false);
    }
  }

  return (
    <Layout
      header={<DialogHeader title={t('verify.title')} onOpenChange={closeVerify} />}
      content={
        <LayoutContent>
          <VStack gap={4}>
            <Text type="supporting" color="secondary">
              {t('verify.hint')}
            </Text>
            <TextInput
              label={t('verify.label')}
              value={hash}
              onChange={setHash}
              hasAutoFocus
              hasClear
              onEnter={() => {
                if (isValid && !isChecking) void verify();
              }}
              status={showFormatError ? {type: 'error', message: t('errors.sha256')} : undefined}
            />
            {report !== null && <Banner status={report.matches ? 'success' : 'error'} title={resultTitle(report)} />}
            {error !== null && <Banner status="error" title={error} />}
          </VStack>
        </LayoutContent>
      }
      footer={
        <LayoutFooter>
          <HStack gap={2} hAlign="end">
            <Button label={t('common.close')} variant="secondary" clickAction={closeVerify} />
            <Button
              label={t('common.verify')}
              variant="primary"
              isDisabled={!isValid || isChecking}
              isLoading={isChecking}
              clickAction={verify}
            />
          </HStack>
        </LayoutFooter>
      }
    />
  );
}
