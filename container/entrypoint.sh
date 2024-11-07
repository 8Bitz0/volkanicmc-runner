#!/usr/bin/env bash

set -e
set -o pipefail

# Make sure the "USER" variable is set correctly
export USER=$(whoami)

# Ensure the user is on the "volkanic" user
if [[ $USER != "volkanic" ]]; then
    echo "This entrypoint script is only designed for the VolkanicMC Runner Docker container."
    exit 1
fi

if [ ! -f /config/vk-config.json ]; then
    cat <<EOF > /config/config.json
{
  "address": "0.0.0.0",
  "port": 56088,
  "storage": {
    "path": "/vk-store/store.json"
  }
}
EOF
fi

exec /usr/bin/volkanicmc-runner /config/config.json
