#!/bin/sh

export CARGO_HOME=/cargo-home

. /sh-container/resolve-uidgid.sh

if [ ! -z "$HOST_USERNAME" ]; then
    # -v $PWD/monitor/backend:/volume -v $PWD/cache/cargo-home:/cargo-home -v $PWD/volume-dst:/volume-dst -v  $PWD/monitor/sh-container:/sh-container
    test -z "$HOST_GROUPNAME" && chown_arg="$HOST_USERNAME" || chown_arg="$HOST_USERNAME:$HOST_GROUPNAME"

    # TODO: check "! -d /volume/target" - that means "FILE not exists OR is not a directory" - file could exist as non-dir file - should handle that
    if test ! -d /volume/target || chown -R "$chown_arg" /volume/target && chown -R "$chown_arg" /cargo-home ; then
        echo "[INFO] directories ownership successfully changed"
        echo "[INFO] starting build as $HOST_USERNAME"
        su -m "$HOST_USERNAME" -c 'export PATH="$PATH:/root/.cargo/bin" && cargo build --release' && echo '[INFO] build successful' || ( echo '[FAIL] build failed'; exit 1 )
    else 
        echo "[FAIL] directories ownership change failed"
        exit 1
    fi
else
    echo "[INFO] starting build as root"
    cargo build --release && echo '[INFO] build successful' || ( echo '[FAIL] build failed'; exit 1 )
fi

test -f ./target/x86_64-unknown-linux-musl/release/monitor \
&& mv -f ./target/x86_64-unknown-linux-musl/release/monitor /volume-dst/monitor \
&& echo "[INFO] artifact successfuly moved to volume-dst" \
|| ( echo "[FAIL] artidact was not moved to to volume-dst"; exit 1 )
