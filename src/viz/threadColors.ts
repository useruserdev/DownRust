// SPDX-License-Identifier: GPL-3.0-or-later
import * as stylex from '@stylexjs/stylex';
import {dataVars} from '@astryxdesign/core/theme/dataTokens.stylex';

const threadStyles = stylex.create({
  orange: {fill: dataVars['--color-data-categorical-orange']},
  blue: {fill: dataVars['--color-data-categorical-blue']},
  teal: {fill: dataVars['--color-data-categorical-teal']},
  purple: {fill: dataVars['--color-data-categorical-purple']},
  pink: {fill: dataVars['--color-data-categorical-pink']},
  green: {fill: dataVars['--color-data-categorical-green']},
  cyan: {fill: dataVars['--color-data-categorical-cyan']},
  indigo: {fill: dataVars['--color-data-categorical-indigo']},
  red: {fill: dataVars['--color-data-categorical-red']},
  brown: {fill: dataVars['--color-data-categorical-brown']},
});

/** Ten thread colours in a fixed order. Segment i uses entry i modulo 10. */
export const threadPalette = [
  threadStyles.orange,
  threadStyles.blue,
  threadStyles.teal,
  threadStyles.purple,
  threadStyles.pink,
  threadStyles.green,
  threadStyles.cyan,
  threadStyles.indigo,
  threadStyles.red,
  threadStyles.brown,
] as const;

/** Returns the thread colour style for a segment index. */
export function threadStyleFor(index: number) {
  return threadPalette[index % threadPalette.length];
}
