#!/usr/bin/env bash
set -e

if [ -z "$1" ]; then
  echo "Error: No argument provided"
  exit 1
fi

if [[ ! "$1" =~ ^https?:// ]]; then
  echo "The string is not a URL"
  exit 2
fi

# e.g. https://v1.api.staging.obscura.net/api
configFile="/Library/Application Support/obscura-vpn/system-network-extension/config.json"
newConfig=$(sudo jq --indent 4 -r ".api_url = \"${1}\"" "$configFile")
echo "$newConfig" | sudo tee "$configFile" > /dev/null
