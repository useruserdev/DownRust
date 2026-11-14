// SPDX-License-Identifier: GPL-3.0-or-later
import {useState} from 'react';
import {Button} from '@astryxdesign/core/Button';
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {IconButton} from '@astryxdesign/core/IconButton';
import {Icon} from '@astryxdesign/core/Icon';
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {SegmentedControl, SegmentedControlItem} from '@astryxdesign/core/SegmentedControl';
import {TextInput} from '@astryxdesign/core/TextInput';
import {Trash2} from 'lucide-react';
import {api} from '../../ipc/commands';
import type {AppSettings, ProxyMode, ProxyRule, ProxyRuleAction} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';
import type {MessageKey} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

const PROXY_SCHEMES = ['http:', 'https:', 'socks5:', 'socks5h:'];

/** Returns a message when the proxy address is not usable, or null when it is valid or empty. */
function proxyAddressError(value: string): string | null {
  const trimmed = value.trim();
  if (trimmed === '') return t('errors.proxyEmpty');
  try {
    const parsed = new URL(trimmed);
    if (!PROXY_SCHEMES.includes(parsed.protocol)) return t('errors.proxyScheme');
    if (parsed.hostname === '') return t('errors.proxyHost');
    return null;
  } catch {
    return t('errors.proxyFull');
  }
}

const PROXY_TEXT: Record<ProxyMode, MessageKey> = {
  system: 'settings.network.proxySystemHint',
  direct: 'settings.network.proxyDirectHint',
  manual: 'settings.network.proxyManualHint',
};

/** Network settings: identity sent to servers, the proxy, retries and timeouts. */
export function NetworkSection({settings, update}: SectionProps) {
  const engine = settings.engine;
  const proxy = engine.proxy;
  const [password, setPassword] = useState('');
  const [passwordSaved, setPasswordSaved] = useState(false);

  const setRules = (rules: ProxyRule[]) => update({engine: {proxy: {rules}}});

  const savePassword = () => {
    api
      .setProxyPassword(password)
      .then(() => {
        setPassword('');
        setPasswordSaved(true);
      })
      .catch(err => notify('error', errorText(err)));
  };

  const removePassword = () => {
    api
      .setProxyPassword(null)
      .then(() => setPasswordSaved(false))
      .catch(err => notify('error', errorText(err)));
  };

  const addressError = proxy.mode === 'manual' ? proxyAddressError(proxy.url) : null;

  return (
    <VStack gap={5}>
      <Heading level={2}>{t('settings.section.network')}</Heading>

      <TextInput
        label={t('settings.network.userAgent')}
        description={t('settings.network.userAgentHint')}
        value={engine.userAgent}
        onChange={value => update({engine: {userAgent: value}})}
      />

      <SegmentedControl
        label={t('settings.network.proxy')}
        value={proxy.mode}
        onChange={value => update({engine: {proxy: {mode: value as ProxyMode}}})}
        size="md">
        <SegmentedControlItem value="system" label={t('settings.network.proxySystem')} />
        <SegmentedControlItem value="direct" label={t('settings.network.proxyDirect')} />
        <SegmentedControlItem value="manual" label={t('settings.network.proxyManual')} />
      </SegmentedControl>
      <Text type="supporting" color="secondary">
        {t(PROXY_TEXT[proxy.mode])}
      </Text>

      {proxy.mode === 'manual' && (
        <VStack gap={4}>
          <TextInput
            label={t('settings.network.proxyAddress')}
            placeholder="socks5h://127.0.0.1:10808"
            value={proxy.url}
            onChange={value => update({engine: {proxy: {url: value}}})}
            status={addressError === null ? undefined : {type: 'error', message: addressError}}
          />
          <TextInput
            label={t('common.userName')}
            description={t('settings.network.proxyUserHint')}
            value={proxy.username}
            onChange={value => update({engine: {proxy: {username: value}}})}
          />
          <HStack gap={2} vAlign="end">
            <StackItem size="fill">
              <TextInput
                label={t('settings.network.proxyPassword')}
                type="password"
                value={password}
                onChange={setPassword}
                autoComplete="new-password"
              />
            </StackItem>
            <Button
              label={t('common.savePassword')}
              variant="secondary"
              isDisabled={password === ''}
              clickAction={savePassword}
            />
            <Button label={t('settings.network.passwordRemove')} variant="secondary" clickAction={removePassword} />
          </HStack>
          {passwordSaved && <Text type="supporting" color="secondary">{t('common.passwordSaved')}</Text>}

          <VStack gap={2}>
            <Heading level={3}>{t('settings.network.rules')}</Heading>
            <Text type="supporting" color="secondary">
              {t('settings.network.rulesHint')}
            </Text>
            {proxy.rules.map((rule, index) => (
              <HStack key={index} gap={2} vAlign="end">
                <StackItem size="fill">
                  <TextInput
                    label={t('common.hostPattern')}
                    isLabelHidden
                    placeholder="*.example.com"
                    value={rule.host}
                    onChange={value =>
                      setRules(proxy.rules.map((item, position) => (position === index ? {...item, host: value} : item)))
                    }
                  />
                </StackItem>
                <SegmentedControl
                  label={t('settings.network.ruleAction')}
                  size="sm"
                  value={rule.action}
                  onChange={value =>
                    setRules(
                      proxy.rules.map((item, position) =>
                        position === index ? {...item, action: value as ProxyRuleAction} : item,
                      ),
                    )
                  }>
                  <SegmentedControlItem value="proxy" label={t('settings.network.ruleProxy')} />
                  <SegmentedControlItem value="direct" label={t('settings.network.ruleDirect')} />
                </SegmentedControl>
                <IconButton
                  label={t('settings.network.removeRule')}
                  tooltip={t('settings.network.removeRule')}
                  variant="ghost"
                  size="sm"
                  icon={<Icon icon={Trash2} />}
                  onClick={() => setRules(proxy.rules.filter((_, position) => position !== index))}
                />
              </HStack>
            ))}
            <HStack gap={2} hAlign="start">
              <Button
                label={t('settings.network.addRule')}
                variant="secondary"
                size="sm"
                clickAction={() => setRules([...proxy.rules, {host: '', action: 'direct'}])}
              />
            </HStack>
          </VStack>
        </VStack>
      )}

      <NumberInput
        label={t('settings.network.retries')}
        value={engine.maxRetries}
        onChange={value => update({engine: {maxRetries: value}})}
        min={0}
        max={20}
        isIntegerOnly
        hasNumberSteppers
        width={160}
      />

      <NumberInput
        label={t('settings.network.connectTimeout')}
        units={t('units.seconds')}
        value={engine.connectTimeoutSecs}
        onChange={value => update({engine: {connectTimeoutSecs: value}})}
        min={1}
        max={300}
        isIntegerOnly
        hasNumberSteppers
        width={160}
      />

      <NumberInput
        label={t('settings.network.readTimeout')}
        units={t('units.seconds')}
        value={engine.readTimeoutSecs}
        onChange={value => update({engine: {readTimeoutSecs: value}})}
        min={1}
        max={300}
        isIntegerOnly
        hasNumberSteppers
        width={160}
      />
    </VStack>
  );
}
