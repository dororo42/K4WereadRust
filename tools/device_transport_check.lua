io.stdout:setvbuf("no")
local dir, base, ca_path = assert(arg[1]), assert(arg[2]), assert(arg[3])
local samples = tonumber(arg[4]) or 20
require("setupkoenv")
local ffi = require("ffi")
local socket = require("socket")
local http = require("socket.http")
local https = require("ssl.https")
local ltn12 = require("ltn12")
local k4tls = dofile(dir .. "/k4tls.lua")
assert(k4tls.is_available(), "k4tls library unavailable")
print("VERSION", k4tls.version())
local native = k4tls._lib
local ca_file = assert(io.open(ca_path, "rb"))
local pem = ca_file:read("*a")
ca_file:close()
local ca = ffi.new("unsigned char[?]", #pem, pem)
k4tls._lib = {
    k4tls_perform = function(req, resp)
        req.trust_pem = ca
        req.trust_pem_len = #pem
        return native.k4tls_perform(req, resp)
    end,
    k4tls_free_response = native.k4tls_free_response,
}

local function request(backend, path, options)
    options = options or {}
    local chunks = {}
    local block, total = options.block or 2, options.total or 4
    http.TIMEOUT, https.TIMEOUT = block, block
    local opts = {
        url = base .. path, method = options.method or "GET",
        headers = { ["User-Agent"] = "k4tls-acceptance/1", Accept = "*/*" },
        redirect = false, sink = ltn12.sink.table(chunks),
        block_timeout = block, total_timeout = total,
        transport_max_bytes = options.cap or 1024,
        verify = "peer", cafile = ca_path, protocol = "tlsv1_2",
    }
    print("REQUEST", backend, opts.method, path)
    local started = socket.gettime()
    local called, ok, status = pcall(backend == "k4tls" and k4tls.request or http.request, opts)
    local elapsed = (socket.gettime() - started) * 1000
    if not called then return nil, ok, table.concat(chunks), elapsed end
    return ok, status, table.concat(chunks), elapsed
end

for _, backend in ipairs({ "luasec", "k4tls" }) do
    for _, path in ipairs({ "/ok", "/chunked", "/interim" }) do
        local ok, status, body = request(backend, path)
        assert(ok and status == 200 and body == "abc\0def", backend .. path .. ": " .. tostring(status))
    end
    local ok, status, body = request(backend, "/empty")
    assert(ok and status == 200 and body == "", backend .. " empty")
    ok, status, body = request(backend, "/ok", { method = "HEAD" })
    assert(ok and status == 200 and body == "", backend .. " HEAD")
    ok, status, body = request(backend, "/304")
    assert(ok and status == 304 and body == "", backend .. " 304")
    ok, status, body = request(backend, "/error")
    assert(ok and status == 500 and body == "abc\0def", backend .. " HTTP500")
    ok, status = request(backend, "/drop")
    assert(not ok and type(status) == "string", backend .. " drop")
    ok, status = request(backend, "/slow", { block = 0.2, total = 0.4 })
    local timed_out = tostring(status):find("timeout", 1, true)
        or (backend == "luasec" and status == "wantread")
    assert(not ok and timed_out, backend .. " slow: " .. tostring(status))
    print("PASS", backend, "binary/chunked/interim/empty/HEAD/304/HTTP500/drop/timeout")
end

for _, path in ipairs({ "/ok", "/chunked" }) do
    local ok, err = request("k4tls", path, { cap = 3 })
    assert(not ok and tostring(err):find("exceeds", 1, true), "cap " .. path)
end
local ok, err, _, elapsed = request("k4tls", "/drip", { block = 0.2, total = 0.3 })
assert(not ok and tostring(err):find("timeout", 1, true) and elapsed < 600, "drip deadline: " .. tostring(err))
k4tls._lib = native
ok, err = request("k4tls", "/ok")
assert(not ok and tostring(err):find("certificate", 1, true), "untrusted CA must fail")
print("PASS", "k4tls", "caps/drip-deadline/untrusted-CA")
k4tls._lib = {
    k4tls_perform = function(req, resp)
        req.trust_pem = ca
        req.trust_pem_len = #pem
        return native.k4tls_perform(req, resp)
    end,
    k4tls_free_response = native.k4tls_free_response,
}

local timings = { luasec = {}, k4tls = {} }
for i = 1, samples do
    local order = i % 2 == 0 and { "k4tls", "luasec" } or { "luasec", "k4tls" }
    for _, backend in ipairs(order) do
        local success, status, body, ms = request(backend, "/ok")
        assert(success and status == 200 and body == "abc\0def", "benchmark failed: " .. backend)
        timings[backend][#timings[backend] + 1] = ms
        print(string.format("SAMPLE %s %d %.3f", backend, i, ms))
    end
end
for _, backend in ipairs({ "luasec", "k4tls" }) do
    local values, sum = timings[backend], 0
    table.sort(values)
    for _, ms in ipairs(values) do sum = sum + ms end
    local n = #values
    local median = n % 2 == 0 and (values[n / 2] + values[n / 2 + 1]) / 2 or values[(n + 1) / 2]
    print(string.format("SUMMARY %s n=%d mean=%.3f median=%.3f p95=%.3f ms",
        backend, n, sum / n, median, values[math.ceil(n * 0.95)]))
end
print("PASS device transport check; controlled TLS1.2 fixture only, not R4 business acceptance")
