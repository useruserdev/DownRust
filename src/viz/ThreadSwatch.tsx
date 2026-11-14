// SPDX-License-Identifier: GPL-3.0-or-later
import * as stylex from '@stylexjs/stylex';
import {threadStyleFor} from './threadColors';

/** Small dot in the colour of one connection thread. */
export function ThreadSwatch({index}: {index: number}) {
  return (
    <svg viewBox="0 0 10 10" width={10} height={10} aria-hidden="true" focusable="false">
      <circle cx={5} cy={5} r={5} {...stylex.props(threadStyleFor(index))} />
    </svg>
  );
}
