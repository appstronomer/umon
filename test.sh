#!/bin/bash

# echo "apple;banana;cherry" | cut -d ';' -f 2
# docker run -it --rm alpine:3.15 /bin/sh

# test -z $UIDGID && VAR='Hello, World!'
# test -z $VAR && echo 'Not set' || echo "Is set $VAR"

# find volume-dst/ ! -path volume-dst/storage/keep -delete 2>/dev/null && echo OK || echo NOK


# echo "s100:559" | grep -qE '^([0-9]+):[0-9]+$' && echo OK || echo NOK


# entryDir=$(dirname -- "$(readlink -f -- "$BASH_SOURCE")")
# echo $entryDir

# DOCKER_UID=$(id -u)
# HOST_UID=$(ls -ldn volume-dst/ | awk '{print $3}')
# echo $HOST_UID
# echo $DOCKER_UID
# if [ $DOCKER_UID -ne $HOST_UID ]; then
#     echo "adjusting container user UID"
#     usermod -u $HOST_UID -o root
    
# fi

# DOCKER_GID=$(id -g)
# HOST_GID=$(ls -ldn volume-dst/ | awk '{print $4}')
# echo $HOST_GID
# echo $DOCKER_GID
# if [ $DOCKER_GID -ne $HOST_GID ]; then
#     echo "adjusting container group GID"
#     groupmod -g $HOST_GID -o root
# fi

# to remove project label=project=appstronomer/umon
# docker system prune --force --all --filter label=project=appstronomer/umon
# then project label=project=appstronomer/umon could be started with "docker compose up"

# to start with rebuild without cache
# docker compose build --no-cache && docker system prune -fa --filter label=project=appstronomer/umon --filter label=stage=intermediate && docker compose up --force-recreate
# then old images will not be removed

# docker system prune --force --all --filter label=project=appstronomer/umon && docker compose build --no-cache && docker system prune -fa --filter label=project=appstronomer/umon --filter label=stage=intermediate && docker compose up --force-recreate

# rm -rf './volume-dst/!(README.md)'

# docker run -v $PWD/monitor/backend:/volume -v $PWD/cache/cargo-home:/cargo-home -v $PWD/volume-dst:/volume-dst -v  $PWD/monitor/sh-container:/sh-container --rm -t -e UIDGID=1000:1000 clux/muslrust:1.68.2 /sh-container/build-backend.sh

# docker run -it --rm clux/muslrust:1.68.2 /bin/bash

# https://stackoverflow.com/questions/10412162/how-can-i-simply-retrieve-the-absolute-path-of-the-current-script-multi-os-solu/12145443#12145443
# SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
# echo "${BASH_SOURCE[0]}"
# echo $SCRIPT_DIR

if ! command -v readlink > /dev/null; then 
    echo '[FAIL] readlink command is required for the script to work'
    exit 1
fi

# snippet source: https://stackoverflow.com/questions/10412162/how-can-i-simply-retrieve-the-absolute-path-of-the-current-script-multi-os-solu/12145443#12145443
self=$(
    self=${0}
    while [ -L "${self}" ]
    do
        cd "${self%/*}"
        self=$(readlink "${self}")
    done
    cd "${self%/*}"
    echo $(pwd -P)
)

echo "$self"