#!/bin/sh

if [ ! -z "$UIDGID" ]; then 
    if [ "$UIDGID" = "auto" ]; then
        echo '[INFO] UIDGID env is set to "auto"'
        host_uid=$(ls -dn . | awk '{print $3}')
        host_gid=$(ls -dn . | awk '{print $4}')
    elif echo "$UIDGID" | grep -qE '^[0-9]+:[0-9]+$' ; then
        echo "[INFO] UIDGID env is set to \"$UIDGID\""
        host_uid=$(echo "$UIDGID" | grep -oE '^[0-9]+')
        host_gid=$(echo "$UIDGID" | grep -oE '[0-9]+$')
    fi

    if [ -z "$host_uid" ] || [ -z "$host_gid" ]; then
        echo "[WARN] requested UID:GID is not properly formated \"$UIDGID\"; fallback to root"
    else
        echo "[INFO] requested UID:GID is set to \"$host_uid:$host_gid\""
        HOST_USERNAME=$(getent passwd "$host_uid" | cut -d: -f1)
        HOST_GROUPNAME=$(getent group "$host_gid" | cut -d: -f1)
        
        if [ -z "$HOST_GROUPNAME" ]; then
            echo "[INFO] creating group GID=\"$host_gid\"" 
            addgroup --gid "$host_gid" "host$host_gid" \
            && HOST_GROUPNAME="host$host_gid" \
            && echo "[INFO] group GID=$host_gid created as \"$HOST_GROUPNAME\"" \
            || echo "[WARN] group GID=$host_gid was not created"
        fi

        if [ -z "$HOST_USERNAME" ]; then
            echo "[INFO] creating user UID=$host_uid"
            if [ -z "$HOST_GROUPNAME" ]; then
                if adduser --disabled-password --no-create-home --uid "$host_uid" "host$host_uid" --gecos "" ; then 
                    export HOST_GROUPNAME="host$host_uid"
                    export HOST_USERNAME="host$host_uid"
                    echo "[INFO] user UID=$host_uid created as \"$HOST_USERNAME\" with group \"$HOST_GROUPNAME\""
                else
                    echo "[WARN] USER UID=$host_uid was not created; fallback to root"
                fi
            else
                if adduser --disabled-password --no-create-home --uid "$host_uid" --ingroup "$HOST_GROUPNAME" "host$host_uid" --gecos "" ; then
                    export HOST_GROUPNAME
                    export HOST_USERNAME="host$host_uid"
                    echo "[INFO] user UID=$host_uid created as \"$HOST_USERNAME\" with group \"$HOST_GROUPNAME\""
                else
                    echo "[WARN] USER UID=$host_uid was not created; fallback to root"
                fi
            fi
        fi
    fi
fi
