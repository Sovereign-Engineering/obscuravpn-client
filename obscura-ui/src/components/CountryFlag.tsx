import { lazy, Suspense } from 'react';

// Both sides must be literals so Vite constant-folds this and drops the ~235 kB SVG chunk on non-Windows builds.
const CountryFlagWindows = import.meta.env.OBS_WEB_PLATFORM === 'windows'
  ? lazy(() => import('./CountryFlag_Windows'))
  : null;

export function Flag({ countryCode, h = 13 }: { countryCode: string, h?: number }) {
  const emoji = getCountryFlag(countryCode);
  if (CountryFlagWindows === null) return emoji;
  return (
    <Suspense fallback={emoji}>
      <CountryFlagWindows countryCode={countryCode} h={h} fallback={emoji} />
    </Suspense>
  );
}

/** returns a string containing the country flag emoji. */
export function getCountryFlag(countryCode: string): string {
  return countryCode
      .replace(/[A-Za-z]/g, char => {
          let codePoint = char.toUpperCase().codePointAt(0)!
              - "A".codePointAt(0)!
              + "🇦".codePointAt(0)!;
          return String.fromCodePoint(codePoint)
      });
}
