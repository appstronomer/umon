#!/bin/sh

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

# to remove project label=project=appstronomer/message-monitor
# docker system prune --force --all --filter label=project=appstronomer/message-monitor
# then project label=project=appstronomer/message-monitor could be started with "docker compose up"

# to start with rebuild without cache
# docker compose build --no-cache && docker system prune --force --all --filter label=project=appstronomer/message-monitor --filter label=stage=intermediate && docker compose up --force-recreate
# then old images will not be removed

# docker compose build --no-cache && docker system prune --force --all --filter label=project=appstronomer/message-monitor --filter label=stage=intermediate && docker compose up --force-recreate

# rm -rf './volume-dst/!(README.md)'
