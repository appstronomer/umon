#!/bin/sh

func_rm_mv() {
    find /volume-dst/* ! -path /volume-dst/README.md -delete 2>/dev/null  && echo "[INFO] old artifacts were removed" || echo "[WARN] old files were not properly removed"
    mv -f /volume-src/* /volume-dst/ && echo "[INFO] new artifacts were moved to volume-dst" || echo "[WARN] new artifacts were not properly moved to volume-dst"
}

. /sh-container/resolve-uidgid.sh

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
