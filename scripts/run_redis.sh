#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v redis-cli)" ]; then
  echo >&2 "Error: redis-cli is not installed."
  exit 1
fi

# Check if a custom user has been set, otherwise default to 'postgres'
REDIS_USER="${REDIS_USER:=emarconi}"
# Check if a custom password has been set, otherwise default to 'password'
REDIS_PASSWORD="${REDIS_PASSWORD:=supa_password}"
# Check if a custom port has been set, otherwise default to '5432'
REDIS_PORT="${REDIS_PORT:=6379}"
# Check if a custom host has been set, otherwise default to 'localhost'
REDIS_HOST="${REDIS_HOST:=localhost}"


if [[ -z $SKIP_DOCKER ]]; then
  # if a redis container is running, print instructions to kill it and exit
  RUNNING_REDIS_CONTAINER=$(docker ps --filter 'name=redis' --format '{{.ID}}')
  if [[ -n $RUNNING_REDIS_CONTAINER ]]; then
    echo >&2 "there is a redis container already running, kill it with"
    echo >&2 "    docker kill ${RUNNING_REDIS_CONTAINER}"
    exit 1
  fi

  REDIS_ARGS="--requirepass ${REDIS_PASSWORD} \
    --user ${REDIS_USER} on >${REDIS_PASSWORD} allkeys allcommands \
    --user default off nopass nocommands"

  docker run \
    --name "redis_$(date '+%s')" \
    -p "${REDIS_PORT}":6379 \
    -e REDIS_ARGS="${REDIS_ARGS}" \
    -d \
    redis/redis-stack:7.0.6-RC9 
fi

>&2 echo "Redis should be up and running on port ${REDIS_PORT} in a moment!"

# load lua modules
cd "$(dirname "$0")"
ls | grep -o '.*\.lua' | while read -r script ; do
    cat $script | redis-cli \
      -x \
      -h "${REDIS_HOST}" \
      -p "${REDIS_PORT}" \
      --user "${REDIS_USER}" \
      --pass "${REDIS_PASSWORD}" \
      FUNCTION LOAD REPLACE
done