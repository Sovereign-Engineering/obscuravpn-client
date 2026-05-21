import * as Flags from 'country-flag-icons/react/3x2';
import classes from './CountryFlag.module.css';

// On Windows, the system font only supports unicode emojis;
// Flag emojis render as as ISO 3166-1 alpha-2 as well as on Chromium browsers.
// Firefox uses a default font that supports emojis.
// The solution is to show the flag SVGs on Windows or use a font with emojis supported out of the box.
export default function CountryFlag_Windows({ countryCode, h, fallback }: { countryCode: string, h: number, fallback: string }) {
  const FlagComponent = Flags[countryCode.toUpperCase() as keyof typeof Flags];
  return FlagComponent ? <FlagComponent className={classes.flag} height={h} /> : <>{fallback}</>;
}
