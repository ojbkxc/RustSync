# --- 构建阶段：前端 ---
FROM node:22-alpine AS frontend-builder
WORKDIR /app/web
COPY rustsync/web/package.json rustsync/web/package-lock.json ./
RUN npm ci
COPY rustsync/web/ ./
RUN npm run build

# --- 构建阶段：Rust 服务端 ---
FROM rust:1.85-alpine AS server-builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY server/Cargo.toml server/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release
RUN rm -rf src
COPY server/src ./src
COPY server/locales ./locales
COPY --from=frontend-builder /app/web/dist ./static
RUN cargo build --release

# --- 运行阶段 ---
FROM alpine:3.21
VOLUME /app/data
WORKDIR /app
COPY --from=server-builder /app/target/release/rustsync-server /app/rustsync-server
COPY --from=server-builder /app/locales ./locales
COPY --from=server-builder /app/static ./static
ENV RUSTSYNC_PORT=8023 \
    RUSTSYNC_EXPIRES=2 \
    RUSTSYNC_LOG_LEVEL=1 \
    RUSTSYNC_CONSOLE_LEVEL=2 \
    RUSTSYNC_LOG_SAVE=7 \
    RUSTSYNC_TASK_SAVE=0 \
    RUSTSYNC_TASK_TIMEOUT=72 \
    TZ=Asia/Shanghai
EXPOSE 8023
CMD ["./rustsync-server"]