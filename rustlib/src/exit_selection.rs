use std::{
    collections::{HashMap, HashSet},
    num::Saturating,
};

use obscuravpn_api::types::{CityCode, CountryCode, OneExit, OneRelay, RelayPreferredExit};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ExitSelector {
    Any {},
    Exit {
        id: String,
    },
    Country {
        country_code: CountryCode,
    },
    City {
        #[serde(flatten)]
        city_code: CityCode,
    },
}

impl ExitSelector {
    pub fn matches(&self, candidate: &OneExit) -> bool {
        match self {
            ExitSelector::Any {} => true,
            ExitSelector::Exit { id } => candidate.id == *id,
            ExitSelector::Country { country_code } => candidate.city_code.country_code == *country_code,
            ExitSelector::City { city_code } => candidate.city_code == *city_code,
        }
    }
}

impl Default for ExitSelector {
    fn default() -> Self {
        ExitSelector::Any {}
    }
}

#[derive(Debug, Default)]
pub struct ExitSelectionState {
    selected_exit_ids: HashSet<String>,
    selected_datacenters: HashMap<u32, Saturating<u8>>,
    selected_cities: HashMap<CityCode, Saturating<u8>>,
    selected_countries: HashMap<CountryCode, Saturating<u8>>,
}

impl ExitSelectionState {
    pub fn select_next_exit<'a>(&mut self, selector: &ExitSelector, exits: &'a [OneExit], relay: &OneRelay) -> Option<&'a OneExit> {
        let selected = exits
            .iter()
            .filter(|candidate| selector.matches(candidate))
            .filter(|candidate| !self.exclude(candidate))
            .max_by_key(|candidate| Self::rank(candidate, &relay.city_code, &relay.preferred_exits));

        if let Some(selected) = selected {
            self.selected_exit_ids.insert(selected.id.clone());
            *self.selected_datacenters.entry(selected.datacenter_id).or_insert(Saturating(0)) += 1;
            *self.selected_cities.entry(selected.city_code.clone()).or_insert(Saturating(0)) += 1;
            *self
                .selected_countries
                .entry(selected.city_code.country_code.clone())
                .or_insert(Saturating(0)) += 1;
        } else {
            tracing::warn!("no exits left to select, clearing adaptive filters");
            *self = Self::default();
        }

        selected
    }

    fn rank(candidate: &OneExit, relay_city_code: &CityCode, relay_preferred_exits: &[RelayPreferredExit]) -> (bool, bool, bool, u8, u32) {
        let is_preferred = relay_preferred_exits.iter().any(|e| e.id == candidate.id);
        let same_country = relay_city_code.country_code == candidate.city_code.country_code;
        let same_city = relay_city_code == &candidate.city_code;
        (is_preferred, same_city, same_country, candidate.tier, rand::random())
    }

    fn exclude(&self, candidate: &OneExit) -> bool {
        self.selected_exit_ids.contains(&candidate.id)
            || self.selected_datacenters.get(&candidate.datacenter_id) >= Some(&Saturating(2))
            || self.selected_cities.get(&candidate.city_code) >= Some(&Saturating(4))
            || self.selected_countries.get(&candidate.city_code.country_code) >= Some(&Saturating(6))
    }
}
