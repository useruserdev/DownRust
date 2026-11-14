// SPDX-License-Identifier: GPL-3.0-or-later
import * as stylex from '@stylexjs/stylex';
import {colorVars} from '@astryxdesign/core/theme/tokens.stylex';

export const styles = stylex.create({
  fill: {width: '100%', height: '100%'},
  surface: {
    width: '100%',
    height: '100%',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    boxSizing: 'border-box',
    cursor: 'grab',
  },
  icon: {
    width: '100%',
    height: '100%',
    transitionProperty: 'opacity',
    transitionDuration: '150ms',
    ':hover': {opacity: 0.85},
  },
  accepting: {
    outlineStyle: 'solid',
    outlineWidth: 'calc(var(--border-width) * 3)',
    outlineOffset: 'calc(var(--border-width) * -3)',
    outlineColor: colorVars['--color-accent'],
  },
});
