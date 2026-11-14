// SPDX-License-Identifier: GPL-3.0-or-later
// Portions adapted from Astryx templates, Copyright (c) Meta Platforms, Inc. and affiliates (MIT); see THIRD_PARTY_NOTICES.md
import {Avatar} from '@astryxdesign/core/Avatar';
import {Button} from '@astryxdesign/core/Button';
import {Divider} from '@astryxdesign/core/Divider';
import {Icon} from '@astryxdesign/core/Icon';
import {SideNav, SideNavHeading, SideNavItem, SideNavSection} from '@astryxdesign/core/SideNav';
import {Text} from '@astryxdesign/core/Text';
import {BarChart3, Plus, Settings} from 'lucide-react';
import appIconUrl from '../../assets/icon/icon.svg';
import {CATEGORY_META, CATEGORY_ORDER} from '../lib/categories';
import type {Category} from '../ipc/types';
import type {StatusFilterValue} from '../state/downloads';
import {useDownloads} from '../state/downloads';
import {useCategoryCounts, useStatusCounts} from '../state/selectors';
import {useUi} from '../state/ui';
import {t} from '../i18n';
import type {MessageKey} from '../i18n';

const STATUS_ITEMS: {value: StatusFilterValue; labelKey: MessageKey}[] = [
  {value: 'all', labelKey: 'filter.all'},
  {value: 'active', labelKey: 'filter.active'},
  {value: 'queued', labelKey: 'filter.queued'},
  {value: 'paused', labelKey: 'filter.paused'},
  {value: 'completed', labelKey: 'filter.completed'},
  {value: 'failed', labelKey: 'filter.failed'},
];

/** Side navigation: status filters with counts, category filters with counts, and the add and settings entries. */
export function SideNavigation() {
  const filter = useDownloads(state => state.filter);
  const setFilter = useDownloads(state => state.setFilter);
  const view = useUi(state => state.view);
  const setView = useUi(state => state.setView);
  const openAdd = useUi(state => state.openAdd);
  const statusCounts = useStatusCounts();
  const categoryCounts = useCategoryCounts();

  const showStatus = (value: StatusFilterValue) => {
    setFilter({kind: 'status', value});
    setView('downloads');
  };

  const showCategory = (value: Category) => {
    setFilter({kind: 'category', value});
    setView('downloads');
  };

  return (
    <SideNav
      collapsible
      resizable={{defaultWidth: 232, minWidth: 200, maxWidth: 320, autoSaveId: 'downrust-sidenav'}}
      header={
        <SideNavHeading
          heading="DownRust"
          icon={<Avatar src={appIconUrl} alt="" shape="square" size="sm" />}
        />
      }
      topContent={
        <Button
          variant="primary"
          label={t('nav.addDownload')}
          icon={<Icon icon={Plus} size="sm" />}
          width="100%"
          onClick={() => openAdd()}
        />
      }
      footer={
        <SideNavSection title={t('nav.app')} isHeaderHidden>
          <SideNavItem
            label={t('nav.statistics')}
            icon={BarChart3}
            isSelected={view === 'stats'}
            onClick={() => setView('stats')}
          />
          <SideNavItem
            label={t('nav.settings')}
            icon={Settings}
            isSelected={view === 'settings'}
            onClick={() => setView('settings')}
          />
        </SideNavSection>
      }>
      <SideNavSection title={t('nav.downloads')}>
        {STATUS_ITEMS.map(item => (
          <SideNavItem
            key={item.value}
            label={t(item.labelKey)}
            isSelected={view === 'downloads' && filter.kind === 'status' && filter.value === item.value}
            onClick={() => showStatus(item.value)}
            endContent={<Text type="supporting" hasTabularNumbers>{statusCounts[item.value]}</Text>}
          />
        ))}
      </SideNavSection>
      <Divider />
      <SideNavSection title={t('nav.categories')}>
        {CATEGORY_ORDER.map(category => (
          <SideNavItem
            key={category}
            label={t(CATEGORY_META[category].labelKey)}
            icon={CATEGORY_META[category].icon}
            isSelected={view === 'downloads' && filter.kind === 'category' && filter.value === category}
            onClick={() => showCategory(category)}
            endContent={<Text type="supporting" hasTabularNumbers>{categoryCounts[category]}</Text>}
          />
        ))}
      </SideNavSection>
    </SideNav>
  );
}
