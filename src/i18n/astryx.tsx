// SPDX-License-Identifier: GPL-3.0-or-later
import type {ReactNode} from 'react';
import {InternationalizationProvider} from '@astryxdesign/core/i18n';
import ruAstryx from '@astryxdesign/core/locales/ru-RU.generated.js';
import {useT} from './index';

/** Gives the Astryx components (dialog close buttons, pagination, time inputs and so on) the screen language. */
export function AstryxLocale({children}: {children: ReactNode}) {
  const {lang} = useT();
  return (
    <InternationalizationProvider locale={lang === 'ru' ? 'ru-RU' : 'en'} messages={{'ru-RU': ruAstryx}}>
      {children}
    </InternationalizationProvider>
  );
}
