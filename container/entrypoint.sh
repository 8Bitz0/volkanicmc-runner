#!/usr/bin/env bash

set -e
set -o pipefail

if [ ! -f /config/config.json ]; then
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
