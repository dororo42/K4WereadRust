#!/bin/sh
# k4tls 交叉构建脚本（双产物线，依据 K4 用户态 armel glibc 2.12 事实）。
# 用法:
#   ./build.sh           # 全部：musl 静态 bench + armel glibc FFI .so
#   ./build.sh bench     # 仅 musl 静态 k4tls-bench（k4refresh-cli 同路径）
#   ./build.sh so        # 仅 armel libk4tls.so（KOReader FFI 用）
#   ./build.sh host      # 本机自测（cargo test + host 产物）
#
# 前置:
#   - rustup target add armv7-unknown-linux-musleabihf arm-unknown-linux-gnueabi
#   - musl 交叉 gcc: ~/x-tools/arm-linux-musleabihf-cross/（musl.cc 包，
#     GitHub TLS 失败走 LAN 代理 http://192.168.2.153:16492）
#   - armel 交叉 gcc: ~/x-tools/arm-kindle5-linux-gnueabi/（koxtoolchain 产物）
set -e

TARGET_MUSL="armv7-unknown-linux-musleabihf"
TARGET_ARMEL="arm-unknown-linux-gnueabi"
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE"

CC_MUSL="$HOME/x-tools/arm-linux-musleabihf-cross/bin/arm-linux-musleabihf-gcc"
CC_ARMEL="$HOME/x-tools/arm-kindle5-linux-gnueabi/bin/arm-kindle5-linux-gnueabi-gcc"

build_bench() {
    rustup target add "$TARGET_MUSL" 2>/dev/null || true
    if [ ! -x "$CC_MUSL" ]; then
        echo "[build.sh] 缺 musl 交叉 gcc（ring C 部分需要）:" >&2
        echo "  mkdir -p ~/x-tools && cd ~/x-tools && curl -x http://192.168.2.153:16492 -LO https://musl.cc/arm-linux-musleabihf-cross.tgz && tar xzf arm-linux-musleabihf-cross.tgz" >&2
        exit 3
    fi
    export CC_"$(echo "$TARGET_MUSL" | tr 'a-z-' 'A-Z_')"="$CC_MUSL"
    export AR_"$(echo "$TARGET_MUSL" | tr 'a-z-' 'A-Z_')"="${CC_MUSL%-gcc}-ar"
    cargo build --release --target "$TARGET_MUSL"
    echo "bench 产物:"
    file "target/$TARGET_MUSL/release/k4tls-bench" | sed 's/^/  /'
    ls -lh "target/$TARGET_MUSL/release/k4tls-bench" | awk '{print "  " $5, $9}'
}

build_so() {
    rustup target add "$TARGET_ARMEL" 2>/dev/null || true
    if [ ! -x "$CC_ARMEL" ]; then
        echo "[build.sh] 缺 armel 交叉 gcc（~/x-tools/arm-kindle5-linux-gnueabi）" >&2
        exit 3
    fi
    export CC_"$(echo "$TARGET_ARMEL" | tr 'a-z-' 'A-Z_')"="$CC_ARMEL"
    export AR_"$(echo "$TARGET_ARMEL" | tr 'a-z-' 'A-Z_')"="${CC_ARMEL%-gcc}-ar"
    cargo build --release --target "$TARGET_ARMEL" --lib
    echo "so 产物:"
    file "target/$TARGET_ARMEL/release/libk4tls.so" | sed 's/^/  /'
    ls -lh "target/$TARGET_ARMEL/release/libk4tls.so" | awk '{print "  " $5, $9}'
}

case "${1:-all}" in
  bench) build_bench ;;
  so)    build_so ;;
  host)
    cargo test
    cargo build --release
    echo "host 产物: target/release/{k4tls-bench,libk4tls.so}"
    ;;
  all)   build_bench; build_so ;;
  *) echo "unknown target: $1" >&2; exit 2 ;;
esac
