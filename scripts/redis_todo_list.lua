#!lua name=todo_list

local function task_query_path(task_id, field)
    return string.format('$.tasks[?(@.id=="%s")].%s', task_id, field)
end

local function set_task_field(todo_id, task_id, field_name, field_value)
    local query_path = task_query_path(task_id, field_name)

    return redis.call('JSON.SET', todo_id, query_path, field_value)
end

local function get_todo(todo_id)
    return redis.call('JSON.GET', todo_id, '$')
end

local function set_task_done(keys, args)
    local is_done = 'false'
    if args[1] == '1' then
        is_done = 'true'
    end
    set_task_field(keys[1], keys[2], 'done', is_done)
    set_task_field(keys[1], keys[2], 'assignee', args[2])
    return get_todo(keys[1])
end

local function set_task_name(keys, args)
    set_task_field(keys[1], keys[2], 'name', string.format('"%s"', args[1]))
    return get_todo(keys[1])
end

local function set_task_assignee(keys, args)
    set_task_field(keys[1], keys[2], 'assignee', args[1])
    return get_todo(keys[1])
end

local function add_task(keys, args)
    redis.call('JSON.ARRAPPEND', keys[1], '$.tasks', args[1])
    return get_todo(keys[1])
end

local function set_todo_name(keys, args)
    redis.call('JSON.SET', keys[1], '$.name', string.format('"%s"', args[1]))
    return get_todo(keys[1])
end

local function todo_join(keys, args)
    redis.call('JSON.ARRAPPEND', keys[1], '$.connectedUsers', args[1])
    return get_todo(keys[1])
end

local function todo_leave(keys, args)
    local path = string.format('$.connectedUsers[?(@.id=="%s")]', args[1])
    redis.call('JSON.DEL', keys[1], path)
    return get_todo(keys[1])
end


redis.register_function('set_task_done', set_task_done)
redis.register_function('set_task_name', set_task_name)
redis.register_function('set_task_assignee', set_task_assignee)
redis.register_function('add_task', add_task)
redis.register_function('set_todo_name', set_todo_name)
redis.register_function('user_join_todo', todo_join)
redis.register_function('user_leave_todo', todo_leave)