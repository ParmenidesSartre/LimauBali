#!/bin/sh
# Inject the Lichess token from environment variable into config at startup.
# The token is never stored in the image or repo.

if [ -z "$LICHESS_TOKEN" ]; then
    echo "ERROR: LICHESS_TOKEN environment variable is not set."
    exit 1
fi

sed -i "s/TOKEN_PLACEHOLDER/${LICHESS_TOKEN}/" /lichess-bot/config.yml

echo "Starting LimauBali on Lichess..."
exec python /lichess-bot/lichess-bot.py -v
