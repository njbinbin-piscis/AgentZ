import type { Activity } from "./activity";

/** Stable skin/hair variation per agent so desks are distinguishable at a glance. */
function hash(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h);
}

const SKIN = ["#f6d7b8", "#eec29a", "#d9a37a", "#b97d55", "#8d5a3b"];
const HAIR = ["#2b2b2b", "#5a3825", "#a0522d", "#d9b36c", "#6b6b8a", "#c0392b"];

export interface AvatarProps {
  seed: string;
  color: string;
  icon?: string;
  activity: Activity;
  boss?: boolean;
}

/** Flat vector office worker sitting at a desk with a monitor. */
export default function Avatar({ seed, color, icon, activity, boss }: AvatarProps) {
  const h = hash(seed);
  const skin = SKIN[h % SKIN.length];
  const hair = HAIR[(h >> 3) % HAIR.length];
  const shirt = color || "#7c6af7";
  return (
    <svg className={`botz-avatar act-${activity}`} viewBox="0 0 120 110" aria-hidden="true">
      {/* chair back */}
      <rect x="38" y="44" width="44" height="40" rx="8" fill="#3b3f4a" />
      {/* body */}
      <g className="botz-avatar-body">
        <path d="M40 92 Q40 62 60 62 Q80 62 80 92 Z" fill={shirt} />
        {boss && <path d="M57 64 L60 80 L63 64 Z" fill="#c0392b" />}
        <circle cx="60" cy="44" r="15" fill={skin} />
        <path d="M45 42 Q46 26 60 27 Q75 27 75 42 Q70 34 60 34 Q50 34 45 42 Z" fill={hair} />
        <g className="botz-avatar-eyes">
          <circle cx="54.5" cy="45" r="1.8" fill="#222" />
          <circle cx="65.5" cy="45" r="1.8" fill="#222" />
        </g>
        {activity === "error" ? (
          <path d="M55 53 Q60 49 65 53" stroke="#222" strokeWidth="1.6" fill="none" />
        ) : (
          <path d="M55 51 Q60 55 65 51" stroke="#222" strokeWidth="1.6" fill="none" />
        )}
        {activity === "reading" && <rect className="botz-avatar-book" x="48" y="66" width="24" height="14" rx="2" fill="#f4f1e8" stroke="#888" />}
        {activity === "idle" && (
          <g className="botz-avatar-cup">
            <rect x="74" y="68" width="9" height="11" rx="2" fill="#fff" stroke="#999" />
            <path className="botz-steam" d="M77 64 q2 -3 0 -6 M80 64 q2 -3 0 -6" stroke="#aaa" fill="none" />
          </g>
        )}
        <g className="botz-avatar-hands">
          <circle cx="47" cy="84" r="4" fill={skin} />
          <circle cx="73" cy="84" r="4" fill={skin} />
        </g>
      </g>
      {/* desk + monitor */}
      <rect x="8" y="86" width="104" height="10" rx="3" fill="#8b6b4a" />
      <rect x="14" y="96" width="6" height="14" fill="#6d5238" />
      <rect x="100" y="96" width="6" height="14" fill="#6d5238" />
      <g className="botz-monitor">
        <rect x="84" y="62" width="28" height="20" rx="2" fill="#23262e" />
        <rect className="botz-monitor-screen" x="86" y="64" width="24" height="16" rx="1" fill={activity === "idle" ? "#39404d" : "#4fc3f7"} />
        <rect x="96" y="82" width="4" height="4" fill="#23262e" />
      </g>
      {icon && (
        <text x="18" y="80" fontSize="16" textAnchor="middle">
          {icon}
        </text>
      )}
    </svg>
  );
}
