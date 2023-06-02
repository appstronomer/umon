FROM node:18.7-buster AS adapter-web
LABEL project="appstronomer/umon" stage="intermediate"
WORKDIR /project
COPY ./adapter-web .
RUN npm install && npm run build

FROM clux/muslrust:1.68.2 AS backend
LABEL project="appstronomer/umon" stage="intermediate"
WORKDIR /project
COPY ./backend .
RUN RUST_BACKTRACE=1 cargo build --release

FROM alpine:3.15
LABEL project="appstronomer/umon" stage="final"
WORKDIR /volume-dst
COPY ./volume-src /volume-src
COPY ./sh-container /sh-container
COPY --from=adapter-web /project/dst/MQTTMonitor.js /volume-src/storage/public/MQTTMonitor.js
COPY --from=backend /project/target/x86_64-unknown-linux-musl/release/monitor /volume-src/monitor
CMD /sh-container/start-monitor.sh
