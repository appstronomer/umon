#!/bin/sh
while true
do
  echo "[INFO] $2: ping starting"
  stdbuf -oL ping $1 |
  {
    IFS= read -r line
    while IFS= read -r line
    do
      TIME=$(echo "$line" | sed 's/.*time=\([0-9]\{1,\}\.\{0,1\}[0-9]*\).*/\1/')
      test "$TIME" != "$line" && mosquitto_pub -h 127.0.0.1 -p 1883 -t "$2" -m "$TIME" || echo "[WARN] $2: ping line not parsed: \"$line\""
    done
  }
  echo "[WARN] $2: ping interrupted"
done