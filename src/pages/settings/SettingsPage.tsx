// SPDX-License-Identifier: GPL-3.0-or-later
// Portions adapted from Astryx templates, Copyright (c) Meta Platforms, Inc. and affiliates (MIT); see THIRD_PARTY_NOTICES.md

import {useEffect, useState} from 'react';
import {Layout, LayoutContent, LayoutHeader, LayoutPanel, HStack, StackItem} from '@astryxdesign/core/Layout';
import {List, ListItem} from '@astryxdesign/core/List';
import {Heading} from '@astryxdesign/core/Text';
import {Spinner} from '@astryxdesign/core/Spinner';
import {useSettings} from '../../state/settings';
import type {SettingsPatch} from '../../state/settings';
import {t} from '../../i18n';
import type {MessageKey} from '../../i18n';
import {AboutSection} from './AboutSection';
import {BrowserSection} from './BrowserSection';
import {DownloadsSection} from './DownloadsSection';
import {GeneralSection} from './GeneralSection';
import {NetworkSection} from './NetworkSection';
import {ScheduleSection} from './ScheduleSection';
import {SitesSection} from './SitesSection';
import {VideoSection} from './VideoSection';

const SECTIONS = ['general', 'downloads', 'video', 'schedule', 'network', 'sites', 'browser', 'about'] as const;
type SectionName = (typeof SECTIONS)[number];

const SECTION_LABEL: Record<SectionName, MessageKey> = {
  general: 'settings.section.general',
  downloads: 'settings.section.downloads',
  video: 'video.section',
  schedule: 'settings.section.schedule',
  network: 'settings.section.network',
  sites: 'settings.section.sites',
  browser: 'settings.section.browser',
  about: 'settings.section.about',
};

/** Settings page with General, Downloads, Schedule, Network, Sites, Browser and About sections. */
export function SettingsPage() {
  const settings = useSettings(state => state.value);
  const update = useSettings(state => state.update);
  const load = useSettings(state => state.load);
  const [active, setActive] = useState<SectionName>('general');

  useEffect(() => {
    if (settings === null) void load();
  }, [settings, load]);

  const applyPatch = (patch: SettingsPatch) => update(patch);

  return (
    <Layout
      height="fill"
      contentWidth={1440}
      header={
        <LayoutHeader hasDivider>
          <Heading level={1}>{t('settings.title')}</Heading>
        </LayoutHeader>
      }
      start={
        <LayoutPanel hasDivider={false} width={232} padding={2}>
          <List density="balanced">
            {SECTIONS.map(section => (
              <ListItem
                key={section}
                label={t(SECTION_LABEL[section])}
                isSelected={active === section}
                onClick={() => setActive(section)}
              />
            ))}
          </List>
        </LayoutPanel>
      }
      content={
        <LayoutContent padding={4}>
          {settings === null ? (
            <HStack vAlign="center">
              <StackItem size="fill">
                <Spinner label={t('common.loadingSettings')} />
              </StackItem>
            </HStack>
          ) : (
            <SectionView section={active} settings={settings} update={applyPatch} />
          )}
        </LayoutContent>
      }
    />
  );
}

function SectionView({
  section,
  settings,
  update,
}: {
  section: SectionName;
  settings: NonNullable<ReturnType<typeof useSettings.getState>['value']>;
  update: (patch: SettingsPatch) => void;
}) {
  switch (section) {
    case 'general':
      return <GeneralSection settings={settings} update={update} />;
    case 'downloads':
      return <DownloadsSection settings={settings} update={update} />;
    case 'video':
      return <VideoSection settings={settings} update={update} />;
    case 'schedule':
      return <ScheduleSection settings={settings} update={update} />;
    case 'network':
      return <NetworkSection settings={settings} update={update} />;
    case 'sites':
      return <SitesSection settings={settings} update={update} />;
    case 'browser':
      return <BrowserSection settings={settings} update={update} />;
    case 'about':
      return <AboutSection />;
  }
}
