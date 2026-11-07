#!/bin/sh
set -eu

curl --fail-with-body --request POST http://127.0.0.1:3000/signup \
  --header 'content-type: application/json' \
  --data '{"email":"maintainer@example.org","name":"River Cleanup Team","donor_receipts":true,"volunteer_reminders":true,"campaign_reporting":false}'

