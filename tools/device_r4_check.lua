-- K5 R4 前置传输基准：真实 weread.qq.com 端点、双后端交替、真域名 DNS worker 路径。
-- 用法: luajit device_r4_check.lua <k4tls目录> <信任链PEM> [样本数=15] [路径=/web/]
-- 边界: 未认证 GET 的传输层对比；不构成完整同步业务验收。需 LD_LIBRARY_PATH=/mnt/us/koreader/libs。
io.stdout:setvbuf("no")
require("setupkoenv")
local ffi = require("ffi")
local socket = require("socket")
local http = require("socket.http")
local ltn12 = require("ltn12")
local dir, ca_path = assert(arg[1]), assert(arg[2])
local samples = tonumber(arg[3]) or 15
local path = arg[4] or "/web/"
local base = "https://weread.qq.com"

local k4tls = dofile(dir .. "/k4tls.lua")
assert(k4tls.is_available(), "k4tls library unavailable")
print("VERSION", k4tls.version())
local native = k4tls._lib
local ca = assert(io.open(ca_path, "rb")):read("*a")
local ca_cdata = ffi.new("unsigned char[?]", #ca, ca)
k4tls._lib = {
    k4tls_perform = function(req, resp)
        req.trust_pem = ca_cdata
        req.trust_pem_len = #ca
        return native.k4tls_perform(req, resp)
    end,
    k4tls_free_response = native.k4tls_free_response,
}

local headers = {
    ["User-Agent"] = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36 Edg/135.0.0.0",
    ["Accept"] = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    ["Accept-Language"] = "zh-CN,zh;q=0.9",
}
local timings = { luasec = {}, k4tls = {} }
local failcount = { luasec = 0, k4tls = 0 }
for i = 1, samples do
    local order = i % 2 == 0 and { "k4tls", "luasec" } or { "luasec", "k4tls" }
    for _, backend in ipairs(order) do
        local chunks = {}
        http.TIMEOUT = 10
        local opts = {
            url = base .. path,
            method = "GET",
            headers = headers,
            redirect = false, sink = ltn12.sink.table(chunks),
            block_timeout = 10, total_timeout = 30,
            verify = "peer", cafile = ca_path, protocol = "tlsv1_2",
        }
        local started = socket.gettime()
        local called, ok, code = pcall(backend == "k4tls" and k4tls.request or http.request, opts)
        local elapsed = (socket.gettime() - started) * 1000
        local body = table.concat(chunks)
        if called and ok and code == 200 and #body > 1000 then
            timings[backend][#timings[backend] + 1] = elapsed
            print(string.format("SAMPLE %s %d %.3f len=%d", backend, i, elapsed, #body))
        else
            failcount[backend] = failcount[backend] + 1
            print(string.format("FAIL %s %d called=%s ok=%s code=%s len=%d",
                backend, i, tostring(called), tostring(ok), tostring(code), #body))
        end
    end
end
for _, backend in ipairs({ "luasec", "k4tls" }) do
    local values, sum = timings[backend], 0
    table.sort(values)
    for _, ms in ipairs(values) do sum = sum + ms end
    local n = #values
    if n > 0 then
        local median = n % 2 == 0 and (values[n / 2] + values[n / 2 + 1]) / 2 or values[(n + 1) / 2]
        print(string.format("SUMMARY %s n=%d fail=%d mean=%.3f median=%.3f p95=%.3f",
            backend, n, failcount[backend], sum / n, median, values[math.ceil(n * 0.95)]))
    else
        print(string.format("SUMMARY %s n=0 fail=%d ALL FAILED", backend, failcount[backend]))
    end
end
print("R4 TRANSPORT DONE; real-endpoint unauthenticated GET benchmark, not full sync acceptance")
