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
#     GitHub TLS 失败走 LAN 代理，默认 http://192.168.2.153:16492，
#     可用 K4_LAN_PROXY 覆盖；本脚本只提示不下载，下载/校验在 CI workflow）
#   - armel 交叉 gcc: ~/x-tools/arm-kindle5-linux-gnueabi/（koxtoolchain 产物）
# 注：Rust 编译产物统一由 CI/授权环境执行，本机只做源码级检查。
set -e

# 非交互/后台 shell 的 PATH 可能缺 ~/.cargo/bin（rustup/cargo 所在）。
case ":$PATH:" in
  *":$HOME/.cargo/bin:"*) ;;
  *) export PATH="$HOME/.cargo/bin:$PATH" ;;
esac

TARGET_MUSL="armv7-unknown-linux-musleabihf"
TARGET_ARMEL="arm-unknown-linux-gnueabi"
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE"

CC_MUSL="$HOME/x-tools/arm-linux-musleabihf-cross/bin/arm-linux-musleabihf-gcc"
CC_ARMEL="$HOME/x-tools/arm-kindle5-linux-gnueabi/bin/arm-kindle5-linux-gnueabi-gcc"
# 仅用于缺工具链时的下载提示文本；可被部署环境覆盖。
LAN_PROXY="${K4_LAN_PROXY:-http://192.168.2.153:16492}"

build_bench() {
    rustup target add "$TARGET_MUSL"
    if [ ! -x "$CC_MUSL" ]; then
        echo "[build.sh] 缺 musl 交叉 gcc（ring C 部分需要）:" >&2
        echo "  musl.cc/arm-linux-musleabihf-cross.tgz（GitHub TLS 失败走 LAN 代理 $LAN_PROXY，可用 K4_LAN_PROXY 覆盖）" >&2
        echo "  下载后必须核对 musl.cc 官方 sha256 再解压：供应链校验不可省略。" >&2
        exit 3
    fi
    # cc-rs 1.11+ 只读小写形式：CC_<target_with_underscores>（大写形式不被识别，
    # 早期 build.sh 用 tr 转大写导致 cc-rs 静默回落默认交叉前缀搜索）。
    export CC_armv7_unknown_linux_musleabihf="$CC_MUSL"
    export AR_armv7_unknown_linux_musleabihf="${CC_MUSL%-gcc}-ar"
    cargo build --locked --release --target "$TARGET_MUSL"
    echo "bench 产物:"
    file "target/$TARGET_MUSL/release/k4tls-bench" | sed 's/^/  /'
    ls -lh "target/$TARGET_MUSL/release/k4tls-bench" | awk '{print "  " $5, $9}'
}

build_so() {
    rustup target add "$TARGET_ARMEL"
    if [ ! -x "$CC_ARMEL" ]; then
        echo "[build.sh] 缺 armel 交叉 gcc（~/x-tools/arm-kindle5-linux-gnueabi）" >&2
        exit 3
    fi
    export CC_arm_unknown_linux_gnueabi="$CC_ARMEL"
    export AR_arm_unknown_linux_gnueabi="${CC_ARMEL%-gcc}-ar"
    # .cargo/config.toml 里的绝对路径只是本机示例；用 env 覆盖，避免换机即断。
    export CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABI_LINKER="$CC_ARMEL"
    cargo build --locked --release --target "$TARGET_ARMEL" --lib
    echo "so 产物:"
    file "target/$TARGET_ARMEL/release/libk4tls.so" | sed 's/^/  /'
    ls -lh "target/$TARGET_ARMEL/release/libk4tls.so" | awk '{print "  " $5, $9}'
}

case "${1:-all}" in
  bench) build_bench ;;
  so)    build_so ;;
  host)
    cargo test --locked
    cargo build --locked --release
    echo "host 产物: target/release/{k4tls-bench,libk4tls.so}"
    ;;
  all)   build_bench; build_so ;;
  *) echo "unknown target: $1" >&2; exit 2 ;;
esac
