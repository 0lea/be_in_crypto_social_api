-- KEYS[1]: rate limit key (e.g., rl:user:uuid or rl:ip:1.2.3.4)
-- ARGV[1]: bucket capacity (maximum burst size)
-- ARGV[2]: fill rate (tokens added per second)
-- ARGV[3]: current timestamp (Unix seconds)
-- ARGV[4]: request cost (typically 1)

-- Retrieve current bucket state
local bucket = redis.call("HMGET", KEYS[1], "tokens", "last_update")
local capacity = tonumber(ARGV[1])
local fill_rate = tonumber(ARGV[2])
local now = tonumber(ARGV[3])
local cost = tonumber(ARGV[4])

-- Initialize values if the key does not exist (first request)
local last_tokens = tonumber(bucket[1]) or capacity
local last_update = tonumber(bucket[2]) or now

-- Calculate token replenishment based on time elapsed
local delta = math.max(0, now - last_update)
local current_tokens = math.min(capacity, last_tokens + (delta * fill_rate))

local allowed = false

-- Determine if the request can be processed
if current_tokens >= cost then
	current_tokens = current_tokens - cost
	allowed = true

	-- Update bucket state and persist to Redis
	redis.call("HMSET", KEYS[1], "tokens", current_tokens, "last_update", now)

	-- Set TTL (Time To Live) to clean up idle keys after 1 hour
	redis.call("EXPIRE", KEYS[1], 3600)
else
	allowed = false
end

-- Calculate time remaining until the bucket is fully replenished
local reset_after = math.ceil((capacity - current_tokens) / fill_rate)

-- Return results: [remaining_tokens, seconds_until_full, request_allowed (1 or 0)]
return {
	math.floor(current_tokens),
	reset_after,
	allowed and 1 or 0,
}
