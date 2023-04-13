#!/bin/sh
mosquitto -d -c /mosquitto/config/mosquitto.conf & ./$START_TARGET
