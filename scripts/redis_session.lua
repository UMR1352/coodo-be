#!lua name=session

local function prepend_id(key)
    return 'session-' .. key
end

local function session_store(keys, args)
    return redis.call('SET', prepend_id(keys[1]), unpack(args))
end

local function session_load(keys)
    return redis.call('GET', prepend_id(keys[1]))
end

local function session_destroy(keys)
    return redis.call('DEL', prepend_id(keys[1]))
end

local function session_clear_all()
    local ids = redis.call('KEYS', 'session-*')
    return redis.call('DEL', unpack(ids))
end

redis.register_function('session_store', session_store)
redis.register_function('session_load', session_load)
redis.register_function('session_destroy', session_destroy)
redis.register_function('session_clear_all', session_clear_all)
