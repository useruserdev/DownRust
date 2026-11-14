// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {Banner} from '@astryxdesign/core/Banner';
import {Card} from '@astryxdesign/core/Card';
import {EmptyState} from '@astryxdesign/core/EmptyState';
import {Grid} from '@astryxdesign/core/Grid';
import {HStack, Layout, LayoutContent, LayoutHeader, StackItem, VStack} from '@astryxdesign/core/Layout';
import {List, ListItem} from '@astryxdesign/core/List';
import {ProgressBar} from '@astryxdesign/core/ProgressBar';
import {Spinner} from '@astryxdesign/core/Spinner';
import {Heading, Text} from '@astryxdesign/core/Text';
import {CategoryIcon, categoryLabel} from '../../lib/categories';
import {humanBytes} from '../../lib/format';
import {errorText} from '../../lib/notify';
import {fetchStats} from '../../ipc/stats';
import type {StatsDay, StatsSnapshot} from '../../ipc/stats';
import {DailyBars} from '../../viz/DailyBars';
import {t} from '../../i18n';

const REFRESH_MS = 30_000;

interface Totals {
  bytes: number;
  files: number;
}

function sumDays(days: StatsDay[]): Totals {
  return days.reduce((sum, day) => ({bytes: sum.bytes + day.bytes, files: sum.files + day.files}), {bytes: 0, files: 0});
}

/** Statistics page: four totals, a 30-day bar chart and the share of each category. */
export function StatsPage() {
  const [snapshot, setSnapshot] = useState<StatsSnapshot | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const load = () => {
      fetchStats()
        .then(next => {
          if (!active) return;
          setSnapshot(next);
          setFailure(null);
        })
        .catch(err => {
          if (active) setFailure(errorText(err));
        });
    };
    load();
    const timer = window.setInterval(load, REFRESH_MS);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  return (
    <Layout
      height="fill"
      header={
        <LayoutHeader hasDivider>
          <Heading level={1}>{t('stats.title')}</Heading>
        </LayoutHeader>
      }
      content={
        <LayoutContent padding={4}>
          {snapshot === null ? (
            failure === null ? (
              <Spinner label={t('common.loadingStats')} />
            ) : (
              <Banner status="error" title={t('stats.failed')} description={failure} />
            )
          ) : snapshot.allTime.files === 0 ? (
            <EmptyState headingLevel={2} title={t('stats.emptyTitle')} description={t('stats.emptyHint')} />
          ) : (
            <StatsContent snapshot={snapshot} />
          )}
        </LayoutContent>
      }
    />
  );
}

function StatsContent({snapshot}: {snapshot: StatsSnapshot}) {
  const days = snapshot.days;
  const today = days[days.length - 1];
  const tiles: {label: string; total: Totals}[] = [
    {label: t('stats.today'), total: today ? {bytes: today.bytes, files: today.files} : {bytes: 0, files: 0}},
    {label: t('stats.last7'), total: sumDays(days.slice(-7))},
    {label: t('stats.last30'), total: sumDays(days)},
    {label: t('stats.allTime'), total: snapshot.allTime},
  ];

  return (
    <VStack gap={4}>
      <Grid columns={{minWidth: 200, max: 4}} gap={3}>
        {tiles.map(tile => (
          <Card key={tile.label} variant="muted" padding={3}>
            <VStack gap={1}>
              <Text type="supporting">{tile.label}</Text>
              <Text type="large" weight="semibold" hasTabularNumbers>
                {humanBytes(tile.total.bytes)}
              </Text>
              <Text type="supporting" hasTabularNumbers>{t('stats.files', {count: tile.total.files})}</Text>
            </VStack>
          </Card>
        ))}
      </Grid>

      <Card padding={4}>
        <VStack gap={3}>
          <Heading level={2}>{t('stats.last30')}</Heading>
          <DailyBars days={days} label={t('stats.chart')} />
          <HStack gap={2} vAlign="center">
            <StackItem size="fill">
              <Text type="supporting" hasTabularNumbers>{days[0]?.day ?? ''}</Text>
            </StackItem>
            <Text type="supporting">{t('stats.today')}</Text>
          </HStack>
        </VStack>
      </Card>

      <VStack gap={2}>
        <Heading level={2}>{t('stats.byCategory')}</Heading>
        <List hasDividers density="balanced">
          {snapshot.byCategory.map(entry => {
            const label = categoryLabel(entry.category);
            const share = snapshot.allTime.bytes > 0 ? (entry.bytes / snapshot.allTime.bytes) * 100 : 0;
            return (
              <ListItem
                key={entry.category}
                startContent={<CategoryIcon category={entry.category} size="sm" />}
                label={label}
                description={
                  <VStack gap={1}>
                    <Text type="supporting" hasTabularNumbers>
                      {`${humanBytes(entry.bytes)} · ${t('stats.files', {count: entry.files})}`}
                    </Text>
                    <ProgressBar label={t('stats.share', {label})} isLabelHidden value={share} max={100} />
                  </VStack>
                }
                endContent={<Text type="supporting" hasTabularNumbers>{`${Math.round(share)}%`}</Text>}
              />
            );
          })}
        </List>
      </VStack>
    </VStack>
  );
}
