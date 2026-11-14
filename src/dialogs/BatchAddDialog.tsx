// SPDX-License-Identifier: GPL-3.0-or-later
import {useMemo, useRef, useState} from 'react';
import {open as pickFolder} from '@tauri-apps/plugin-dialog';
import {Banner} from '@astryxdesign/core/Banner';
import {Button} from '@astryxdesign/core/Button';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {FileInput} from '@astryxdesign/core/FileInput';
import {HStack, Layout, LayoutContent, LayoutFooter, StackItem, VStack} from '@astryxdesign/core/Layout';
import {List, ListItem} from '@astryxdesign/core/List';
import {ProgressBar} from '@astryxdesign/core/ProgressBar';
import {Slider} from '@astryxdesign/core/Slider';
import {Switch} from '@astryxdesign/core/Switch';
import {Text} from '@astryxdesign/core/Text';
import {TextArea} from '@astryxdesign/core/TextArea';
import {TextInput} from '@astryxdesign/core/TextInput';
import {Icon} from '@astryxdesign/core/Icon';
import {FolderOpen} from 'lucide-react';
import {api} from '../ipc/commands';
import {inTauri} from '../ipc/runtime';
import type {AddRequest} from '../ipc/types';
import {findDuplicate} from '../lib/duplicates';
import {errorText, notify} from '../lib/notify';
import {expandLinks} from '../lib/urlPattern';
import {useDownloads} from '../state/downloads';
import {useSettings} from '../state/settings';
import {useUi} from '../state/ui';
import {t} from '../i18n';

const PREVIEW_COUNT = 10;
const FALLBACK_CONNECTIONS = 8;
const LINK_LIMIT = 10000;

/** Batch add dialog: many links or patterns in one text box, added one by one. Driven by `useUi().batchDialog`. */
export function BatchAddDialog() {
  const isOpen = useUi(state => state.batchDialog.open);
  const seq = useUi(state => state.batchDialog.seq);
  const closeBatch = useUi(state => state.closeBatch);

  if (!isOpen) return null;

  return (
    <Dialog
      isOpen
      onOpenChange={open => {
        if (!open) closeBatch();
      }}
      purpose="form"
      width={640}>
      <BatchForm key={seq} />
    </Dialog>
  );
}

function BatchForm() {
  const initialText = useUi(state => state.batchDialog.text);
  const closeBatch = useUi(state => state.closeBatch);
  const upsert = useDownloads(state => state.upsert);
  const records = useDownloads(state => state.records);
  const defaults = useSettings(state => state.value?.engine);

  const [text, setText] = useState(initialText);
  const [saveDir, setSaveDir] = useState(defaults?.defaultDir ?? '');
  const [connections, setConnections] = useState(defaults?.defaultConnections ?? FALLBACK_CONNECTIONS);
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [startNow, setStartNow] = useState(true);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState({done: 0, total: 0});
  const userPickedDir = useRef(false);

  const expanded = useMemo(() => expandLinks(text, LINK_LIMIT), [text]);
  const links = expanded.links;

  const existing = useMemo(() => Object.values(records), [records]);
  const duplicateLinks = useMemo(() => {
    const set = new Set<string>();
    for (const link of links) {
      if (findDuplicate(existing, {url: link}) !== null) set.add(link);
    }
    return set;
  }, [links, existing]);

  const toAdd = skipDuplicates ? links.filter(link => !duplicateLinks.has(link)) : links;
  const preview = links.slice(0, PREVIEW_COUNT);

  function importFile(file: File | File[] | null) {
    if (!(file instanceof File)) return;
    file
      .text()
      .then(content => {
        setText(current => (current.trim() === '' ? content : `${current.trimEnd()}\n${content}`));
      })
      .catch(err => notify('error', errorText(err)));
  }

  async function browse() {
    if (!inTauri) return;
    try {
      const picked = await pickFolder({directory: true, multiple: false, defaultPath: saveDir || undefined});
      if (typeof picked === 'string') {
        userPickedDir.current = true;
        setSaveDir(picked);
      }
    } catch (err) {
      notify('error', errorText(err));
    }
  }

  async function addAll() {
    if (busy || toAdd.length === 0) return;
    setBusy(true);
    const total = toAdd.length;
    let added = 0;
    let failed = 0;
    setProgress({done: 0, total});
    for (const link of toAdd) {
      const request: AddRequest = {
        url: link,
        fileName: null,
        saveDir: saveDir || null,
        connections,
        speedLimit: null,
        start: startNow,
        referrer: null,
        headers: [],
        probe: null,
      };
      try {
        const record = await api.addDownload(request);
        upsert(record);
        added += 1;
      } catch {
        failed += 1;
      }
      setProgress({done: added + failed, total});
    }
    closeBatch();
    const summary = t('batch.added', {count: added});
    notify('info', failed > 0 ? `${summary}, ${t('batch.failedSuffix', {failed})}` : summary);
  }

  function close() {
    if (!busy) closeBatch();
  }

  return (
    <Layout
      header={<DialogHeader title={t('batch.title')} onOpenChange={close} />}
      content={
        <LayoutContent>
          <VStack gap={4}>
            <TextArea
              label={t('batch.linksLabel')}
              description={t('batch.hint')}
              value={text}
              onChange={setText}
              placeholder="https://example.com/file[001-020].jpg"
              rows={8}
              hasAutoFocus
            />

            <FileInput
              label={t('batch.importFile')}
              accept=".txt,text/plain"
              value={null}
              onChange={importFile}
              isDisabled={busy}
            />

            <HStack gap={4} vAlign="center">
              <Text type="supporting" hasTabularNumbers>{t('batch.linksCount', {count: links.length})}</Text>
              <Text type="supporting" hasTabularNumbers>{t('batch.linesIgnored', {count: expanded.invalid})}</Text>
              <Text type="supporting" hasTabularNumbers>{t('batch.alreadyInList', {count: duplicateLinks.size})}</Text>
            </HStack>

            {expanded.truncated && (
              <Banner status="warning" title={t('batch.truncated', {max: LINK_LIMIT})} />
            )}

            {preview.length > 0 && (
              <List
                header={<Text type="label">{t('batch.preview', {count: preview.length})}</Text>}
                hasDividers
                density="compact">
                {preview.map(link => (
                  <ListItem
                    key={link}
                    label={link}
                    description={duplicateLinks.has(link) ? t('batch.inList') : undefined}
                  />
                ))}
              </List>
            )}

            <HStack gap={2} vAlign="end">
              <StackItem size="fill">
                <TextInput label={t('add.saveTo')} value={saveDir} isReadOnly />
              </StackItem>
              <Button
                label={t('common.browse')}
                variant="secondary"
                icon={<Icon icon={FolderOpen} />}
                onClick={() => void browse()}
                isDisabled={!inTauri || busy}
              />
            </HStack>

            <Slider
              label={t('common.connections')}
              min={1}
              max={32}
              step={1}
              value={connections}
              onChange={setConnections}
              valueDisplay="text"
              formatValue={value => String(value)}
            />

            <Switch label={t('batch.skipDuplicates')} value={skipDuplicates} onChange={setSkipDuplicates} />
            <Switch label={t('batch.startNow')} value={startNow} onChange={setStartNow} />

            {busy && (
              <ProgressBar
                label={t('batch.adding', {done: progress.done, total: progress.total})}
                value={progress.done}
                max={Math.max(progress.total, 1)}
                hasValueLabel
              />
            )}
          </VStack>
        </LayoutContent>
      }
      footer={
        <LayoutFooter>
          <HStack gap={2} hAlign="end">
            <Button label={t('common.cancel')} variant="secondary" isDisabled={busy} clickAction={close} />
            <Button
              label={t('batch.addN', {count: toAdd.length})}
              variant="primary"
              isDisabled={toAdd.length === 0 || busy}
              clickAction={addAll}
            />
          </HStack>
        </LayoutFooter>
      }
    />
  );
}
