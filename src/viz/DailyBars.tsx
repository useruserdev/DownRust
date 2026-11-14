// SPDX-License-Identifier: GPL-3.0-or-later
import * as stylex from '@stylexjs/stylex';
import {colorVars} from '@astryxdesign/core/theme/tokens.stylex';
import {humanBytes} from '../lib/format';

/** Geometry is in viewBox units. The chart keeps its aspect ratio at any width. */
const W = 600;
const H = 120;
const BAR_SHARE = 0.72;

const styles = stylex.create({
  svg: {display: 'block', width: '100%', height: 'auto', overflow: 'visible'},
  baseline: {stroke: colorVars['--color-border'], fill: 'none', vectorEffect: 'non-scaling-stroke'},
  bar: {fill: colorVars['--color-accent'], fillOpacity: 0.45},
  today: {fill: colorVars['--color-accent']},
});

export interface DailyBarDatum {
  day: string;
  bytes: number;
  files: number;
}

/** Bars for one value per day. The last bar is today and is drawn at full strength. Hover shows the value. */
export function DailyBars({days, label}: {days: DailyBarDatum[]; label: string}) {
  const count = Math.max(days.length, 1);
  const slot = W / count;
  const barWidth = slot * BAR_SHARE;
  const peak = days.reduce((max, day) => Math.max(max, day.bytes), 0);
  const scale = peak > 0 ? H / peak : 0;

  return (
    <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={label} {...stylex.props(styles.svg)}>
      {days.map((datum, index) => {
        const height = datum.bytes * scale;
        const x = index * slot + (slot - barWidth) / 2;
        const isToday = index === days.length - 1;
        return (
          <rect
            key={datum.day}
            x={x.toFixed(2)}
            y={(H - height).toFixed(2)}
            width={barWidth.toFixed(2)}
            height={height.toFixed(2)}
            {...stylex.props(isToday ? styles.today : styles.bar)}
          >
            <title>{`${datum.day}: ${humanBytes(datum.bytes)}, ${datum.files} files`}</title>
          </rect>
        );
      })}
      <line x1={0} y1={H} x2={W} y2={H} strokeWidth={1} {...stylex.props(styles.baseline)} />
    </svg>
  );
}
