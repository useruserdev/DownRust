// SPDX-License-Identifier: GPL-3.0-or-later
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {Button} from '@astryxdesign/core/Button';
import {Collapsible} from '@astryxdesign/core/Collapsible';
import {Icon} from '@astryxdesign/core/Icon';
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {Switch} from '@astryxdesign/core/Switch';
import {TextInput} from '@astryxdesign/core/TextInput';
import {FolderOpen} from 'lucide-react';
import {open as pickFolder} from '@tauri-apps/plugin-dialog';
import {CATEGORY_ORDER, categoryLabel} from '../../lib/categories';
import {errorText, notify} from '../../lib/notify';
import {inTauri} from '../../ipc/runtime';
import type {AppSettings, Category} from '../../ipc/types';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

const KIBIBYTE = 1024;
const DEFAULT_LIMIT_KB = 1024;

async function browseFolder(current: string): Promise<string | null> {
  if (!inTauri) return null;
  try {
    const picked = await pickFolder({directory: true, multiple: false, defaultPath: current || undefined});
    return typeof picked === 'string' ? picked : null;
  } catch (err) {
    notify('error', errorText(err));
    return null;
  }
}

/** Download settings: folders, connection limits and speed limit. */
export function DownloadsSection({settings, update}: SectionProps) {
  const engine = settings.engine;
  const limitKb = engine.globalSpeedLimit === null ? null : Math.round(engine.globalSpeedLimit / KIBIBYTE);

  return (
    <VStack gap={5}>
      <Heading level={2}>{t('settings.section.downloads')}</Heading>

      <HStack gap={2} vAlign="end">
        <StackItem size="fill">
          <TextInput label={t('settings.downloads.defaultDir')} value={engine.defaultDir} isReadOnly />
        </StackItem>
        <Button
          label={t('common.browse')}
          variant="secondary"
          icon={<Icon icon={FolderOpen} />}
          isDisabled={!inTauri}
          clickAction={async () => {
            const dir = await browseFolder(engine.defaultDir);
            if (dir) update({engine: {defaultDir: dir}});
          }}
        />
      </HStack>

      <Switch
        label={t('settings.downloads.sortCategories')}
        description={t('settings.downloads.sortCategoriesHint')}
        value={engine.autoSortByCategory}
        onChange={value => update({engine: {autoSortByCategory: value}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      {engine.autoSortByCategory &&
        CATEGORY_ORDER.map((category: Category) => (
          <HStack key={category} gap={2} vAlign="end">
            <StackItem size="fill">
              <TextInput
                label={t('settings.downloads.categoryFolder', {category: categoryLabel(category)})}
                value={engine.categoryDirs[category] ?? ''}
                placeholder={t('settings.downloads.useDefaultFolder')}
                isReadOnly
              />
            </StackItem>
            <Button
              label={t('common.browse')}
              variant="secondary"
              icon={<Icon icon={FolderOpen} />}
              isDisabled={!inTauri}
              clickAction={async () => {
                const dir = await browseFolder(engine.categoryDirs[category] ?? engine.defaultDir);
                if (dir) update({engine: {categoryDirs: {[category]: dir}}});
              }}
            />
          </HStack>
        ))}

      <NumberInput
        label={t('settings.downloads.atOnce')}
        description={t('settings.downloads.atOnceHint')}
        value={engine.maxConcurrentDownloads}
        onChange={value => update({engine: {maxConcurrentDownloads: value}})}
        min={1}
        max={10}
        isIntegerOnly
        hasNumberSteppers
        width={160}
      />

      <NumberInput
        label={t('settings.downloads.connectionsPerDownload')}
        description={t('settings.downloads.connectionsPerDownloadHint')}
        value={engine.defaultConnections}
        onChange={value => update({engine: {defaultConnections: value}})}
        min={1}
        max={32}
        isIntegerOnly
        hasNumberSteppers
        width={160}
      />

      <Switch
        label={t('settings.downloads.limitTotal')}
        description={t('settings.downloads.limitTotalHint')}
        value={engine.globalSpeedLimit !== null}
        onChange={value =>
          update({engine: {globalSpeedLimit: value ? DEFAULT_LIMIT_KB * KIBIBYTE : null}})
        }
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />
      {engine.globalSpeedLimit !== null && (
        <NumberInput
          label={t('settings.downloads.totalLimit')}
          units={t('units.kbps')}
          value={limitKb ?? DEFAULT_LIMIT_KB}
          onChange={value => update({engine: {globalSpeedLimit: value * KIBIBYTE}})}
          min={1}
          isIntegerOnly
          width={160}
        />
      )}

      <Switch
        label={t('settings.downloads.resumeStartup')}
        description={t('settings.downloads.resumeStartupHint')}
        value={engine.resumeOnStartup}
        onChange={value => update({engine: {resumeOnStartup: value}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />
      <Switch
        label={t('settings.downloads.preallocate')}
        description={t('settings.downloads.preallocateHint')}
        value={engine.preallocate}
        onChange={value => update({engine: {preallocate: value}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <Switch
        label={t('settings.downloads.scan')}
        description={t('settings.downloads.scanHint')}
        value={engine.scan.enabled}
        onChange={value => update({engine: {scan: {enabled: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />
      <Collapsible trigger={t('settings.downloads.scanAdvanced')} defaultIsOpen={false}>
        <VStack gap={3}>
          <TextInput
            label={t('settings.downloads.scanProgram')}
            placeholder={t('settings.downloads.scanProgramPlaceholder')}
            value={engine.scan.program}
            onChange={value => update({engine: {scan: {program: value}}})}
          />
          <TextInput
            label={t('settings.downloads.scanArgs')}
            description={t('settings.downloads.scanArgsHint')}
            value={engine.scan.args}
            onChange={value => update({engine: {scan: {args: value}}})}
          />
        </VStack>
      </Collapsible>

      <Switch
        label={t('mirrors.autoChecksum')}
        description={t('mirrors.autoChecksumHint')}
        value={engine.autoChecksum}
        onChange={value => update({engine: {autoChecksum: value}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <Text type="supporting" color="secondary">
        {t('settings.downloads.changesNote')}
      </Text>
    </VStack>
  );
}
