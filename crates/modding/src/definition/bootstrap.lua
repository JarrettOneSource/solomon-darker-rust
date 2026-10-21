
local getinfo = debug.getinfo
local load_chunk = load
local site = __sd_site
local unknown_member = __sd_unknown_member
local unknown_global = __sd_unknown_global
local include_begin = __sd_include_begin
local include_end = __sd_include_end
local submit_reducer_result = __sd_submit_reducer_result
local pairs, ipairs, next, type, error, setmetatable, pcall, tostring = pairs, ipairs, next, type, error, setmetatable, pcall, tostring
local globals = _G

local function locate()
  local info = getinfo(3, "Sl")
  if info then site(info.source, info.currentline) else site(nil, 0) end
end

local function located(fn)
  return function(...)
    locate()
    return fn(...)
  end
end

local function wrap_namespace(namespace)
  for name, member in pairs(namespace) do
    if type(member) == "function" then namespace[name] = located(member) end
  end
end

wrap_namespace(sd.art)
wrap_namespace(sd.kit)
wrap_namespace(sd.rules)
wrap_namespace(sd.effect)
wrap_namespace(sd.intent)
wrap_namespace(sd.prefab)
wrap_namespace(sd.schema)
sd.mod = located(sd.mod)
sd.ref = located(sd.ref)

local register_reducer = sd.advanced.reducer
sd.advanced.reducer = function(spec)
  locate()
  if type(spec) == "table" and type(spec.reduce) == "function" then
    local reduce = spec.reduce
    spec.reduce = function(state, event, context)
      local next_state, intents = reduce(state, event, context)
      submit_reducer_result(next_state, intents)
    end
  end
  return register_reducer(spec)
end

for name, member in pairs(sd.kit) do sd[name] = member end
for _, name in ipairs({"on", "all", "first", "when", "after", "every"}) do sd[name] = sd.rules[name] end
for _, name in ipairs({"sprite", "sheet", "wearable", "sound", "music"}) do sd[name] = sd.art[name] end

local include_values = {}
sd.include = function(path)
  locate()
  local plan = include_begin(path)
  if plan.status == "cached" then
    local entry = include_values[path]
    return entry and entry.value
  end
  local chunk, message = load_chunk(plan.text, "@" .. path, "t", globals)
  if not chunk then
    include_end(path, false)
    error(message, 0)
  end
  local ok, result = pcall(chunk)
  include_end(path, ok)
  if not ok then error(result, 0) end
  include_values[path] = {value = result}
  return result
end

local function lock(namespace, label)
  local members, names = {}, {}
  for name, member in pairs(namespace) do
    members[name] = member
    names[#names + 1] = name
  end
  for _, name in ipairs(names) do namespace[name] = nil end
  setmetatable(namespace, {
    __index = function(_, key)
      local member = members[key]
      if member ~= nil then return member end
      error(unknown_member(label, key), 2)
    end,
    __newindex = function(_, key)
      error(label .. "." .. tostring(key) .. " cannot be assigned; Web Lua names are read-only", 2)
    end,
    __pairs = function() return next, members, nil end,
    __metatable = label .. " is read-only",
  })
end
lock(sd.advanced, "sd.advanced")
lock(sd.art, "sd.art")
lock(sd.effect, "sd.effect")
lock(sd.intent, "sd.intent")
lock(sd.kit, "sd.kit")
lock(sd.prefab, "sd.prefab")
lock(sd.rules, "sd.rules")
lock(sd.schema, "sd.schema")
lock(sd, "sd")

__sd_site = nil
__sd_unknown_member = nil
__sd_unknown_global = nil
__sd_include_begin = nil
__sd_include_end = nil
__sd_submit_reducer_result = nil
collectgarbage = nil
coroutine = nil
debug = nil
dofile = nil
io = nil
load = nil
loadfile = nil
module = nil
os = nil
package = nil
require = nil
rawset = nil

local global_members, global_names = {}, {}
for name, member in pairs(globals) do
  global_members[name] = member
  global_names[#global_names + 1] = name
end
for _, name in ipairs(global_names) do globals[name] = nil end

setmetatable(globals, {
  __index = function(_, key)
    local member = global_members[key]
    if member ~= nil then return member end
    local names = {}
    for name in pairs(global_members) do names[#names + 1] = name end
    error(unknown_global(key, names), 2)
  end,
  __newindex = function(_, key)
    error("global '" .. tostring(key) .. "' cannot be assigned; write local " .. tostring(key) .. " = ...", 2)
  end,
  __pairs = function() return next, global_members, nil end,
  __metatable = "Web Lua globals are protected",
})
