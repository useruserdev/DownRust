// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import type {DragEvent, MouseEvent} from 'react';
import * as stylex from '@stylexjs/stylex';
import {ContextMenu} from '@astryxdesign/core/ContextMenu';
import {Thumbnail} from '@astryxdesign/core/Thumbnail';
import {Theme} from '@astryxdesign/core/theme';
import {Menu} from '@tauri-apps/api/menu';
import {defaultThemeSlug, themes} from '../astryx-themes';
import iconUrl from '../../assets/icon/icon.svg';
import {api} from '../ipc/commands';
import {onEvent} from '../ipc/events';
import {inTauri} from '../ipc/runtime';
import type {ThemeMode} from '../ipc/types';
import {AstryxLocale} from '../i18n/astryx';
import {setLanguagePreference, t, useT} from '../i18n';
import {carriesLink, parseLinks} from './links';
import {styles} from './dropzone.styles';

function report(error: unknown): void {
  console.warn('drop zone command failed', error);
}

const openMain = () => void api.showMain().catch(report);
const hideZone = () => void api.setDropZoneVisible(false).catch(report);

/** Shows the operating system menu at the pointer. The 72 px window is too small for the in-page menu. */
async function showNativeMenu(event: MouseEvent<HTMLDivElement>): Promise<void> {
  event.preventDefault();
  try {
    const menu = await Menu.new({
      items: [
        {id: 'open-main', text: t('dropzone.open'), action: openMain},
        {id: 'hide-zone', text: t('dropzone.hide'), action: hideZone},
      ],
    });
    await menu.popup();
  } catch (error) {
    report(error);
  }
}

/** The floating drop target: accepts dropped links and opens the Add dialog in the main window. */
export function DropZone() {
  const [mode, setMode] = useState<ThemeMode>('system');
  const [accepting, setAccepting] = useState(false);
  // Re-renders the zone when the language changes, so the menu labels follow it.
  useT();

  useEffect(() => {
    let active = true;
    const apply = (settings: {theme: ThemeMode; language: 'system' | 'en' | 'ru'}) => {
      setLanguagePreference(settings.language);
      setMode(settings.theme);
    };
    api
      .getSettings()
      .then(settings => {
        if (active) apply(settings);
      })
      .catch(report);
    const unlisten = onEvent('settings-changed', settings => {
      if (active) apply(settings);
    });
    return () => {
      active = false;
      void unlisten.then(stop => stop());
    };
  }, []);

  const onDragEnter = (event: DragEvent<HTMLDivElement>) => {
    if (!carriesLink(Array.from(event.dataTransfer.types))) return;
    event.preventDefault();
    setAccepting(true);
  };

  const onDragOver = (event: DragEvent<HTMLDivElement>) => {
    if (!carriesLink(Array.from(event.dataTransfer.types))) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  };

  const onDragLeave = (event: DragEvent<HTMLDivElement>) => {
    if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
    setAccepting(false);
  };

  const onDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setAccepting(false);
    const text =
      event.dataTransfer.getData('text/uri-list') || event.dataTransfer.getData('text/plain');
    const links = parseLinks(text);
    if (links.length > 0) api.handleDroppedLinks(links).catch(report);
  };

  const surface = (
    <div
      data-tauri-drag-region="deep"
      {...stylex.props(styles.surface, accepting && styles.accepting)}
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      onDoubleClick={openMain}
      onContextMenu={inTauri ? event => void showNativeMenu(event) : undefined}>
      <Thumbnail src={iconUrl} alt="DownRust" xstyle={styles.icon} />
    </div>
  );

  return (
    <AstryxLocale>
      <Theme theme={themes[defaultThemeSlug]} mode={mode}>
        {inTauri ? (
          surface
        ) : (
          <ContextMenu
            label={t('dropzone.menu')}
            size="sm"
            triggerXstyle={styles.fill}
            items={[
              {label: t('dropzone.open'), onClick: openMain},
              {label: t('dropzone.hide'), onClick: hideZone},
            ]}>
            {surface}
          </ContextMenu>
        )}
      </Theme>
    </AstryxLocale>
  );
}
