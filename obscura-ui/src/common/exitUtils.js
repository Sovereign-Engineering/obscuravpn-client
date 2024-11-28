import { countries } from 'countries-list'

export function countryCodeToFlagEmoji(countryCode) {
    return countryCode
        .toUpperCase()
        .replace(/./g, char => {
            let codePoint = char.codePointAt(0)
                - "A".codePointAt(0)
                + "🇦".codePointAt(0);
            return String.fromCodePoint(codePoint)
        });
}

export function exitsSortComparator(connectedToExitId, lastChosenExitId, pinnedExitsList) {
    const pinnedExits = new Set(pinnedExitsList);
    return (left, right) => {
        if (left.id === connectedToExitId) return -1;
        if (right.id === connectedToExitId) return 1;

        if (left.id === lastChosenExitId) return -1;
        if (right.id === lastChosenExitId) return 1;

        const leftIsPinned = pinnedExits.has(left.id) ? 1 : 0;
        const rightIsPinned = pinnedExits.has(right.id) ? 1 : 0;

        const leftCountry = countries[left.country_code.toUpperCase()];
        const rightCountry = countries[right.country_code.toUpperCase()];

        const leftContinent = leftCountry.continent;
        const rightContinent = rightCountry.continent;

        const leftCountryName = leftCountry.name;
        const rightCountryName = rightCountry.name;

        return rightIsPinned - leftIsPinned || continentCmp(leftContinent, rightContinent) || leftCountryName.localeCompare(rightCountryName) || left.city_name.localeCompare(right.city_name) || left.id.localCompare(right.id);
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

export function continentCmp(left, right) {
    return continentRankings.indexOf(left) - continentRankings.indexOf(right);
}
