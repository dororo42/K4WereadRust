#!/usr/bin/env lua
-- Runs the same offline specs under Lua 5.1 or LuaJIT, without busted.
local root = arg[0]:match("^(.*)/tools/[^/]+$") or "."
local module_dir = root .. "/lua/"
local spec_dir = module_dir .. "spec/"
if arg[1] then
    module_dir = arg[1] .. "/weread/lib/"
    spec_dir = arg[1] .. "/spec/"
end
for _, name in ipairs({ "k4tls", "transport" }) do
    local path = module_dir .. name .. ".lua"
    package.preload["weread.lib." .. name] = function() return dofile(path) end
end

local assertions = setmetatable({}, { __call = function(_, ...) return assert(...) end })
function assertions.equals(expected, actual)
    if expected ~= actual then
        error("expected " .. tostring(expected) .. ", got " .. tostring(actual), 2)
    end
end
function assertions.is_nil(value) assertions.equals(nil, value) end
function assertions.is_true(value) assertions.equals(true, value) end
function assertions.is_false(value) assertions.equals(false, value) end
function assertions.truthy(value)
    if not value then error("expected a truthy value", 2) end
end

local total, passed, failed = 0, 0, 0
local env = setmetatable({ assert = assertions }, { __index = _G })
local before, after, suite
function env.before_each(fn) before = fn end
function env.after_each(fn) after = fn end
function env.describe(name, fn)
    suite, before, after = name, nil, nil
    fn()
end
function env.it(name, fn)
    total = total + 1
    local ok, err = pcall(function()
        if before then before() end
        fn()
    end)
    local clean_ok, clean_err = true, nil
    if after then clean_ok, clean_err = pcall(after) end
    if not clean_ok then ok, err = false, tostring(err or "") .. " teardown: " .. tostring(clean_err) end
    if ok then
        passed = passed + 1
    else
        failed = failed + 1
        print("FAIL " .. suite .. " / " .. name .. ": " .. tostring(err))
    end
end

for _, name in ipairs({ "k4tls", "transport" }) do
    local chunk = assert(loadfile(spec_dir .. name .. "_spec.lua"))
    setfenv(chunk, env)
    chunk()
end
print(string.format("test_k4tls_lua: %d tests, %d passed, %d failed (%s)",
    total, passed, failed, jit and jit.version or _VERSION))
os.exit(failed == 0 and 0 or 1)
