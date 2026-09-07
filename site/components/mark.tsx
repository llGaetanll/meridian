/// The globe, drawn on the same 200x200 grid as `sketches/brand/meridian-mark.svg`.
///
/// Strokes are `currentColor`, so the mark takes the colour of whatever it sits
/// in rather than carrying a violet of its own.
export function Mark({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 200 200"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <defs>
        <clipPath id="meridian-globe-clip">
          <circle cx="100" cy="100" r="93" />
        </clipPath>
      </defs>
      <g clipPath="url(#meridian-globe-clip)" strokeWidth="14">
        <path d="M 181.753,73.308 C 171.596,42.2,126.76,28.932,81.61,43.674 C 36.459,58.415,8.09,95.584,18.247,126.692" />
        <path d="M 18.247,126.692 C 8.09,95.584,36.459,58.415,81.61,43.674 C 126.76,28.932,171.596,42.2,181.753,73.308" />
        <path d="M 118.172,15.942 C 108.574,13.867,92.658,49.819,82.622,96.243 C 72.586,142.667,72.23,181.983,81.828,184.058" />
        <path d="M 81.828,184.058 C 72.23,181.983,72.586,142.667,82.622,96.243 C 92.658,49.819,108.574,13.867,118.172,15.942" />
        <path d="M 14,100 C 14,133.585,52.504,160.811,100,160.811 C 147.496,160.811,186,133.585,186,100" />
        <path d="M 100,14 C 66.415,14,39.189,33.252,39.189,57 C 39.189,80.748,66.415,100,100,100 C 133.585,100,160.811,80.748,160.811,57 C 160.811,33.252,133.585,14,100,14" />
        <path d="M 147.342,171.797 C 166.464,159.188,160.77,116.822,134.624,77.17 C 108.477,37.517,71.78,15.595,52.658,28.203" />
        <path d="M 52.658,28.203 C 71.78,15.595,108.477,37.517,134.624,77.17 C 160.77,116.822,166.464,159.188,147.342,171.797" />
      </g>
      <circle cx="100" cy="100" r="86" strokeWidth="14" />
    </svg>
  );
}
