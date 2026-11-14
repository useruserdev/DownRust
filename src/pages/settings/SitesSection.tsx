// SPDX-License-Identifier: GPL-3.0-or-later
import {useState} from 'react';
import {Button} from '@astryxdesign/core/Button';
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {Icon} from '@astryxdesign/core/Icon';
import {IconButton} from '@astryxdesign/core/IconButton';
import {List, ListItem} from '@astryxdesign/core/List';
import {TextInput} from '@astryxdesign/core/TextInput';
import {Trash2} from 'lucide-react';
import {api} from '../../ipc/commands';
import type {AppSettings, SiteLogin} from '../../ipc/types';
import {errorText, notify} from '../../lib/notify';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

/** Sites section: user names and passwords for sites that ask for them. Passwords go to the secure store, never to settings. */
export function SitesSection({settings, update}: SectionProps) {
  const logins = settings.engine.siteLogins;
  const [host, setHost] = useState('');
  const [userName, setUserName] = useState('');
  const [password, setPassword] = useState('');
  const [changingHost, setChangingHost] = useState<string | null>(null);
  const [changedPassword, setChangedPassword] = useState('');

  const canAdd = host.trim() !== '' && userName.trim() !== '' && password !== '';

  const setLogins = (next: SiteLogin[]) => update({engine: {siteLogins: next}});

  const addLogin = () => {
    const key = host.trim().toLowerCase();
    const login: SiteLogin = {host: key, username: userName.trim()};
    const next = [...logins.filter(item => item.host !== key), login];
    setLogins(next);
    api
      .setSitePassword(key, password)
      .then(() => {
        setHost('');
        setUserName('');
        setPassword('');
      })
      .catch(err => notify('error', errorText(err)));
  };

  const removeLogin = (siteHost: string) => {
    setLogins(logins.filter(item => item.host !== siteHost));
    api.setSitePassword(siteHost, null).catch(err => notify('error', errorText(err)));
  };

  const saveChangedPassword = (siteHost: string) => {
    api
      .setSitePassword(siteHost, changedPassword)
      .then(() => {
        setChangedPassword('');
        setChangingHost(null);
        notify('info', t('common.passwordSaved'));
      })
      .catch(err => notify('error', errorText(err)));
  };

  return (
    <VStack gap={5}>
      <Heading level={2}>{t('settings.section.sites')}</Heading>
      <Text type="supporting" color="secondary">
        {t('settings.sites.intro')}
      </Text>

      {logins.length > 0 ? (
        <List density="balanced" header={t('settings.sites.savedLogins')}>
          {logins.map(login => (
            <ListItem
              key={login.host}
              label={login.host}
              description={login.username}
              endContent={
                <HStack gap={1} vAlign="center">
                  <Button
                    label={t('settings.sites.changePassword')}
                    variant="secondary"
                    size="sm"
                    clickAction={() => {
                      setChangingHost(changingHost === login.host ? null : login.host);
                      setChangedPassword('');
                    }}
                  />
                  <IconButton
                    label={t('settings.sites.removeLogin')}
                    tooltip={t('settings.sites.removeLogin')}
                    variant="ghost"
                    size="sm"
                    icon={<Icon icon={Trash2} />}
                    onClick={() => removeLogin(login.host)}
                  />
                </HStack>
              }
            />
          ))}
        </List>
      ) : (
        <Text type="supporting" color="secondary">
          {t('settings.sites.noLogins')}
        </Text>
      )}

      {changingHost !== null && (
        <HStack gap={2} vAlign="end">
          <StackItem size="fill">
            <TextInput
              label={t('settings.sites.newPasswordFor', {host: changingHost})}
              type="password"
              autoComplete="new-password"
              value={changedPassword}
              onChange={setChangedPassword}
            />
          </StackItem>
          <Button
            label={t('common.savePassword')}
            variant="primary"
            isDisabled={changedPassword === ''}
            clickAction={() => saveChangedPassword(changingHost)}
          />
        </HStack>
      )}

      <VStack gap={3}>
        <Heading level={3}>{t('settings.sites.addLogin')}</Heading>
        <TextInput
          label={t('common.hostPattern')}
          placeholder="files.example.com"
          value={host}
          onChange={setHost}
        />
        <TextInput label={t('common.userName')} value={userName} onChange={setUserName} autoComplete="username" />
        <TextInput
          label={t('common.password')}
          type="password"
          value={password}
          onChange={setPassword}
          autoComplete="new-password"
        />
        <HStack gap={2} hAlign="start">
          <Button
            label={t('settings.sites.addLogin')}
            variant="primary"
            isDisabled={!canAdd}
            clickAction={addLogin}
          />
        </HStack>
      </VStack>
    </VStack>
  );
}
