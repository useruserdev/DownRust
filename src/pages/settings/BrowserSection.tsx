// SPDX-License-Identifier: GPL-3.0-or-later
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {Button} from '@astryxdesign/core/Button';
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {Switch} from '@astryxdesign/core/Switch';
import {TextInput} from '@astryxdesign/core/TextInput';
import {api} from '../../ipc/commands';
import type {AppSettings} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import {useSettings} from '../../state/settings';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

/** Browser integration: the local port and token that the browser extension uses. */
export function BrowserSection({settings, update}: SectionProps) {
  const copyToken = async () => {
    try {
      await navigator.clipboard.writeText(settings.browserToken);
      notify('info', t('settings.browser.tokenCopied'));
    } catch (err) {
      notify('error', errorText(err));
    }
  };

  const regenerateToken = async () => {
    try {
      const saved = await api.regenerateBrowserToken();
      useSettings.getState().applyRemote(saved);
      notify('info', t('settings.browser.tokenRegenerated'));
    } catch (err) {
      notify('error', errorText(err));
    }
  };

  return (
    <VStack gap={5}>
      <Heading level={2}>{t('settings.section.browser')}</Heading>

      <Switch
        label={t('settings.browser.accept')}
        description={t('settings.browser.acceptHint')}
        value={settings.browserIntegration}
        onChange={value => update({browserIntegration: value})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <NumberInput
        label={t('settings.browser.port')}
        description={t('settings.browser.portHint')}
        value={settings.browserPort}
        onChange={value => update({browserPort: value})}
        min={1024}
        max={65535}
        isIntegerOnly
        hasNumberSteppers
        width={160}
      />

      <VStack gap={2}>
        <TextInput
          label={t('settings.browser.token')}
          description={t('settings.browser.tokenHint')}
          value={settings.browserToken}
          isReadOnly
          width="100%"
        />
        <HStack gap={2} vAlign="center">
          <Button label={t('common.copy')} variant="secondary" clickAction={copyToken} />
          <Button label={t('settings.browser.regenerate')} variant="secondary" clickAction={regenerateToken} />
        </HStack>
      </VStack>

      <VStack gap={1}>
        <Text type="label">{t('settings.browser.connectTitle')}</Text>
        <StackItem>
          <Text type="supporting" color="secondary">
            {t('settings.browser.connectHint')}
          </Text>
        </StackItem>
      </VStack>
    </VStack>
  );
}
