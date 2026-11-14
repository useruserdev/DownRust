// SPDX-License-Identifier: GPL-3.0-or-later
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {SegmentedControl, SegmentedControlItem} from '@astryxdesign/core/SegmentedControl';
import {Switch} from '@astryxdesign/core/Switch';
import {api} from '../../ipc/commands';
import type {AppSettings, ThemeMode} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';
import type {LanguagePreference, MessageKey} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

const TOGGLES: {key: 'launchOnStartup' | 'startMinimized' | 'closeToTray' | 'notifyOnComplete' | 'clipboardMonitor' | 'showDropZone'; labelKey: MessageKey; descriptionKey: MessageKey}[] = [
  {key: 'showDropZone', labelKey: 'settings.general.showDropZone', descriptionKey: 'settings.general.showDropZoneHint'},
  {key: 'launchOnStartup', labelKey: 'settings.general.startWithWindows', descriptionKey: 'settings.general.startWithWindowsHint'},
  {key: 'startMinimized', labelKey: 'settings.general.startMinimized', descriptionKey: 'settings.general.startMinimizedHint'},
  {key: 'closeToTray', labelKey: 'settings.general.closeToTray', descriptionKey: 'settings.general.closeToTrayHint'},
  {key: 'notifyOnComplete', labelKey: 'settings.general.notifyComplete', descriptionKey: 'settings.general.notifyCompleteHint'},
  {key: 'clipboardMonitor', labelKey: 'settings.general.clipboardMonitor', descriptionKey: 'settings.general.clipboardMonitorHint'},
];

/** General settings: theme, language and window behaviour. */
export function GeneralSection({settings, update}: SectionProps) {
  return (
    <VStack gap={5}>
      <Heading level={2}>{t('settings.section.general')}</Heading>
      <HStack gap={4} vAlign="center">
        <StackItem size="fill">
          <VStack gap={1}>
            <Text type="label">{t('settings.general.theme')}</Text>
            <Text type="supporting" color="secondary">{t('settings.general.themeHint')}</Text>
          </VStack>
        </StackItem>
        <SegmentedControl
          label={t('settings.general.theme')}
          value={settings.theme}
          onChange={value => update({theme: value as ThemeMode})}>
          <SegmentedControlItem value="system" label={t('settings.general.themeSystem')} />
          <SegmentedControlItem value="light" label={t('settings.general.themeLight')} />
          <SegmentedControlItem value="dark" label={t('settings.general.themeDark')} />
        </SegmentedControl>
      </HStack>
      <HStack gap={4} vAlign="center">
        <StackItem size="fill">
          <Text type="label">{t('settings.general.language')}</Text>
        </StackItem>
        <SegmentedControl
          label={t('settings.general.language')}
          value={settings.language}
          onChange={value => update({language: value as LanguagePreference})}>
          <SegmentedControlItem value="system" label={t('settings.general.languageSystem')} />
          <SegmentedControlItem value="en" label={t('settings.general.languageEnglish')} />
          <SegmentedControlItem value="ru" label={t('settings.general.languageRussian')} />
        </SegmentedControl>
      </HStack>
      {TOGGLES.map(toggle => (
        <Switch
          key={toggle.key}
          label={t(toggle.labelKey)}
          description={t(toggle.descriptionKey)}
          value={settings[toggle.key]}
          onChange={value => update({[toggle.key]: value})}
          labelPosition="start"
          labelSpacing="spread"
          width="100%"
        />
      ))}
      <Switch
        label={t('update.magnetLabel')}
        description={t('update.magnetHint')}
        value={settings.handleMagnetLinks}
        onChange={value => {
          update({handleMagnetLinks: value});
          api.setMagnetHandler(value).catch(err => notify('error', errorText(err)));
        }}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />
    </VStack>
  );
}
