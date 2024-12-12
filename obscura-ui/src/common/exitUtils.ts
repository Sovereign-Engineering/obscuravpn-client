import { TCountryCode } from 'countries-list';
import { Exit, getExitCountry } from './api';
import { randomChoice } from './utils';

/** returns a string containing the country flag emoji. */
function getCountryFlag(countryCode: TCountryCode): string {
  return countryCode
      .replace(/./g, char => {
          let codePoint = char.codePointAt(0)!
              - "A".codePointAt(0)!
              + "🇦".codePointAt(0)!;
          return String.fromCodePoint(codePoint)
      });
}

export function getExitCountryFlag(exit: Exit): string {
  return getCountryFlag(getExitCountry(exit).iso2);
}

/** returns a sort comparator for Exit[] given some parameters */
export function exitsSortComparator(
    connectedToExitId: string | null,
    lastChosenExitId: string | null,
    pinnedExitsList: string[],
): (l: Exit, r: Exit) => number {
    const pinnedExits = new Set(pinnedExitsList);
    return (left, right) => {
        if (left.id === connectedToExitId) return -1;
        if (right.id === connectedToExitId) return 1;

        if (left.id === lastChosenExitId) return -1;
        if (right.id === lastChosenExitId) return 1;

        const leftIsPinned = pinnedExits.has(left.id) ? 1 : 0;
        const rightIsPinned = pinnedExits.has(right.id) ? 1 : 0;

        const leftCountry = getExitCountry(left);
        const rightCountry = getExitCountry(right);

        const leftContinent = leftCountry.continent;
        const rightContinent = rightCountry.continent;

        const leftCountryName = leftCountry.name;
        const rightCountryName = rightCountry.name;

        return rightIsPinned - leftIsPinned || continentCmp(leftContinent, rightContinent) || leftCountryName.localeCompare(rightCountryName) || left.city_name.localeCompare(right.city_name) || left.id.localeCompare(right.id);
    }
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

export class CityNotFoundError extends Error {}

export function getRandomExitFromCity(exits: Exit[] | null, country_code: string, city_code: string): Exit {
  const error = new CityNotFoundError(`no exits matching country ${country_code} and city ${city_code} were found`);
  if (exits === null) throw error;
  const cityExits = exits.filter(loc => loc.country_code === country_code && loc.city_code === city_code);
  try {
    return randomChoice(cityExits);
  } catch {
    throw error;
  }
}
