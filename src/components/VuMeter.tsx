import { memo, useId } from "react";
import { SCALE_MARKS, positionToAngle, vuToPosition } from "../lib/vu";

/** Çizim ölçüleri (SVG birimi): ibrenin dönme noktası ölçek yüzünün altındadır. */
const WIDTH = 240;
const HEIGHT = 140;
const CX = WIDTH / 2;
const CY = 172;
const SCALE_RADIUS = 130;
const LABEL_RADIUS = 146;
const NEEDLE_RADIUS = 140;

/** Dönme noktasına göre açı (derece) ve yarıçaptan nokta. */
function polar(angle: number, radius: number): [number, number] {
  const rad = (angle * Math.PI) / 180;
  return [CX + radius * Math.sin(rad), CY - radius * Math.cos(rad)];
}

function arc(fromVu: number, toVu: number, radius: number): string {
  const [x1, y1] = polar(positionToAngle(vuToPosition(fromVu)), radius);
  const [x2, y2] = polar(positionToAngle(vuToPosition(toVu)), radius);
  return `M${x1.toFixed(2)},${y1.toFixed(2)}A${radius},${radius} 0 0,1 ${x2.toFixed(2)},${y2.toFixed(2)}`;
}

/** Ölçek yüzü: ibre dışında hiç değişmediği için bir kez çizilir. */
const Scale = memo(function Scale() {
  return (
    <g className="vu-meter__scale">
      <path className="vu-meter__arc" d={arc(-20, 0, SCALE_RADIUS)} />
      <path className="vu-meter__arc vu-meter__arc--red" d={arc(0, 3, SCALE_RADIUS)} />
      {SCALE_MARKS.map((vu) => {
        const angle = positionToAngle(vuToPosition(vu));
        const [x1, y1] = polar(angle, SCALE_RADIUS);
        const [x2, y2] = polar(angle, SCALE_RADIUS + (vu === 0 ? 12 : 8));
        const [lx, ly] = polar(angle, LABEL_RADIUS + (vu === 0 ? 4 : 0));
        const red = vu > 0;
        return (
          <g key={vu} className={red ? "vu-meter__mark vu-meter__mark--red" : "vu-meter__mark"}>
            <line x1={x1} y1={y1} x2={x2} y2={y2} />
            <text x={lx} y={ly} textAnchor="middle" dominantBaseline="central">
              {Math.abs(vu)}
            </text>
          </g>
        );
      })}
      <text className="vu-meter__sign" x={18} y={44} textAnchor="middle">
        −
      </text>
      <text
        className="vu-meter__sign vu-meter__sign--red"
        x={WIDTH - 18}
        y={44}
        textAnchor="middle"
      >
        +
      </text>
      <text className="vu-meter__unit" x={CX} y={96} textAnchor="middle">
        VU
      </text>
    </g>
  );
});

export interface VuMeterProps {
  channel: "L" | "R";
  /** İbre konumu (0 = sol uç, 1 = +3 VU). */
  position: number;
  /** Kırmızı bölge ışığı yanıyor mu? */
  lamp: boolean;
}

/**
 * Özgün tasarım analog VU ölçer: arkadan aydınlatılmış kehribar yüz, kırmızı
 * bölge, gölgeli ibre ve kırmızı bölge ışığı. Renkler `:root` değişkenlerinden gelir.
 */
export const VuMeter = memo(function VuMeter({ channel, position, lamp }: VuMeterProps) {
  const id = useId();
  const face = `${id}-face`;
  const clip = `${id}-clip`;
  const angle = positionToAngle(position);
  const [nx, ny] = polar(angle, NEEDLE_RADIUS);
  const needle = `M${CX},${CY}L${nx.toFixed(2)},${ny.toFixed(2)}`;

  return (
    <svg
      className="vu-meter"
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      role="img"
      aria-label={channel === "L" ? "Sol kanal VU ölçer" : "Sağ kanal VU ölçer"}
    >
      <defs>
        <radialGradient id={face} cx="50%" cy="85%" r="80%">
          <stop offset="0%" className="vu-meter__face-hot" />
          <stop offset="55%" className="vu-meter__face-mid" />
          <stop offset="100%" className="vu-meter__face-edge" />
        </radialGradient>
        <clipPath id={clip}>
          <rect x={0} y={0} width={WIDTH} height={HEIGHT} rx={8} />
        </clipPath>
      </defs>
      <g clipPath={`url(#${clip})`}>
        <rect
          className="vu-meter__face"
          x={0}
          y={0}
          width={WIDTH}
          height={HEIGHT}
          fill={`url(#${face})`}
        />
        <Scale />
        <text className="vu-meter__channel" x={16} y={HEIGHT - 32}>
          {channel}
        </text>
        <path className="vu-meter__needle-shadow" d={needle} transform="translate(3 4)" />
        <path className="vu-meter__needle" d={needle} />
        <path
          className="vu-meter__hood"
          d={`M0,${HEIGHT}L0,${HEIGHT - 22}Q${CX},${HEIGHT - 44} ${WIDTH},${HEIGHT - 22}L${WIDTH},${HEIGHT}Z`}
        />
        <circle
          className={lamp ? "vu-meter__lamp is-on" : "vu-meter__lamp"}
          cx={WIDTH - 22}
          cy={HEIGHT - 12}
          r={4}
        />
        <text className="vu-meter__lamp-label" x={WIDTH - 32} y={HEIGHT - 12} textAnchor="end">
          PEAK
        </text>
        <rect className="vu-meter__glass" x={0} y={0} width={WIDTH} height={HEIGHT} />
      </g>
      <rect
        className="vu-meter__bezel"
        x={0.5}
        y={0.5}
        width={WIDTH - 1}
        height={HEIGHT - 1}
        rx={8}
      />
    </svg>
  );
});
