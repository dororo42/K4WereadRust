#!/usr/bin/env lua
-- k4tls.lua / transport.lua 纯函数层测试（host lua5.1，无 ffi 依赖）。
-- 运行: lua tools/test_k4tls_lua.lua   （在仓根执行）

local luaunit_ok, _ = pcall(require, "luaunit") -- 可选；缺失则用内置断言

local function assert_eq(actual, expected, msg)
    if actual ~= expected then
        error(string.format("%s: expected %q, got %q",
            msg or "assert_eq", tostring(expected), tostring(actual)), 2)
    end
end

local function assert_true(v, msg)
    if not v then error(msg or "expected true", 2) end
end

local function assert_raises(fn, needle, msg)
    local ok, err = pcall(fn)
    if ok then error(msg or "expected error, none raised", 2) end
    if needle and not tostring(err):lower():find(needle:lower(), 1, true) then
        error(string.format("%s: error %q lacks %q", msg or "assert_raises",
            tostring(err), needle), 2)
    end
end

package.path = "lua/?.lua;" .. package.path
local k4tls = require("k4tls")
local transport = require("transport")

-- ---------------------------------------------------------------------------
-- headers_table_to_list：保留原名、剔除传输专属键、排序稳定
local list, n = k4tls.headers_table_to_list({
    Cookie = "a=1",
    Accept = "application/json",
    ["Content-Length"] = "5",
    Host = "weread.qq.com",
    Connection = "close",
    ["X-Custom"] = "v",
})
assert_eq(n, 3, "reserved headers dropped (kept: Accept/Cookie/X-Custom)")
assert_true(list[1] == "Accept: application/json" or list[1] == "Cookie: a=1",
    "only accepted headers remain")
for _, h in ipairs(list) do
    assert_true(not h:lower():find("host"), "no host leaked: " .. h)
    assert_true(not h:lower():find("content%-length"), "no content-length leaked")
end

-- headers_blob_to_table：原名键、同名后者胜、跳过残行
local t = k4tls.headers_blob_to_table(
    "Content-Type: application/json\r\nLocation: https://x/Y\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\n")
assert_eq(t["Content-Type"], "application/json", "original case key")
assert_eq(t.Location, "https://x/Y", "location preserved")
assert_eq(t["Set-Cookie"], "b=2", "duplicate: last wins")
local empty = k4tls.headers_blob_to_table("")
assert_true(next(empty) == nil, "empty blob -> empty table")

-- source_to_body：多 chunk 拼接与 nil 源
assert_eq(k4tls.source_to_body(nil), nil, "nil source")
local parts = { "ab", "cd" }
local i = 0
assert_eq(k4tls.source_to_body(function()
    i = i + 1
    return parts[i]
end), "abcd", "source chunks concatenated")

-- error_text：映射到 client.lua 重试分类词表（transient/resolution/timeout）
assert_true(k4tls.error_text(-13):find("timeout", 1, true), "-13 -> timeout")
assert_true(k4tls.error_text(-11):find("connection refused", 1, true), "-11 -> refused")
assert_true(k4tls.error_text(-10):find("host or service not provided", 1, true),
    "-10 -> resolution class (ipv4_dns.is_resolution_error 命中)")
assert_true(k4tls.error_text(-15):find("closed", 1, true), "-15 -> closed")
assert_true(k4tls.error_text(-17):find("certificate", 1, true), "-17 -> cert")
assert_true(#k4tls.error_text(-99, "boom") > 0, "-99 has text")

-- select：host 无 ffi/.so → 不可用；强制 luasec 恒回退
local backend, reason = k4tls.select("luasec")
assert_eq(backend, nil, "forced luasec returns nil")
assert_true(tostring(reason):find("luasec"), "reason mentions luasec")

-- transport.resolve：默认/强制路径
transport.backend = "luasec"
local b2, r2 = transport.resolve()
assert_eq(b2, nil, "transport forced luasec -> no k4tls")
assert_eq(r2, "luasec", "reason is luasec")

-- transport.request 回退路径（host 无 socket.http → 报 no backend；有则走 luasocket）
local ok, err = pcall(function()
    return transport.request({ url = "https://example.invalid/" })
end)
-- host 上 socket.http 可能存在（luasocket rocks）或不存在；只验证不崩溃且形状可控
assert_true(ok or tostring(err):find("no backend"), "fallback path deterministic")

print("test_k4tls_lua: PASS (headers/error mapping/source/select/fallback)")
