FROM eclipse-mosquitto:2.0.15
LABEL project="appstronomer/umon" stage="final"
RUN apk add coreutils
COPY ./src /mosquitto/src
COPY ./config /mosquitto/config
WORKDIR /mosquitto/src
CMD ./start.sh
