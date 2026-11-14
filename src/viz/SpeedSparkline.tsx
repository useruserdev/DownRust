// SPDX-License-Identifier: GPL-3.0-or-later
import * as stylex from '@stylexjs/stylex';
import {colorVars, sizeVars} from '@astryxdesign/core/theme/tokens.stylex';

const W = 120;
const H = 32;
/** Slots on the time axis: the newest sample is always at the right edge. */
const CAPACITY = 120;
/** Lowest vertical scale in bytes per second, so tiny speeds do not look like spikes. */
const FLOOR_BPS = 64 * 1024;

const styles = stylex.create({
  svg: {display: 'block', width: '100%', overflow: 'hidden'},
  compact: {width: 120, flexShrink: 0},
  sm: {height: sizeVars['--size-element-sm']},
  md: {height: sizeVars['--size-element-lg']},
  area: {fill: colorVars['--color-accent'], fillOpacity: 0.18},
  line: {fill: 'none', stroke: colorVars['--color-accent'], vectorEffect: 'non-scaling-stroke'},
});

/** Speed history line in the space of one progress sample per slot. */
export function SpeedSparkline({
  samples,
  size,
  width = 'fill',
}: {
  samples: number[];
  size: 'sm' | 'md';
  width?: 'fill' | 'compact';
}) {
  const count = samples.length;
  const peak = samples.reduce((max, value) => Math.max(max, value), 0);
  const yMax = Math.max(peak * 1.15, FLOOR_BPS);
  const points = samples.map((value, index) => {
    const slot = CAPACITY - count + index;
    return {x: (slot / (CAPACITY - 1)) * W, y: H - (value / yMax) * H};
  });
  const linePath = points.map((p, i) => `${i === 0 ? 'M' : 'L'}${p.x.toFixed(2)} ${p.y.toFixed(2)}`).join(' ');
  const first = points[0];
  const last = points[points.length - 1];
  const areaPath =
    first && last ? `${linePath} L${last.x.toFixed(2)} ${H} L${first.x.toFixed(2)} ${H} Z` : '';

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      role="img"
      aria-label="Download speed over the last minute"
      {...stylex.props(styles.svg, styles[size], width === 'compact' && styles.compact)}
    >
      {areaPath ? <path d={areaPath} {...stylex.props(styles.area)} /> : null}
      {linePath ? (
        <path d={linePath} strokeWidth={1.5} strokeLinejoin="round" {...stylex.props(styles.line)} />
      ) : null}
    </svg>
  );
}
