-- K5 传输层可控负载基准：双后端交替采样 /bench?size=N（回环 TLS1.2 夹具）。
-- 用法: luajit [-joff] device_bench_check.lua <k4tls目录> <基址> <CA路径> [每档采样数]
-- 仅传输层基准（等长合法请求、同机交替），不构成 R4 业务验收。
io.stdout:setvbuf("no")
require("setupkoenv")
local ffi = require("ffi")
local socket = require("socket")
local http = require("socket.http")
local ltn12 = require("ltn12")
local dir, base, ca_path = assert(arg[1]), assert(arg[2]), assert(arg[3])
local samples = tonumber(arg[4]) or 15

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

local SIZES = { 16384, 131072, 524288 }
for _, size in ipairs(SIZES) do
    local timings = { luasec = {}, k4tls = {} }
    for i = 1, samples do
        local order = i % 2 == 0 and { "k4tls", "luasec" } or { "luasec", "k4tls" }
        for _, backend in ipairs(order) do
            local chunks = {}
            http.TIMEOUT = 10
            local opts = {
                url = base .. "/bench?size=" .. size,
                method = "GET",
                headers = { ["User-Agent"] = "k4tls-bench/1", Accept = "application/json" },
                redirect = false, sink = ltn12.sink.table(chunks),
                block_timeout = 5, total_timeout = 15,
                transport_max_bytes = size + 1024,
                verify = "peer", cafile = ca_path, protocol = "tlsv1_2",
            }
            local started = socket.gettime()
            local called, ok, code = pcall(backend == "k4tls" and k4tls.request or http.request, opts)
            local elapsed = (socket.gettime() - started) * 1000
            local body = table.concat(chunks)
            assert(called and ok and code == 200 and #body == size,
                backend .. " size " .. size .. " #" .. i .. ": "
                .. tostring(code) .. " len=" .. #body)
            timings[backend][#timings[backend] + 1] = elapsed
            print(string.format("SAMPLE %s %d %d %.3f", backend, size, i, elapsed))
        end
    end
    for _, backend in ipairs({ "luasec", "k4tls" }) do
        local values, sum = timings[backend], 0
        table.sort(values)
        for _, ms in ipairs(values) do sum = sum + ms end
        local n = #values
        local median = n % 2 == 0 and (values[n / 2] + values[n / 2 + 1]) / 2 or values[(n + 1) / 2]
        print(string.format("SUMMARY %s size=%d n=%d mean=%.3f median=%.3f p95=%.3f",
            backend, size, n, sum / n, median, values[math.ceil(n * 0.95)]))
    end
end
print("BENCH DONE; controlled fixture transport benchmark, not full R4 business acceptance")
