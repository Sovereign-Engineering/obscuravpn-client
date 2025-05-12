import { TCountryCode } from 'countries-list';
import { PinnedLocation } from '../common/appContext';
import { Exit, getContinent, getExitCountry } from './api';
import { randomChoice } from './utils';

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

export function getExitCountryFlag(exit: Exit): string {
  return getCountryFlag(getExitCountry(exit).iso2);
}

/** returns a sort comparator for Exit[] given some parameters */
export function exitsSortComparator(left: Exit, right: Exit): number {
  const leftCountry = getExitCountry(left);
  const rightCountry = getExitCountry(right);

  const leftContinent = getContinent(leftCountry);
  const rightContinent = getContinent(rightCountry);

  const leftCountryName = leftCountry.name;
  const rightCountryName = rightCountry.name;

  return continentCmp(leftContinent, rightContinent) || leftCountryName.localeCompare(rightCountryName) || left.city_name.localeCompare(right.city_name) || left.id.localeCompare(right.id);
}

const continentRankings = [
    'NA',
    'EU',
    'SA',
    'AS',
    'AF',
    'OC',
    'AN',
];

export function continentCmp(left: string, right: string): number {
    return continentRankings.indexOf(left) - continentRankings.indexOf(right);
}

export function exitLocation(exit: Exit): PinnedLocation {
  let {city_code, country_code} = exit;
  return {
    city_code,
    country_code,
    pinned_at: 0,
  };
}
