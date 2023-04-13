#!/bin/sh

if [ ! -z "$UIDGID" ]; then 
    if [ "$UIDGID" = "auto" ]; then
        echo '[INFO] UIDGID env is set to "auto"'
        HOST_UID=$(ls -dn /volume-dst | awk '{print $3}')
        HOST_GID=$(ls -dn /volume-dst | awk '{print $4}')
    elif echo "$UIDGID" | grep -qE '^[0-9]+:[0-9]+$' ; then
        echo "[INFO] UIDGID env is set to \"$UIDGID\""
        HOST_UID=$(echo "$UIDGID" | grep -oE '^[0-9]+')
        HOST_GID=$(echo "$UIDGID" | grep -oE '[0-9]+$')
    fi

    if [ -z "$HOST_UID" ] || [ -z "$HOST_GID" ]; then
        echo "[WARN] requested UID:GID is not properly formated \"$UIDGID\"; fallback to root"
    else
        echo "[INFO] requested UID:GID is set to \"$HOST_UID:$HOST_GID\""
        HOST_USERNAME=$(getent passwd "$HOST_UID" | cut -d: -f1)
        HOST_GROUPNAME=$(getent group "$HOST_GID" | cut -d: -f1)
        
        if [ -z "$HOST_GROUPNAME" ]; then
            echo "[INFO] creating group GID=\"$HOST_GID\"" 
            addgroup -g "$HOST_GID" "host$HOST_GID" \
            && HOST_GROUPNAME="host$HOST_GID" \
            && echo "[INFO] group GID=$HOST_GID created as \"$HOST_GROUPNAME\"" \
            || echo "[WARN] group GID=$HOST_GID was not created"
        fi

        if [ -z "$HOST_USERNAME" ]; then
            echo "[INFO] creating user UID=$HOST_UID"
            if [ -z "$HOST_GROUPNAME" ]; then
                if adduser -DH -u "$HOST_UID" "host$HOST_UID" -s /bin/sh ; then 
                    HOST_GROUPNAME="host$HOST_UID"
                    HOST_USERNAME="host$HOST_UID"
                    echo "[INFO] user UID=$HOST_UID created as \"$HOST_USERNAME\" with group \"$HOST_GROUPNAME\""
                else
                    echo "[WARN] USER UID=$HOST_UID was not created; fallback to root"
                fi
            else
                if adduser -DH -u "$HOST_UID" -G "$HOST_GROUPNAME" "host$HOST_UID" -s /bin/sh ; then
                    HOST_USERNAME="host$HOST_UID"
                    echo "[INFO] user UID=$HOST_UID created as \"$HOST_USERNAME\" with group \"$HOST_GROUPNAME\""
                else
                    echo "[WARN] USER UID=$HOST_UID was not created; fallback to root"
                fi
            fi
        fi
    fi
fi


func_rm_mv() {
    find /volume-dst/* ! -path /volume-dst/README.md -delete 2>/dev/null  && echo "[INFO] old artifacts were removed" || echo "[WARN] old files were not properly removed"
    mv -f /volume-src/* /volume-dst/ && echo "[INFO] new artifacts were moved to volume-dst" || echo "[WARN] new artifacts were not properly moved to volume-dst"
}


if [ ! -z "$HOST_USERNAME" ]; then 
    if test -z "$HOST_GROUPNAME" && chown -R "$HOST_USERNAME" /volume-src || chown -R "$HOST_USERNAME:$HOST_GROUPNAME" /volume-src ; then
        test -f /volume-src/monitor && func_rm_mv
        echo "[INFO] starting app as $HOST_USERNAME"
        su -m "$HOST_USERNAME" -c './monitor serve 0.0.0.0:80 storage/config/main.json'
        exit $?
    else
        echo "[WARN] files ownership was not transferred to the requested user $HOST_USERNAME; fallback to root"
    fi
fi


test -f /volume-src/monitor && func_rm_mv
echo '[INFO] starting app as root'
./monitor serve 0.0.0.0:80 storage/config/main.json
